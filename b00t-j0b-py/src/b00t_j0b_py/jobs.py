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


def cleanup_old_data_job(
    max_age_days: int = 7,
    clean_failed_jobs: bool = True,
    clean_finished_jobs: bool = True,
) -> Dict[str, Any]:
    """RQ job to clean up old crawl data and stale job registries.

    Args:
        max_age_days: Maximum age in days for crawl data (default: 7)
        clean_failed_jobs: Clean failed job registry (default: True)
        clean_finished_jobs: Clean finished job registry older than max_age (default: True)

    Returns:
        Dictionary with cleanup stats and results
    """
    job = get_current_job()
    job_id = job.id if job else "local"

    print(f"[{job_id}] Starting cleanup of old crawl data (max_age={max_age_days}d)")

    try:
        from datetime import datetime, timedelta
        from .rq_integration import get_redis_connection, get_all_queues

        # Get current stats before cleanup
        stats_before = tracker.get_stats()
        cleanup_results = {
            "crawl_data_cleaned": 0,
            "failed_jobs_cleaned": 0,
            "finished_jobs_cleaned": 0,
            "stale_queues_cleaned": 0,
            "errors": []
        }

        # Calculate cutoff timestamp
        cutoff_time = datetime.utcnow() - timedelta(days=max_age_days)

        # 1. Clean old crawl data beyond TTL
        # Note: Redis TTL handles expiration automatically, but we can clean manually
        # for data that may have been set without TTL or needs earlier cleanup
        try:
            redis_conn = get_redis_connection()

            # Scan and clean old crawled URLs
            cursor = 0
            crawl_cleaned = 0
            while True:
                cursor, keys = redis_conn.scan(cursor, match="crawl:url:*", count=100)
                for key in keys:
                    try:
                        data = redis_conn.get(key)
                        if data:
                            import json
                            crawl_info = json.loads(data)
                            crawled_at = datetime.fromisoformat(crawl_info.get("crawled_at", ""))
                            if crawled_at < cutoff_time:
                                redis_conn.delete(key)
                                crawl_cleaned += 1
                    except Exception as e:
                        cleanup_results["errors"].append(f"Error processing {key}: {str(e)}")

                if cursor == 0:
                    break

            cleanup_results["crawl_data_cleaned"] = crawl_cleaned
            print(f"[{job_id}] Cleaned {crawl_cleaned} old crawl entries")

        except Exception as e:
            cleanup_results["errors"].append(f"Crawl data cleanup error: {str(e)}")

        # 2. Clean RQ job registries
        try:
            queues = get_all_queues()

            for queue in queues:
                # Clean failed jobs if requested
                if clean_failed_jobs:
                    failed_registry = queue.failed_job_registry
                    failed_count = len(failed_registry)
                    if failed_count > 0:
                        # Clean all failed jobs older than max_age
                        failed_jobs = failed_registry.get_job_ids()
                        for job_id_to_clean in failed_jobs:
                            try:
                                from rq.job import Job
                                job_obj = Job.fetch(job_id_to_clean, connection=redis_conn)
                                if job_obj.ended_at and job_obj.ended_at < cutoff_time:
                                    failed_registry.remove(job_obj)
                                    cleanup_results["failed_jobs_cleaned"] += 1
                            except Exception:
                                pass  # Job may already be gone

                # Clean finished jobs if requested
                if clean_finished_jobs:
                    finished_registry = queue.finished_job_registry
                    finished_jobs = finished_registry.get_job_ids()
                    for job_id_to_clean in finished_jobs:
                        try:
                            from rq.job import Job
                            job_obj = Job.fetch(job_id_to_clean, connection=redis_conn)
                            if job_obj.ended_at and job_obj.ended_at < cutoff_time:
                                finished_registry.remove(job_obj)
                                cleanup_results["finished_jobs_cleaned"] += 1
                        except Exception:
                            pass  # Job may already be gone

            print(f"[{job_id}] Cleaned {cleanup_results['failed_jobs_cleaned']} failed jobs, "
                  f"{cleanup_results['finished_jobs_cleaned']} finished jobs")

        except Exception as e:
            cleanup_results["errors"].append(f"Job registry cleanup error: {str(e)}")

        # 3. Clean stale processing queues (items that never got processed)
        try:
            for queue_name in ["default", "high", "low"]:
                queue_key = f"crawl:queue:{queue_name}"
                queue_size = redis_conn.scard(queue_key)

                # If queue has stale items (optional: check timestamp in items)
                if queue_size > 1000:  # Arbitrary threshold for "stale"
                    print(f"[{job_id}] Warning: Queue {queue_name} has {queue_size} items")
                    # Optionally clear or trim
                    # For now, just report

        except Exception as e:
            cleanup_results["errors"].append(f"Queue cleanup error: {str(e)}")

        # Get stats after cleanup
        stats_after = tracker.get_stats()

        print(f"[{job_id}] Cleanup completed - removed {cleanup_results['crawl_data_cleaned']} entries, "
              f"{cleanup_results['failed_jobs_cleaned'] + cleanup_results['finished_jobs_cleaned']} jobs")

        return {
            "status": "success",
            "stats_before": stats_before,
            "stats_after": stats_after,
            "cleanup_results": cleanup_results,
            "max_age_days": max_age_days,
            "job_id": job_id
        }

    except Exception as e:
        print(f"[{job_id}] Error during cleanup: {e}")
        return {
            "status": "error",
            "error": str(e),
            "job_id": job_id
        }