"""
b00t grok guru - FastAPI MCP server for RAG knowledgebase

A high-level Python interface to the b00t-grok Rust core via PyO3.
Provides FastAPI server with MCP (Model Context Protocol) integration
for AI assistants to access personal knowledgebase capabilities.
"""

__version__ = "0.1.0"

from .guru import GrokGuru
from .server import app, create_guru_server
from .types import GrokRequest, GrokResponse, ChunkData

__all__ = [
    "GrokGuru", 
    "app", 
    "create_guru_server",
    "GrokRequest", 
    "GrokResponse", 
    "ChunkData"
]