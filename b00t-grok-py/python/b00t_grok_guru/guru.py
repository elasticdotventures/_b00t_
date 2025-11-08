"""High-level Python interface to b00t-grok Rust core."""

import json
import asyncio
import logging
from typing import List, Optional, Dict, Any
from datetime import datetime
from pathlib import Path
from urllib.parse import urlparse

# Import the compiled Rust module when available
try:
    import b00t_grok
    RUST_MODULE_AVAILABLE = True
except ImportError:
    RUST_MODULE_AVAILABLE = False
    logging.warning("b00t_grok Rust module not available, using mock implementation")

# Import RAG-Anything integration
try:
    from .rag_anything_integration import RAGAnythingIntegration, RAG_ANYTHING_AVAILABLE
except ImportError:
    RAG_ANYTHING_AVAILABLE = False
    logging.warning("RAG-Anything integration not available")

# Import embedding model (fallback)
try:
    from InstructorEmbedding import INSTRUCTOR
    EMBEDDING_MODEL_AVAILABLE = True
except ImportError:
    EMBEDDING_MODEL_AVAILABLE = False
    logging.warning("InstructorEmbedding not available, using mock embeddings")

# Import crawl4ai for web crawling
try:
    from crawl4ai import AsyncWebCrawler
    CRAWL4AI_AVAILABLE = True
except ImportError:
    CRAWL4AI_AVAILABLE = False
    logging.warning("crawl4ai not available, URL crawling disabled")

from .types import ChunkData, DigestResponse, AskResponse, LearnResponse


class GrokGuru:
    """High-level interface to b00t-grok knowledgebase system."""

    def __init__(
        self,
        qdrant_url: str = "http://localhost:6333",
        api_key: Optional[str] = None,
        use_rag_anything: bool = True,
        llm_model_func: Optional[Any] = None,
        vision_model_func: Optional[Any] = None,
        embedding_func: Optional[Any] = None
    ):
        self.qdrant_url = qdrant_url
        self.api_key = api_key or "dummy-key"
        self.use_rag_anything = use_rag_anything and RAG_ANYTHING_AVAILABLE
        self.client: Optional[Any] = None
        self.rag_integration: Optional[RAGAnythingIntegration] = None
        self._initialized = False
        self._start_time = datetime.now()

        # 🤓 Model functions for RAG-Anything
        self._llm_model_func = llm_model_func
        self._vision_model_func = vision_model_func
        self._embedding_func = embedding_func

    async def initialize(self) -> None:
        """Initialize the grok client."""
        # Initialize RAG-Anything if requested
        if self.use_rag_anything:
            try:
                self.rag_integration = RAGAnythingIntegration(
                    qdrant_url=self.qdrant_url,
                    qdrant_api_key=self.api_key,
                    llm_model_func=self._llm_model_func,
                    vision_model_func=self._vision_model_func,
                    embedding_func=self._embedding_func
                )
                await self.rag_integration.initialize()
                logging.info("RAG-Anything integration enabled")
            except Exception as e:
                logging.warning(f"RAG-Anything initialization failed: {e}, falling back to Rust module")
                self.use_rag_anything = False

        # Initialize Rust module or mock client as fallback
        if RUST_MODULE_AVAILABLE and not self.use_rag_anything:
            self.client = b00t_grok.PyGrokClient(self.qdrant_url, self.api_key)
        else:
            # Mock client for development
            self.client = MockGrokClient(rag_integration=self.rag_integration)

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
    
    async def ask(
        self,
        query: str,
        topic: Optional[str] = None,
        limit: int = 5,
        mode: str = "hybrid",
        multimodal_content: Optional[Dict[str, Any]] = None
    ) -> AskResponse:
        """
        Search the knowledgebase.

        Args:
            query: Search query
            topic: Optional topic filter
            limit: Max results
            mode: Search mode ("hybrid", "vector", "graph") - only for RAG-Anything
            multimodal_content: Optional multimodal content (equations, tables, images)
        """
        self._ensure_initialized()

        try:
            # 🤓 Use RAG-Anything for advanced multimodal queries
            if self.use_rag_anything and self.rag_integration:
                if multimodal_content or mode != "hybrid":
                    results = await self.rag_integration.query(
                        query=query,
                        mode=mode,
                        limit=limit,
                        multimodal_content=multimodal_content
                    )
                    # Convert RAG-Anything results to our format
                    results_dicts = results
                else:
                    # Use Qdrant for standard vector search
                    results = await self.rag_integration.search_qdrant(
                        query=query,
                        limit=limit,
                        filter_dict={"topic": topic} if topic else None
                    )
                    results_dicts = results
            elif RUST_MODULE_AVAILABLE:
                results_json = self.client.ask(query, topic)
                results_dicts = [json.loads(r) for r in results_json]
            else:
                results_dicts = await self.client.ask(query, topic)

            chunks = []
            for result_dict in results_dicts[:limit]:
                # 🤓 Handle different result formats
                metadata = result_dict.get("metadata", {})
                if isinstance(metadata, dict):
                    result_topic = metadata.get("topic", topic or "general")
                    tags = metadata.get("tags", [])
                else:
                    result_topic = topic or "general"
                    tags = []

                attribution = result_dict.get("attribution", {})
                chunk = ChunkData(
                    id=str(result_dict["id"]),
                    content=result_dict["content"],
                    datum=result_dict.get("datum", result_topic),
                    topic=result_topic,
                    tags=tags,
                    attribution_url=attribution.get("url"),
                    attribution_filename=attribution.get("filename"),
                    created_at=attribution.get("date", datetime.now().isoformat()),
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
        """Learn from content, creating multiple chunks. Automatically detects and crawls URLs."""
        self._ensure_initialized()

        try:
            # 🤓 Detect if content/source is a URL and crawl it
            url_to_crawl = None
            if source and self._is_url(source):
                url_to_crawl = source
            elif self._is_url(content):
                url_to_crawl = content

            if url_to_crawl:
                if not CRAWL4AI_AVAILABLE:
                    return LearnResponse(
                        success=False,
                        message="crawl4ai not available - cannot crawl URLs",
                        chunks=[],
                        source=url_to_crawl,
                        chunks_created=0
                    )

                logging.info(f"Detected URL, crawling: {url_to_crawl}")
                crawled_content = await self._crawl_url(url_to_crawl)
                if not crawled_content:
                    return LearnResponse(
                        success=False,
                        message=f"Failed to crawl URL: {url_to_crawl}",
                        chunks=[],
                        source=url_to_crawl,
                        chunks_created=0
                    )

                # Replace content with crawled content, keep URL as source
                content = crawled_content
                source = url_to_crawl

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
    
    async def process_multimodal_document(
        self,
        file_path: str,
        parse_method: str = "auto"
    ) -> Dict[str, Any]:
        """
        Process a multimodal document (PDF, images, Office docs, etc.).

        Args:
            file_path: Path to document
            parse_method: "auto", "ocr", or "txt"

        Returns:
            Processing results with extracted content
        """
        self._ensure_initialized()

        if not self.use_rag_anything or not self.rag_integration:
            return {
                "success": False,
                "message": "RAG-Anything not enabled for multimodal processing"
            }

        try:
            result = await self.rag_integration.process_document(
                file_path=file_path,
                parse_method=parse_method
            )
            return {"success": True, "result": result}
        except Exception as e:
            logging.error(f"Error processing multimodal document: {e}")
            return {"success": False, "message": str(e)}

    def get_status(self) -> Dict[str, Any]:
        """Get current status."""
        uptime = (datetime.now() - self._start_time).total_seconds()

        status = {
            "status": "ok" if self._initialized else "initializing",
            "version": "0.1.0",
            "qdrant_connected": self._initialized and self.client is not None,
            "embedding_model_loaded": self._initialized,
            "uptime_seconds": uptime,
            "rust_module_available": RUST_MODULE_AVAILABLE,
            "rag_anything_enabled": self.use_rag_anything
        }

        # Add RAG-Anything status if available
        if self.rag_integration:
            status["rag_anything_status"] = self.rag_integration.get_status()

        return status

    def _is_url(self, text: str) -> bool:
        """Check if text is a valid URL."""
        try:
            result = urlparse(text)
            return all([result.scheme, result.netloc])
        except Exception:
            return False

    async def _crawl_url(self, url: str) -> Optional[str]:
        """
        Crawl URL using crawl4ai and extract clean markdown content.

        Args:
            url: URL to crawl

        Returns:
            Extracted markdown content or None if failed
        """
        if not CRAWL4AI_AVAILABLE:
            logging.error("crawl4ai not available")
            return None

        try:
            async with AsyncWebCrawler(verbose=False) as crawler:
                result = await crawler.arun(url=url)

                if not result.success:
                    logging.error(f"Failed to crawl {url}: {result.error_message}")
                    return None

                # 🤓 crawl4ai provides cleaned markdown by default
                content = result.markdown

                if not content or len(content.strip()) < 50:
                    logging.warning(f"Crawled content too short or empty for {url}")
                    return None

                logging.info(f"Successfully crawled {url}: {len(content)} chars")
                return content

        except Exception as e:
            logging.error(f"Error crawling {url}: {e}")
            return None


class MockGrokClient:
    """Mock implementation for development when Rust module is not available."""

    def __init__(self, rag_integration: Optional[Any] = None):
        self.rag_integration = rag_integration
        self.embedding_model = None

        # 🤓 Prefer RAG-Anything embeddings if available
        if rag_integration:
            logging.info("MockGrokClient using RAG-Anything integration")
        elif EMBEDDING_MODEL_AVAILABLE:
            # Use instructor-large model for better embeddings
            self.embedding_model = INSTRUCTOR('hkunlp/instructor-large')
            logging.info("Loaded InstructorEmbedding model: hkunlp/instructor-large")
    
    def _generate_embedding(self, text: str, instruction: str = "Represent this text for retrieval:") -> List[float]:
        """Generate embedding for text using RAG-Anything, Instructor model, or mock."""
        # 🤓 Use RAG-Anything embeddings if available
        if self.rag_integration:
            try:
                return self.rag_integration.get_embedding(text)
            except Exception as e:
                logging.error(f"RAG-Anything embedding failed: {e}")

        # Fallback to Instructor embeddings
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