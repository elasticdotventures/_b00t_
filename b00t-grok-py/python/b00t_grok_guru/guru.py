"""High-level Python interface to b00t-grok Rust core."""

import json
import asyncio
import logging
from typing import List, Optional, Dict, Any
from datetime import datetime

# Import the compiled Rust module when available
try:
    import b00t_grok
    RUST_MODULE_AVAILABLE = True
except ImportError:
    RUST_MODULE_AVAILABLE = False
    logging.warning("b00t_grok Rust module not available, using mock implementation")

# Import embedding model
try:
    from InstructorEmbedding import INSTRUCTOR
    EMBEDDING_MODEL_AVAILABLE = True
except ImportError:
    EMBEDDING_MODEL_AVAILABLE = False
    logging.warning("InstructorEmbedding not available, using mock embeddings")

from .types import ChunkData, DigestResponse, AskResponse, LearnResponse


class GrokGuru:
    """High-level interface to b00t-grok knowledgebase system."""
    
    def __init__(
        self, 
        qdrant_url: str = "https://a0cfd978-2e95-499c-93cc-9acd66b16d35.us-west-1-0.aws.cloud.qdrant.io:6333",
        api_key: Optional[str] = None
    ):
        self.qdrant_url = qdrant_url
        self.api_key = api_key or "dummy-key"
        self.client: Optional[Any] = None
        self._initialized = False
        self._start_time = datetime.now()
        
    async def initialize(self) -> None:
        """Initialize the grok client."""
        if RUST_MODULE_AVAILABLE:
            self.client = b00t_grok.PyGrokClient(self.qdrant_url, self.api_key)
        else:
            # Mock client for development
            self.client = MockGrokClient()
        
        self._initialized = True
        logging.info("GrokGuru initialized")
    
    def _ensure_initialized(self) -> None:
        """Ensure the client is initialized."""
        if not self._initialized:
            raise RuntimeError("GrokGuru not initialized. Call await initialize() first.")
    
    async def digest(self, topic: str, content: str) -> DigestResponse:
        """Digest content into a knowledge chunk."""
        self._ensure_initialized()
        
        try:
            if RUST_MODULE_AVAILABLE:
                chunk_json = self.client.digest(topic, content)
                chunk_dict = json.loads(chunk_json)
            else:
                chunk_dict = await self.client.digest(topic, content)
            
            chunk = ChunkData(
                id=chunk_dict["id"],
                content=chunk_dict["content"],
                datum=chunk_dict["datum"],
                topic=chunk_dict["metadata"]["topic"],
                tags=chunk_dict["metadata"].get("tags", []),
                attribution_url=chunk_dict["attribution"].get("url"),
                attribution_filename=chunk_dict["attribution"].get("filename"),
                created_at=chunk_dict["attribution"]["date"],
                vector=chunk_dict.get("vector")
            )
            
            return DigestResponse(chunk=chunk)
            
        except Exception as e:
            logging.error(f"Error in digest: {e}")
            return DigestResponse(success=False, message=str(e))
    
    async def ask(self, query: str, topic: Optional[str] = None, limit: int = 5) -> AskResponse:
        """Search the knowledgebase."""
        self._ensure_initialized()
        
        try:
            if RUST_MODULE_AVAILABLE:
                results_json = self.client.ask(query, topic)
                results_dicts = [json.loads(r) for r in results_json]
            else:
                results_dicts = await self.client.ask(query, topic)
            
            chunks = []
            for result_dict in results_dicts[:limit]:
                chunk = ChunkData(
                    id=result_dict["id"],
                    content=result_dict["content"],
                    datum=result_dict["datum"],
                    topic=result_dict["metadata"]["topic"],
                    tags=result_dict["metadata"].get("tags", []),
                    attribution_url=result_dict["attribution"].get("url"),
                    attribution_filename=result_dict["attribution"].get("filename"),
                    created_at=result_dict["attribution"]["date"],
                    vector=result_dict.get("vector")
                )
                chunks.append(chunk)
            
            return AskResponse(
                results=chunks,
                query=query,
                total_found=len(results_dicts)
            )
            
        except Exception as e:
            logging.error(f"Error in ask: {e}")
            return AskResponse(
                success=False,
                message=str(e),
                results=[],
                query=query,
                total_found=0
            )
    
    async def learn(self, content: str, source: Optional[str] = None) -> LearnResponse:
        """Learn from content, creating multiple chunks."""
        self._ensure_initialized()
        
        try:
            if RUST_MODULE_AVAILABLE:
                chunks_json = self.client.learn(source or "direct_input", content)
                chunks_dicts = [json.loads(c) for c in chunks_json]
            else:
                chunks_dicts = await self.client.learn(source or "direct_input", content)
            
            chunks = []
            for chunk_dict in chunks_dicts:
                chunk = ChunkData(
                    id=chunk_dict["id"],
                    content=chunk_dict["content"],
                    datum=chunk_dict["datum"],
                    topic=chunk_dict["metadata"]["topic"],
                    tags=chunk_dict["metadata"].get("tags", []),
                    attribution_url=chunk_dict["attribution"].get("url"),
                    attribution_filename=chunk_dict["attribution"].get("filename"),
                    created_at=chunk_dict["attribution"]["date"],
                    vector=chunk_dict.get("vector")
                )
                chunks.append(chunk)
            
            return LearnResponse(
                chunks=chunks,
                source=source,
                chunks_created=len(chunks)
            )
            
        except Exception as e:
            logging.error(f"Error in learn: {e}")
            return LearnResponse(
                success=False,
                message=str(e),
                chunks=[],
                source=source,
                chunks_created=0
            )
    
    def get_status(self) -> Dict[str, Any]:
        """Get current status."""
        uptime = (datetime.now() - self._start_time).total_seconds()
        
        return {
            "status": "ok" if self._initialized else "initializing",
            "version": "0.1.0",
            "qdrant_connected": self._initialized and self.client is not None,
            "embedding_model_loaded": self._initialized,
            "uptime_seconds": uptime,
            "rust_module_available": RUST_MODULE_AVAILABLE
        }


class MockGrokClient:
    """Mock implementation for development when Rust module is not available."""
    
    def __init__(self):
        self.embedding_model = None
        if EMBEDDING_MODEL_AVAILABLE:
            # Use instructor-large model for better embeddings
            self.embedding_model = INSTRUCTOR('hkunlp/instructor-large')
            logging.info("Loaded InstructorEmbedding model: hkunlp/instructor-large")
    
    def _generate_embedding(self, text: str, instruction: str = "Represent this text for retrieval:") -> List[float]:
        """Generate embedding for text using Instructor model or mock."""
        if self.embedding_model:
            try:
                embeddings = self.embedding_model.encode([[instruction, text]])
                return embeddings[0].tolist()  # Convert numpy array to list
            except Exception as e:
                logging.error(f"Embedding generation failed: {e}")
                # Fall back to mock embedding
                pass
        
        # Mock embedding - deterministic based on text hash for consistency
        import hashlib
        text_hash = int(hashlib.md5(text.encode()).hexdigest(), 16)
        return [(text_hash % 10000) / 10000.0 + i * 0.001 for i in range(768)]  # instructor-large uses 768 dims
    
    async def digest(self, topic: str, content: str) -> Dict[str, Any]:
        """Mock digest implementation."""
        # Generate real or mock embedding
        vector = self._generate_embedding(content, f"Represent this {topic} knowledge:")
        
        return {
            "id": f"mock-{hash(content) % 100000}",  # More realistic ID
            "content": content,
            "datum": topic,
            "attribution": {
                "url": None,
                "filename": None,
                "date": datetime.now().isoformat()
            },
            "metadata": {
                "topic": topic,
                "tags": [],
                "created_at": datetime.now().isoformat()
            },
            "vector": vector
        }
    
    async def ask(self, query: str, topic: Optional[str] = None) -> List[Dict[str, Any]]:
        """Mock ask implementation."""
        # Return empty results for now
        return []
    
    async def learn(self, source: str, content: str) -> List[Dict[str, Any]]:
        """Mock learn implementation."""
        # Simple chunking by double newlines
        chunks = [chunk.strip() for chunk in content.split("\n\n") if chunk.strip()]
        
        results = []
        for i, chunk_content in enumerate(chunks):
            if len(chunk_content) < 10:
                continue
                
            # Infer topic from source
            topic = "general"
            if "rust" in source.lower() or source.endswith(".rs"):
                topic = "rust"
            elif "python" in source.lower() or source.endswith(".py"):
                topic = "python"
                
            results.append({
                "id": f"mock-chunk-{i}",
                "content": chunk_content,
                "datum": topic,
                "attribution": {
                    "url": source if source.startswith("http") else None,
                    "filename": source if not source.startswith("http") else None,
                    "date": datetime.now().isoformat()
                },
                "metadata": {
                    "topic": topic,
                    "tags": [f"chunk_{i}"],
                    "created_at": datetime.now().isoformat()
                },
                "vector": self._generate_embedding(chunk_content, f"Represent this {topic} knowledge:")
            })
        
        return results