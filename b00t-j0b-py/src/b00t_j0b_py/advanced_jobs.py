"""Enhanced RQ job definitions with advanced chunking and grok integration.

Extends the basic jobs with:
- Advanced chunking strategies
- b00t-grok integration for knowledge storage
- Hierarchical chunk relationships
- Enhanced metadata processing
- Batch processing capabilities
"""

import asyncio
import logging
from typing import Dict, Any, List, Optional
from rq import get_current_job
from datetime import datetime

from .grok_integration import (
    AdvancedGrokProcessor, 
    get_grok_processor,
    process_crawl_result_job as base_crawl_job,
    process_batch_crawl_results_job as base_batch_job
)
from .advanced_chunking import ChunkingStrategy
from .redis_client import tracker


def enhanced_crawl_url_job(url: str, depth: int = 0, max_depth: Optional[int] = None, 
                          chunking_strategy: str = "hybrid") -> Dict[str, Any]:
    """Enhanced RQ job that crawls URL and processes with advanced chunking."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting enhanced crawl of {url} at depth {depth}")
    
    try:
        # Import here to avoid circular dependencies
        from .jobs import crawl_url_job
        
        # First perform the basic crawl
        crawl_result = crawl_url_job(url, depth, max_depth)
        
        if crawl_result["status"] != "success":
            return {
                **crawl_result,
                "enhanced": False,
                "reason": "base_crawl_failed"
            }
        
        # Then enhance with advanced chunking
        enhancement_result = asyncio.run(_enhance_crawl_result(
            crawl_result, 
            ChunkingStrategy(chunking_strategy)
        ))
        
        return {
            **crawl_result,
            "enhanced": True,
            "enhancement": enhancement_result,
            "job_id": job_id
        }
        
    except Exception as e:
        print(f"[{job_id}] Enhanced crawl failed for {url}: {e}")
        return {
            "status": "error",
            "url": url,
            "depth": depth,
            "error": str(e),
            "enhanced": False,
            "job_id": job_id
        }


def enhanced_digest_url_job(url: str, depth: int = 1, 
                           chunking_strategy: str = "hybrid") -> Dict[str, Any]:
    """Enhanced digest job with advanced chunking for all discovered pages."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting enhanced digest of {url} with max depth {depth}")
    
    try:
        # Import here to avoid circular dependencies
        from .jobs import digest_url_job
        
        # Perform base digest
        digest_result = digest_url_job(url, depth)
        
        if digest_result["status"] != "success":
            return {
                **digest_result,
                "enhanced": False,
                "reason": "base_digest_failed"
            }
        
        # Enhance all crawled pages
        enhanced_results = []
        for page_result in digest_result.get("results", []):
            enhancement = asyncio.run(_enhance_crawl_result(
                page_result,
                ChunkingStrategy(chunking_strategy)
            ))
            enhanced_results.append({
                "page": page_result,
                "enhancement": enhancement
            })
        
        return {
            **digest_result,
            "enhanced": True,
            "enhanced_results": enhanced_results,
            "total_enhanced": len(enhanced_results),
            "job_id": job_id
        }
        
    except Exception as e:
        print(f"[{job_id}] Enhanced digest failed for {url}: {e}")
        return {
            "status": "error",
            "start_url": url,
            "max_depth": depth,
            "error": str(e),
            "enhanced": False,
            "job_id": job_id
        }


def batch_enhance_crawl_results_job(crawl_results: List[Dict[str, Any]], 
                                   chunking_strategy: str = "hybrid") -> Dict[str, Any]:
    """Batch job to enhance multiple crawl results with advanced chunking."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting batch enhancement of {len(crawl_results)} crawl results")
    
    try:
        # Use the grok integration batch processor
        batch_result = process_batch_crawl_results_job(crawl_results)
        
        return {
            **batch_result,
            "job_type": "enhanced_batch_processing",
            "chunking_strategy": chunking_strategy,
            "job_id": job_id
        }
        
    except Exception as e:
        print(f"[{job_id}] Batch enhancement failed: {e}")
        return {
            "status": "error",
            "batch_size": len(crawl_results),
            "error": str(e),
            "job_id": job_id
        }


def knowledge_integration_job(crawl_results: List[Dict[str, Any]], 
                             topic: Optional[str] = None) -> Dict[str, Any]:
    """Specialized job for integrating crawled content into b00t-grok knowledge base."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting knowledge integration for {len(crawl_results)} results")
    
    try:
        async def process_knowledge_integration():
            processor = await get_grok_processor()
            
            total_chunks = 0
            successful_integrations = 0
            failed_integrations = 0
            integration_details = []
            
            for crawl_result in crawl_results:
                try:
                    result = await processor.process_crawl_result(crawl_result)
                    
                    if result.is_success():
                        data = result.unwrap()
                        total_chunks += data.get("chunks_created", 0)
                        successful_integrations += 1
                        
                        integration_details.append({
                            "url": crawl_result.get("url"),
                            "status": "success", 
                            "chunks_created": data.get("chunks_created", 0),
                            "strategies_used": data.get("strategies_used", [])
                        })
                    else:
                        failed_integrations += 1
                        integration_details.append({
                            "url": crawl_result.get("url"),
                            "status": "failed",
                            "error": str(result.failure())
                        })
                        
                except Exception as e:
                    failed_integrations += 1
                    integration_details.append({
                        "url": crawl_result.get("url", "unknown"),
                        "status": "error",
                        "error": str(e)
                    })
            
            # Store integration summary
            summary = {
                "job_id": job_id,
                "topic": topic,
                "processed_at": datetime.utcnow().isoformat(),
                "total_sources": len(crawl_results),
                "successful_integrations": successful_integrations,
                "failed_integrations": failed_integrations,
                "total_chunks_created": total_chunks,
                "integration_details": integration_details,
                "processor_stats": processor.get_processing_stats()
            }
            
            # Cache integration summary
            import json
            integration_key = f"knowledge_integration:{job_id}"
            tracker.redis.setex(
                integration_key,
                86400 * 30,  # 30 days
                json.dumps(summary)
            )
            
            return summary
        
        result = asyncio.run(process_knowledge_integration())
        
        print(f"[{job_id}] Knowledge integration completed: {result['total_chunks_created']} chunks created")
        
        return {
            "status": "success",
            "job_type": "knowledge_integration",
            **result
        }
        
    except Exception as e:
        print(f"[{job_id}] Knowledge integration failed: {e}")
        return {
            "status": "error",
            "job_type": "knowledge_integration", 
            "batch_size": len(crawl_results),
            "error": str(e),
            "job_id": job_id
        }


def hierarchical_analysis_job(url: str, analysis_depth: int = 2) -> Dict[str, Any]:
    """Specialized job for deep hierarchical analysis of website structure."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting hierarchical analysis of {url} with depth {analysis_depth}")
    
    try:
        async def perform_hierarchical_analysis():
            # First crawl with structural focus
            from .jobs import digest_url_job
            digest_result = digest_url_job(url, analysis_depth)
            
            if digest_result["status"] != "success":
                return {
                    "status": "failed",
                    "reason": "digest_failed",
                    "error": digest_result.get("error", "Unknown error")
                }
            
            # Process each page with structural emphasis
            processor = await get_grok_processor()
            
            hierarchical_map = {}
            total_structural_elements = 0
            
            for page_result in digest_result.get("results", []):
                page_url = page_result.get("url", "unknown")
                
                # Force structural chunking for hierarchy analysis
                processor.chunking_engine.default_strategy = ChunkingStrategy.STRUCTURAL
                
                result = await processor.process_crawl_result(page_result)
                
                if result.is_success():
                    data = result.unwrap()
                    
                    # Analyze structural elements
                    chunk_types = data.get("processing_stats", {}).get("chunk_types", {})
                    structural_count = sum(count for chunk_type, count in chunk_types.items() 
                                         if chunk_type in ["heading", "code", "table", "list"])
                    
                    hierarchical_map[page_url] = {
                        "total_chunks": data.get("chunks_created", 0),
                        "structural_elements": structural_count,
                        "chunk_types": chunk_types,
                        "hierarchical_chunks": data.get("hierarchical_chunks", 0),
                        "depth": page_result.get("depth", 0)
                    }
                    
                    total_structural_elements += structural_count
            
            return {
                "status": "success",
                "analyzed_pages": len(hierarchical_map),
                "total_structural_elements": total_structural_elements,
                "hierarchical_map": hierarchical_map,
                "depth_distribution": _analyze_depth_distribution(hierarchical_map)
            }
        
        result = asyncio.run(perform_hierarchical_analysis())
        
        return {
            "job_type": "hierarchical_analysis",
            "url": url,
            "analysis_depth": analysis_depth,
            "job_id": job_id,
            **result
        }
        
    except Exception as e:
        print(f"[{job_id}] Hierarchical analysis failed for {url}: {e}")
        return {
            "status": "error",
            "job_type": "hierarchical_analysis",
            "url": url,
            "error": str(e),
            "job_id": job_id
        }


def cleanup_enhanced_data_job() -> Dict[str, Any]:
    """Clean up enhanced processing data and relationships."""
    job = get_current_job()
    job_id = job.id if job else "local"
    
    print(f"[{job_id}] Starting cleanup of enhanced processing data")
    
    try:
        # Clean up chunk relationships
        relationship_keys = tracker.redis.keys("chunk_relationships:*")
        relationship_count = len(relationship_keys) if relationship_keys else 0
        
        if relationship_keys:
            tracker.redis.delete(*relationship_keys)
        
        # Clean up integration summaries older than 30 days
        integration_keys = tracker.redis.keys("knowledge_integration:*")
        integration_count = len(integration_keys) if integration_keys else 0
        
        if integration_keys:
            tracker.redis.delete(*integration_keys)
        
        # Clean up advanced chunk cache
        advanced_keys = tracker.redis.keys("*:advanced_chunks")
        advanced_count = len(advanced_keys) if advanced_keys else 0
        
        if advanced_keys:
            tracker.redis.delete(*advanced_keys)
        
        print(f"[{job_id}] Cleanup completed: {relationship_count + integration_count + advanced_count} keys removed")
        
        return {
            "status": "success",
            "job_type": "enhanced_cleanup",
            "cleaned_items": {
                "chunk_relationships": relationship_count,
                "integration_summaries": integration_count,
                "advanced_chunks": advanced_count
            },
            "total_cleaned": relationship_count + integration_count + advanced_count,
            "job_id": job_id
        }
        
    except Exception as e:
        print(f"[{job_id}] Enhanced cleanup failed: {e}")
        return {
            "status": "error",
            "job_type": "enhanced_cleanup",
            "error": str(e),
            "job_id": job_id
        }


async def _enhance_crawl_result(crawl_result: Dict[str, Any], 
                               strategy: ChunkingStrategy) -> Dict[str, Any]:
    """Helper function to enhance a single crawl result."""
    try:
        processor = await get_grok_processor()
        
        # Override strategy if specified
        if strategy != ChunkingStrategy.HYBRID:
            processor.chunking_engine.default_strategy = strategy
        
        result = await processor.process_crawl_result(crawl_result)
        
        if result.is_success():
            return result.unwrap()
        else:
            return {
                "status": "enhancement_failed",
                "error": str(result.failure())
            }
            
    except Exception as e:
        return {
            "status": "enhancement_error", 
            "error": str(e)
        }


def _analyze_depth_distribution(hierarchical_map: Dict[str, Dict[str, Any]]) -> Dict[str, Any]:
    """Analyze the distribution of content across crawl depths."""
    depth_stats = {}
    
    for url, data in hierarchical_map.items():
        depth = data.get("depth", 0)
        
        if depth not in depth_stats:
            depth_stats[depth] = {
                "pages": 0,
                "total_chunks": 0,
                "structural_elements": 0
            }
        
        depth_stats[depth]["pages"] += 1
        depth_stats[depth]["total_chunks"] += data.get("total_chunks", 0)
        depth_stats[depth]["structural_elements"] += data.get("structural_elements", 0)
    
    return depth_stats