"""FastAPI server with MCP integration for b00t grok guru."""

import os
import logging
from typing import Dict, Any, List
from contextlib import asynccontextmanager

from fastapi import FastAPI, HTTPException, BackgroundTasks
from fastapi.middleware.cors import CORSMiddleware
from fastmcp import FastMCP

from .guru import GrokGuru
from .config import GrokConfig
from .types import (
    DigestRequest, DigestResponse,
    AskRequest, AskResponse,
    LearnRequest, LearnResponse,
    StatusResponse
)


# Global guru instance
guru: GrokGuru = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Manage application startup and shutdown."""
    global guru

    # Startup
    # 🤓 Load configuration from environment
    config = GrokConfig()

    # Initialize guru with model functions from config
    guru = GrokGuru(
        qdrant_url=config.qdrant_url,
        api_key=config.qdrant_api_key,
        use_rag_anything=config.use_rag_anything,
        llm_model_func=config.get_llm_func(),
        vision_model_func=config.get_vision_func(),
        embedding_func=config.get_embedding_func()
    )
    await guru.initialize()

    logging.info("b00t grok guru server started with RAG-Anything integration")
    yield

    # Shutdown
    logging.info("b00t grok guru server shutting down")


# Create FastAPI app
app = FastAPI(
    title="b00t grok guru",
    description="RAG knowledgebase server with MCP integration",
    version="0.1.0",
    lifespan=lifespan
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],  # Configure as needed
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Create MCP server
mcp = FastMCP("b00t-grok-guru")


# Health/status endpoint
@app.get("/", response_model=StatusResponse)
@app.get("/health", response_model=StatusResponse)
async def health():
    """Health check endpoint."""
    if guru is None:
        raise HTTPException(status_code=503, detail="Guru not initialized")
    
    status = guru.get_status()
    return StatusResponse(**status)


# REST API endpoints
@app.post("/digest", response_model=DigestResponse)
async def digest_endpoint(request: DigestRequest):
    """Digest content into a knowledge chunk."""
    if guru is None:
        raise HTTPException(status_code=503, detail="Guru not initialized")
    
    result = await guru.digest(request.topic, request.content)
    if not result.success:
        raise HTTPException(status_code=500, detail=result.message)
    
    return result


@app.post("/ask", response_model=AskResponse)
async def ask_endpoint(request: AskRequest):
    """Search the knowledgebase."""
    if guru is None:
        raise HTTPException(status_code=503, detail="Guru not initialized")
    
    result = await guru.ask(request.query, request.topic, request.limit)
    if not result.success:
        raise HTTPException(status_code=500, detail=result.message)
    
    return result


@app.post("/learn", response_model=LearnResponse)
async def learn_endpoint(request: LearnRequest):
    """Learn from content, creating multiple chunks."""
    if guru is None:
        raise HTTPException(status_code=503, detail="Guru not initialized")
    
    result = await guru.learn(request.content, request.source)
    if not result.success:
        raise HTTPException(status_code=500, detail=result.message)
    
    return result


# MCP Tool Definitions
@mcp.tool()
async def grok_digest(topic: str, content: str) -> Dict[str, Any]:
    """
    Digest content into a knowledge chunk about a specific topic.
    
    Args:
        topic: Topic to categorize the content under
        content: Text content to digest and store
        
    Returns:
        Dictionary containing the created chunk information
    """
    if guru is None:
        raise ValueError("Guru not initialized")
    
    result = await guru.digest(topic, content)
    if not result.success:
        raise ValueError(f"Digest failed: {result.message}")
    
    return {
        "success": True,
        "chunk_id": result.chunk.id,
        "topic": result.chunk.topic,
        "content_preview": result.chunk.content[:100] + ("..." if len(result.chunk.content) > 100 else ""),
        "created_at": result.chunk.created_at
    }


@mcp.tool()
async def grok_ask(query: str, topic: str = None, limit: int = 5) -> Dict[str, Any]:
    """
    Search the knowledgebase for information related to a query.
    
    Args:
        query: Search query or question
        topic: Optional topic filter to narrow results
        limit: Maximum number of results to return (default: 5)
        
    Returns:
        Dictionary containing search results and metadata
    """
    if guru is None:
        raise ValueError("Guru not initialized")
    
    result = await guru.ask(query, topic, limit)
    if not result.success:
        raise ValueError(f"Ask failed: {result.message}")
    
    # Format results for MCP response
    formatted_results = []
    for chunk in result.results:
        formatted_results.append({
            "id": chunk.id,
            "content": chunk.content,
            "topic": chunk.topic,
            "tags": chunk.tags,
            "source": chunk.attribution_url or chunk.attribution_filename,
            "created_at": chunk.created_at
        })
    
    return {
        "success": True,
        "query": result.query,
        "total_found": result.total_found,
        "results": formatted_results
    }


@mcp.tool()
async def grok_learn(content: str, source: str = None) -> Dict[str, Any]:
    """
    Learn from content by breaking it into chunks and storing in knowledgebase.
    
    Args:
        content: Text content to learn from
        source: Optional source identifier (URL or filename)
        
    Returns:
        Dictionary containing information about created chunks
    """
    if guru is None:
        raise ValueError("Guru not initialized")
    
    result = await guru.learn(content, source)
    if not result.success:
        raise ValueError(f"Learn failed: {result.message}")
    
    # Format chunk summaries
    chunk_summaries = []
    for chunk in result.chunks:
        chunk_summaries.append({
            "id": chunk.id,
            "topic": chunk.topic,
            "content_preview": chunk.content[:100] + ("..." if len(chunk.content) > 100 else ""),
            "tags": chunk.tags
        })
    
    return {
        "success": True,
        "source": result.source,
        "chunks_created": result.chunks_created,
        "chunk_summaries": chunk_summaries
    }


@mcp.tool()
async def grok_status() -> Dict[str, Any]:
    """
    Get the current status of the grok system.

    Returns:
        Dictionary containing system status information
    """
    if guru is None:
        return {"status": "error", "message": "Guru not initialized"}

    return guru.get_status()


@mcp.tool()
async def grok_process_multimodal_document(
    file_path: str,
    parse_method: str = "auto"
) -> Dict[str, Any]:
    """
    Process a multimodal document (PDF, images, Office docs, etc.) using RAG-Anything.

    Extracts text, images, tables, and equations from documents and stores them
    in the knowledge graph.

    Args:
        file_path: Absolute path to the document file
        parse_method: Processing method - "auto" (smart detection), "ocr" (force OCR), or "txt" (text only)

    Returns:
        Dictionary containing processing results with extracted content
    """
    if guru is None:
        raise ValueError("Guru not initialized")

    result = await guru.process_multimodal_document(file_path, parse_method)
    return result


@mcp.tool()
async def grok_ask_multimodal(
    query: str,
    topic: str = None,
    limit: int = 5,
    mode: str = "hybrid",
    include_images: bool = False,
    include_tables: bool = False,
    include_equations: bool = False
) -> Dict[str, Any]:
    """
    Search the knowledgebase with multimodal capabilities.

    Uses RAG-Anything's hybrid search combining vector similarity and knowledge graph
    traversal. Can optionally filter for specific content types.

    Args:
        query: Search query or question
        topic: Optional topic filter to narrow results
        limit: Maximum number of results to return (default: 5)
        mode: Search mode - "hybrid" (vector+graph), "vector" (similarity only), or "graph" (graph only)
        include_images: Include results with image content
        include_tables: Include results with table content
        include_equations: Include results with equation content

    Returns:
        Dictionary containing search results and metadata
    """
    if guru is None:
        raise ValueError("Guru not initialized")

    # Build multimodal content filter
    multimodal_content = None
    if include_images or include_tables or include_equations:
        multimodal_content = {
            "images": include_images,
            "tables": include_tables,
            "equations": include_equations
        }

    result = await guru.ask(
        query=query,
        topic=topic,
        limit=limit,
        mode=mode,
        multimodal_content=multimodal_content
    )

    if not result.success:
        raise ValueError(f"Multimodal query failed: {result.message}")

    # Format results for MCP response
    formatted_results = []
    for chunk in result.results:
        formatted_results.append({
            "id": chunk.id,
            "content": chunk.content,
            "topic": chunk.topic,
            "tags": chunk.tags,
            "source": chunk.attribution_url or chunk.attribution_filename,
            "created_at": chunk.created_at
        })

    return {
        "success": True,
        "query": result.query,
        "mode": mode,
        "total_found": result.total_found,
        "results": formatted_results
    }


# Create combined server with both FastAPI and MCP
def create_guru_server():
    """Create the combined FastAPI + MCP server."""
    return mcp.wrap(app)


if __name__ == "__main__":
    import uvicorn
    
    # Setup logging
    logging.basicConfig(level=logging.INFO)
    
    # Run the server
    uvicorn.run(
        "b00t_grok_guru.server:app",
        host="0.0.0.0",
        port=8000,
        reload=True
    )