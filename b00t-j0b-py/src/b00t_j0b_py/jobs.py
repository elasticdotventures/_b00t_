"""RQ job definitions for web crawling tasks."""

from typing import Dict, Any, List, Optional
from rq import get_current_job
import time
from returns.result import Result, Success, Failure

from .config import config
from .crawler import crawler
from .parsers import registry as parser_registry
from .content_processors import content_registry
from .redis_client import tracker


def crawl_url_job(url: str, depth: int = 0, max_depth: Optional[int] = None) -> Dict[str, Any]:
    """RQ job to crawl a single URL."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting crawl of {url} at depth {depth}")
    
    try:
        # Set max depth
        if max_depth is not None:
            crawler.max_depth = max_depth
        
        # Crawl the URL
        result = crawler.crawl_url(url, depth)
        
        match result:
            case Success(data):
                print(f"[{job_id}] Successfully crawled {url}")
                
                # Try to parse with specialized parser
                parser_result = parser_registry.parse_content(
                    url, 
                    data.get("content", ""), 
                    data.get("content_type", "text/html")
                )
                
                match parser_result:
                    case Success(parsed_data):
                        # Update with parsed data
                        data.update({
                            "parsed_title": parsed_data.title,
                            "parsed_content": parsed_data.content,
                            "parsed_metadata": parsed_data.metadata,
                            "parsed_tags": parsed_data.tags
                        })
                        print(f"[{job_id}] Applied specialized parser for {url}")
                    case Failure(error):
                        print(f"[{job_id}] Parser failed for {url}: {error}")
                
                # Queue child links if depth allows
                if depth < crawler.max_depth:
                    child_links = data.get("links", [])
                    if child_links:
                        queued = tracker.add_to_queue(set(child_links), depth + 1)
                        match queued:
                            case Success(count):
                                print(f"[{job_id}] Queued {count} child links from {url}")
                            case Failure(error):
                                print(f"[{job_id}] Failed to queue child links: {error}")
                
                return {
                    "status": "success",
                    "url": url,
                    "depth": depth,
                    "data": data,
                    "job_id": job_id
                }
                
            case Failure(error):
                print(f"[{job_id}] Failed to crawl {url}: {error}")
                return {
                    "status": "error",
                    "url": url,
                    "depth": depth,
                    "error": str(error),
                    "job_id": job_id
                }
    
    except Exception as e:
        print(f"[{job_id}] Unexpected error crawling {url}: {e}")
        return {
            "status": "error",
            "url": url,
            "depth": depth,
            "error": str(e),
            "job_id": job_id
        }


def digest_url_job(url: str, depth: int = 1) -> Dict[str, Any]:
    """RQ job to digest (crawl recursively) a URL."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting digest of {url} with max depth {depth}")
    
    try:
        # Perform recursive crawl
        results = crawler.crawl_recursive(url, depth)
        
        processed_results = []
        for data in results:
            # Try to parse each result with specialized parser
            parser_result = parser_registry.parse_content(
                data["url"], 
                data.get("content", ""), 
                data.get("content_type", "text/html")
            )
            
            match parser_result:
                case Success(parsed_data):
                    data.update({
                        "parsed_title": parsed_data.title,
                        "parsed_content": parsed_data.content,
                        "parsed_metadata": parsed_data.metadata,
                        "parsed_tags": parsed_data.tags
                    })
                case Failure(error):
                    print(f"[{job_id}] Parser failed for {data['url']}: {error}")
            
            processed_results.append(data)
        
        print(f"[{job_id}] Digest completed: {len(processed_results)} pages crawled")
        
        return {
            "status": "success",
            "start_url": url,
            "max_depth": depth,
            "results": processed_results,
            "total_pages": len(processed_results),
            "job_id": job_id
        }
    
    except Exception as e:
        print(f"[{job_id}] Unexpected error during digest of {url}: {e}")
        return {
            "status": "error",
            "start_url": url,
            "max_depth": depth,
            "error": str(e),
            "job_id": job_id
        }


def process_binary_content_job(url: str, content: bytes, content_type: str) -> Dict[str, Any]:
    """RQ job to process binary content (PDFs, images, etc.)."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Processing binary content from {url} ({content_type})")
    
    try:
        result = content_registry.process_content(content, content_type, url)
        
        match result:
            case Success(processed_text):
                print(f"[{job_id}] Successfully processed binary content from {url}")
                
                # Cache the processed content
                cache_result = tracker.cache_content(url, processed_text, "text/markdown")
                match cache_result:
                    case Success(_):
                        print(f"[{job_id}] Cached processed content for {url}")
                    case Failure(error):
                        print(f"[{job_id}] Failed to cache content: {error}")
                
                return {
                    "status": "success",
                    "url": url,
                    "content_type": content_type,
                    "processed_content": processed_text,
                    "content_size": len(content),
                    "job_id": job_id
                }
                
            case Failure(error):
                print(f"[{job_id}] Failed to process binary content from {url}: {error}")
                return {
                    "status": "error",
                    "url": url,
                    "content_type": content_type,
                    "error": str(error),
                    "job_id": job_id
                }
    
    except Exception as e:
        print(f"[{job_id}] Unexpected error processing binary content from {url}: {e}")
        return {
            "status": "error",
            "url": url,
            "content_type": content_type,
            "error": str(e),
            "job_id": job_id
        }


def cleanup_old_data_job() -> Dict[str, Any]:
    """RQ job to clean up old crawl data."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting cleanup of old crawl data")
    
    try:
        # Get current stats
        stats_before = tracker.get_stats()
        
        # TODO: Implement actual cleanup logic
        # For now, just return current stats
        
        stats_after = tracker.get_stats()
        
        print(f"[{job_id}] Cleanup completed")
        
        return {
            "status": "success",
            "stats_before": stats_before,
            "stats_after": stats_after,
            "job_id": job_id
        }
    
    except Exception as e:
        print(f"[{job_id}] Error during cleanup: {e}")
        return {
            "status": "error",
            "error": str(e),
            "job_id": job_id
        }