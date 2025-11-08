"""Test script for RAG-Anything integration with b00t-grok."""

import asyncio
import logging
from pathlib import Path

from python.b00t_grok_guru.guru import GrokGuru
from python.b00t_grok_guru.config import GrokConfig

# Setup logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


async def test_basic_integration():
    """Test basic RAG-Anything integration."""
    logger.info("=" * 60)
    logger.info("Testing RAG-Anything Integration")
    logger.info("=" * 60)

    # Initialize config
    config = GrokConfig()
    logger.info(f"Configuration loaded:")
    logger.info(f"  Qdrant URL: {config.qdrant_url}")
    logger.info(f"  Use RAG-Anything: {config.use_rag_anything}")
    logger.info(f"  LLM Provider: {config.llm_provider}")
    logger.info(f"  Embedding Provider: {config.embedding_provider}")

    # Initialize guru
    guru = GrokGuru(
        qdrant_url=config.qdrant_url,
        api_key=config.qdrant_api_key,
        use_rag_anything=config.use_rag_anything,
        llm_model_func=config.get_llm_func(),
        vision_model_func=config.get_vision_func(),
        embedding_func=config.get_embedding_func()
    )

    await guru.initialize()
    logger.info("GrokGuru initialized successfully")

    # Check status
    status = guru.get_status()
    logger.info("\n" + "=" * 60)
    logger.info("System Status:")
    logger.info("=" * 60)
    for key, value in status.items():
        logger.info(f"  {key}: {value}")

    return guru


async def test_digest_with_rag_anything(guru: GrokGuru):
    """Test digest with RAG-Anything embeddings."""
    logger.info("\n" + "=" * 60)
    logger.info("Testing Digest with RAG-Anything")
    logger.info("=" * 60)

    # Create test content
    content = """
    RAG-Anything is a multimodal RAG system that can process:
    - Text documents (PDF, Markdown, HTML)
    - Images and diagrams
    - Tables and structured data
    - Mathematical equations

    It uses hybrid search combining vector similarity with knowledge graph traversal
    for more accurate and contextual retrieval.
    """

    result = await guru.digest(topic="rag-anything", content=content)

    if result.success:
        logger.info(f"✓ Digest successful!")
        logger.info(f"  Chunk ID: {result.chunk.id}")
        logger.info(f"  Topic: {result.chunk.topic}")
        logger.info(f"  Content length: {len(result.chunk.content)} chars")
        logger.info(f"  Vector dims: {len(result.chunk.vector) if result.chunk.vector else 'N/A'}")
    else:
        logger.error(f"✗ Digest failed: {result.message}")

    return result


async def test_search_with_rag_anything(guru: GrokGuru):
    """Test search with RAG-Anything."""
    logger.info("\n" + "=" * 60)
    logger.info("Testing Search with RAG-Anything")
    logger.info("=" * 60)

    # Test different search modes
    queries = [
        ("What can RAG-Anything process?", "hybrid"),
        ("multimodal RAG", "vector"),
    ]

    for query, mode in queries:
        logger.info(f"\nQuery: '{query}' (mode: {mode})")

        result = await guru.ask(
            query=query,
            limit=3,
            mode=mode
        )

        if result.success:
            logger.info(f"✓ Found {result.total_found} results")
            for i, chunk in enumerate(result.results, 1):
                logger.info(f"\n  Result {i}:")
                logger.info(f"    Topic: {chunk.topic}")
                logger.info(f"    Content preview: {chunk.content[:150]}...")
        else:
            logger.error(f"✗ Search failed: {result.message}")


async def test_learn_with_rag_anything(guru: GrokGuru):
    """Test learn (multi-chunk processing)."""
    logger.info("\n" + "=" * 60)
    logger.info("Testing Learn with RAG-Anything")
    logger.info("=" * 60)

    content = """
# b00t-grok Architecture

b00t-grok is a RAG knowledgebase system with three main components:

## 1. Rust Core
The Rust core provides high-performance vector operations and Qdrant integration.
It handles chunk management and efficient similarity search.

## 2. Python API
The Python layer provides a user-friendly interface through FastAPI and MCP tools.
It integrates with RAG-Anything for multimodal capabilities.

## 3. RAG-Anything Integration
RAG-Anything adds support for:
- Document parsing (PDF, Office, images)
- Multimodal content extraction
- Knowledge graph construction
- Hybrid retrieval (vector + graph)

## Benefits
- Fast retrieval with Rust
- Flexible API with Python
- Multimodal support with RAG-Anything
- Scalable storage with Qdrant
"""

    result = await guru.learn(
        content=content,
        source="b00t-grok-architecture.md"
    )

    if result.success:
        logger.info(f"✓ Learn successful!")
        logger.info(f"  Chunks created: {result.chunks_created}")
        logger.info(f"  Source: {result.source}")
        for i, chunk in enumerate(result.chunks, 1):
            logger.info(f"\n  Chunk {i}:")
            logger.info(f"    Topic: {chunk.topic}")
            logger.info(f"    Preview: {chunk.content[:100]}...")
    else:
        logger.error(f"✗ Learn failed: {result.message}")


async def main():
    """Run all tests."""
    try:
        # Initialize
        guru = await test_basic_integration()

        # Test digest
        await test_digest_with_rag_anything(guru)

        # Test learn
        await test_learn_with_rag_anything(guru)

        # Test search
        await test_search_with_rag_anything(guru)

        logger.info("\n" + "=" * 60)
        logger.info("All tests completed!")
        logger.info("=" * 60)

    except Exception as e:
        logger.error(f"Test failed with error: {e}", exc_info=True)


if __name__ == "__main__":
    asyncio.run(main())
