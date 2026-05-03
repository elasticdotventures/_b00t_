"""Type definitions for b00t grok guru."""

from typing import List, Optional, Dict, Any
from pydantic import BaseModel, Field
from datetime import datetime


class ChunkData(BaseModel):
    """Represents a knowledge chunk."""
    id: str
    content: str
    datum: str
    topic: str
    tags: List[str] = Field(default_factory=list)
    attribution_url: Optional[str] = None
    attribution_filename: Optional[str] = None
    created_at: str
    vector: Optional[List[float]] = None


class GrokRequest(BaseModel):
    """Base request for grok operations."""
    pass


class DigestRequest(GrokRequest):
    """Request to digest content into chunks."""
    topic: str = Field(..., description="Topic to digest content about")
    content: str = Field(..., description="Content to digest")


class AskRequest(GrokRequest):
    """Request to search the knowledgebase."""
    query: str = Field(..., description="Query to search for")
    topic: Optional[str] = Field(None, description="Optional topic filter")
    limit: int = Field(5, description="Maximum number of results")


class LearnRequest(GrokRequest):
    """Request to learn from URLs or content."""
    content: str = Field(..., description="Content to learn from")
    source: Optional[str] = Field(None, description="Source URL or filename")


class GrokResponse(BaseModel):
    """Base response for grok operations."""
    success: bool = True
    message: Optional[str] = None


class DigestResponse(GrokResponse):
    """Response from digest operation."""
    chunk: ChunkData


class AskResponse(GrokResponse):
    """Response from ask operation."""
    results: List[ChunkData]
    query: str
    total_found: int


class LearnResponse(GrokResponse):
    """Response from learn operation."""
    chunks: List[ChunkData]
    source: Optional[str] = None
    chunks_created: int


class StatusResponse(BaseModel):
    """Health/status response."""
    status: str = "ok"
    version: str = "0.1.0"
    qdrant_connected: bool = False
    embedding_model_loaded: bool = False
    uptime_seconds: float = 0.0