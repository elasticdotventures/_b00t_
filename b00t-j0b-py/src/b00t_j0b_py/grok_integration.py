"""Integration layer between b00t-j0b-py crawler and b00t-grok knowledge system.

Provides the post-processing pipeline:
1. Crawled content → Advanced chunking → b00t-grok storage
2. Multi-strategy chunking with hierarchy preservation
3. Metadata enrichment and relationship tracking
4. Job queue integration for scalable processing
"""

import asyncio
import logging
from typing import List, Dict, Any, Optional
from dataclasses import asdict
from returns.result import Result, Success, Failure
from datetime import datetime

try:
    from b00t_grok_guru import GrokGuru, ChunkData
    GROK_GURU_AVAILABLE = True
except ImportError:
    GROK_GURU_AVAILABLE = False
    logging.warning("b00t-grok-guru not available, using mock implementation")

from .advanced_chunking import (
    AdvancedChunkingEngine, 
    AdvancedChunk, 
    ChunkingStrategy,
    process_crawled_content
)
from .config import config
from .redis_client import tracker


class GrokIntegrationError(Exception):
    """Errors in grok integration processing."""
    pass


class MockGrokGuru:
    """Mock implementation when b00t-grok-guru is not available."""
    
    def __init__(self, *args, **kwargs):
        self.initialized = False
    
    async def initialize(self):
        """Mock initialization."""
        self.initialized = True
        logging.info("MockGrokGuru initialized")
    
    async def learn(self, source: str, content: str) -> Any:
        """Mock learn method."""
        # Simulate processing
        chunks_created = len(content.split('\n\n'))  # Simple chunk count
        
        return type('MockLearnResponse', (), {
            'success': True,
            'chunks_created': chunks_created,
            'source': source,
            'chunks': [
                type('MockChunk', (), {
                    'id': f'mock-{i}',
                    'content': chunk,
                    'topic': 'general',
                    'tags': ['mock'],
                    'created_at': datetime.utcnow().isoformat()
                })
                for i, chunk in enumerate(content.split('\n\n')[:chunks_created])
            ]
        })()


class AdvancedGrokProcessor:
    """Advanced processor that combines chunking strategies with grok integration."""
    
    def __init__(self, 
                 qdrant_url: Optional[str] = None,
                 api_key: Optional[str] = None):
        self.qdrant_url = qdrant_url
        self.api_key = api_key
        self.grok_guru: Optional[Any] = None
        self.chunking_engine: Optional[AdvancedChunkingEngine] = None
        self._initialized = False
        
        # Processing stats
        self.processed_urls = 0
        self.total_chunks_created = 0
        self.processing_errors = 0
        
    async def initialize(self):
        """Initialize the grok processor."""
        try:
            if GROK_GURU_AVAILABLE:
                self.grok_guru = GrokGuru(self.qdrant_url, self.api_key)
                await self.grok_guru.initialize()
            else:
                self.grok_guru = MockGrokGuru()
                await self.grok_guru.initialize()
            
            self.chunking_engine = AdvancedChunkingEngine(self.grok_guru)
            self._initialized = True
            
            logging.info("AdvancedGrokProcessor initialized")
            
        except Exception as e:
            raise GrokIntegrationError(f"Failed to initialize grok processor: {e}")
    
    def _ensure_initialized(self):
        """Ensure processor is initialized."""
        if not self._initialized:
            raise GrokIntegrationError("Processor not initialized. Call await initialize() first.")
    
    async def process_crawl_result(self, crawl_result: Dict[str, Any]) -> Result[Dict[str, Any], Exception]:
        """Process a single crawl result through advanced chunking to grok storage."""
        self._ensure_initialized()
        
        try:
            url = crawl_result.get("url", "unknown")
            content = crawl_result.get("content", "")
            
            if not content or len(content.strip()) < 50:
                return Success({
                    "status": "skipped",
                    "reason": "content_too_short",
                    "url": url,
                    "chunks_created": 0
                })
            
            # Step 1: Advanced chunking with multiple strategies
            chunks = await self._perform_advanced_chunking(crawl_result)
            
            if not chunks:
                return Success({
                    "status": "skipped", 
                    "reason": "no_chunks_created",
                    "url": url,
                    "chunks_created": 0
                })
            
            # Step 2: Process chunks through grok for storage
            grok_results = await self._process_chunks_with_grok(chunks, crawl_result)
            
            # Step 3: Store chunk relationships and metadata
            relationship_info = await self._store_chunk_relationships(chunks, crawl_result)
            
            # Update processing stats
            self.processed_urls += 1
            self.total_chunks_created += len(chunks)
            
            # Cache processing result
            cache_result = tracker.cache_content(
                f"{url}:advanced_chunks",
                f"Processed {len(chunks)} chunks with advanced strategies",
                "application/json"
            )
            
            return Success({
                "status": "success",
                "url": url,
                "chunks_created": len(chunks),
                "strategies_used": list(set(chunk.metadata.strategy_used.value for chunk in chunks)),
                "grok_response": grok_results,
                "hierarchical_chunks": len([c for c in chunks if c.children]),
                "relationship_info": relationship_info,
                "processing_stats": {
                    "total_chars": sum(chunk.metadata.char_count for chunk in chunks),
                    "total_words": sum(chunk.metadata.word_count for chunk in chunks),
                    "chunk_types": {
                        chunk_type.value: len([c for c in chunks if c.metadata.chunk_type == chunk_type])
                        for chunk_type in set(chunk.metadata.chunk_type for chunk in chunks)
                    }
                }
            })
            
        except Exception as e:
            self.processing_errors += 1
            logging.error(f"Error processing crawl result for {crawl_result.get('url')}: {e}")
            return Failure(e)
    
    async def _perform_advanced_chunking(self, crawl_result: Dict[str, Any]) -> List[AdvancedChunk]:
        """Perform advanced chunking with strategy selection."""
        content = crawl_result.get("content", "")
        source_url = crawl_result.get("url", "")
        content_type = crawl_result.get("content_type", "text/markdown")
        parsed_metadata = crawl_result.get("parsed_metadata", {})
        
        # Determine optimal chunking strategy based on content characteristics
        strategy = self._select_chunking_strategy(content, content_type, parsed_metadata)
        
        # Perform chunking
        chunks = self.chunking_engine.chunk_content(content, source_url, strategy)
        
        # Enrich with crawler-specific metadata
        chunks = self.chunking_engine.enrich_metadata(chunks, {
            **parsed_metadata,
            "crawler_metadata": {
                "status_code": crawl_result.get("status_code", 200),
                "depth": crawl_result.get("depth", 0),
                "links_found": len(crawl_result.get("links", [])),
                "content_type": content_type
            }
        })
        
        return chunks
    
    def _select_chunking_strategy(self, content: str, content_type: str, metadata: Dict[str, Any]) -> ChunkingStrategy:
        """Select optimal chunking strategy based on content analysis."""
        
        # Code-heavy content → Structural
        if (metadata.get("platform") in ["github", "crates"] or 
            "```" in content or 
            content.count("```") > 2):
            return ChunkingStrategy.STRUCTURAL
        
        # Documentation/articles → Hybrid  
        if (content.count("#") > 3 or  # Many headings
            len(content) > 5000 or     # Long content
            metadata.get("platform") in ["pypi", "npm"]):
            return ChunkingStrategy.HYBRID
        
        # Short content → Structural only
        if len(content) < 1000:
            return ChunkingStrategy.STRUCTURAL
        
        # Default to semantic for general content
        return ChunkingStrategy.SEMANTIC
    
    async def _process_chunks_with_grok(self, chunks: List[AdvancedChunk], crawl_result: Dict[str, Any]) -> Dict[str, Any]:
        """Process chunks through b00t-grok for vector storage."""
        source = crawl_result.get("url", "direct_input")
        
        # Combine all chunk content for grok processing
        combined_content = "\n\n".join([
            f"# Chunk {i+1} ({chunk.metadata.chunk_type.value})\n{chunk.content}"
            for i, chunk in enumerate(chunks)
        ])
        
        # Use grok's learn method for batch processing
        grok_response = await self.grok_guru.learn(source, combined_content)
        
        return {
            "success": getattr(grok_response, 'success', True),
            "chunks_stored": getattr(grok_response, 'chunks_created', 0),
            "source": source
        }
    
    async def _store_chunk_relationships(self, chunks: List[AdvancedChunk], crawl_result: Dict[str, Any]) -> Dict[str, Any]:
        """Store hierarchical relationships and enhanced metadata."""
        url = crawl_result.get("url", "")
        
        # Build relationship map
        relationships = {
            "url": url,
            "total_chunks": len(chunks),
            "hierarchical_structure": self._build_hierarchy_map(chunks),
            "chunk_metadata": [chunk.to_dict() for chunk in chunks],
            "processing_timestamp": datetime.utcnow().isoformat()
        }
        
        # Store in Redis for relationship queries
        import hashlib
        relationship_key = f"chunk_relationships:{hashlib.sha256(url.encode()).hexdigest()[:16]}"
        import json
        cache_result = tracker.redis.setex(
            relationship_key,
            86400 * 7,  # 7 days
            json.dumps(relationships)
        )
        
        return {
            "relationship_key": relationship_key,
            "hierarchical_chunks": len(relationships["hierarchical_structure"]),
            "stored": bool(cache_result)
        }
    
    def _build_hierarchy_map(self, chunks: List[AdvancedChunk]) -> List[Dict[str, Any]]:
        """Build a map of hierarchical relationships."""
        hierarchy = []
        
        for chunk in chunks:
            if not chunk.parent:  # Root level chunks
                chunk_info = {
                    "chunk_id": chunk.metadata.chunk_id,
                    "type": chunk.metadata.chunk_type.value,
                    "has_children": len(chunk.children) > 0,
                    "children": []
                }
                
                # Add children recursively
                if chunk.children:
                    chunk_info["children"] = self._build_children_map(chunk.children)
                
                hierarchy.append(chunk_info)
        
        return hierarchy
    
    def _build_children_map(self, children: List[AdvancedChunk]) -> List[Dict[str, Any]]:
        """Build children map recursively."""
        children_map = []
        
        for child in children:
            child_info = {
                "chunk_id": child.metadata.chunk_id,
                "type": child.metadata.chunk_type.value,
                "has_children": len(child.children) > 0,
            }
            
            if child.children:
                child_info["children"] = self._build_children_map(child.children)
            
            children_map.append(child_info)
        
        return children_map
    
    def get_processing_stats(self) -> Dict[str, Any]:
        """Get current processing statistics."""
        return {
            "initialized": self._initialized,
            "processed_urls": self.processed_urls,
            "total_chunks_created": self.total_chunks_created,
            "processing_errors": self.processing_errors,
            "avg_chunks_per_url": (self.total_chunks_created / self.processed_urls) if self.processed_urls > 0 else 0,
            "error_rate": (self.processing_errors / max(1, self.processed_urls + self.processing_errors)) * 100
        }


# Job functions for RQ integration
async def process_crawl_result_job(crawl_result: Dict[str, Any]) -> Dict[str, Any]:
    """RQ job for processing crawl results with advanced chunking."""
    processor = AdvancedGrokProcessor()
    await processor.initialize()
    
    result = await processor.process_crawl_result(crawl_result)
    
    if isinstance(result, Success):
        return {
            "status": "success",
            "job_type": "advanced_chunking",
            **result.unwrap()
        }
    elif isinstance(result, Failure):
        return {
            "status": "error", 
            "job_type": "advanced_chunking",
            "error": str(result.failure())
        }
    else:
        return {
            "status": "error",
            "job_type": "advanced_chunking", 
            "error": f"Unknown result type: {type(result)}"
        }


def process_batch_crawl_results_job(crawl_results: List[Dict[str, Any]]) -> Dict[str, Any]:
    """RQ job for batch processing multiple crawl results."""
    async def process_batch():
        processor = AdvancedGrokProcessor()
        await processor.initialize()
        
        results = []
        for crawl_result in crawl_results:
            result = await processor.process_crawl_result(crawl_result)
            results.append(result)
        
        return {
            "batch_size": len(crawl_results),
            "successful": len([r for r in results if isinstance(r, Success)]),
            "failed": len([r for r in results if isinstance(r, Failure)]),
            "processing_stats": processor.get_processing_stats(),
            "results": [r.unwrap() if isinstance(r, Success) else {"error": str(r.failure())} for r in results]
        }
    
    return asyncio.run(process_batch())


# Global processor instance for reuse
_global_processor: Optional[AdvancedGrokProcessor] = None


async def get_grok_processor() -> AdvancedGrokProcessor:
    """Get or create global grok processor instance."""
    global _global_processor
    
    if _global_processor is None:
        _global_processor = AdvancedGrokProcessor()
        await _global_processor.initialize()
    
    return _global_processor