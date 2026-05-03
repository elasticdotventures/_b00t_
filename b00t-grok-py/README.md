# b00t-grok-py

Python FastAPI server with MCP integration for b00t grok RAG knowledgebase system.

## Overview

b00t-grok-py provides a high-level Python interface to the b00t-grok Rust core, exposing RAG (Retrieval Augmented Generation) capabilities through both REST API and MCP (Model Context Protocol) interfaces.

## Architecture

```
AI Assistant (Claude, etc)
    ↓ MCP Protocol
b00t-grok-py (FastAPI Server)
    ↓ PyO3 bindings  
b00t-grok (Rust Core)
    ↓ Vector ops
Qdrant Vector Database
```

## Features

- **FastAPI REST API** for direct HTTP access
- **MCP Tools** for AI assistant integration
- **PyO3 bindings** to high-performance Rust core
- **Vector search** via Qdrant
- **Content chunking** and topic inference
- **Mock implementations** for development

## Installation

```bash
# Install from source
pip install -e .

# Or with development dependencies
pip install -e ".[dev]"
```

## Usage

### As a Server

```bash
# Start FastAPI server
python -m b00t_grok_guru.server

# Or with uvicorn
uvicorn b00t_grok_guru.server:app --reload
```

### As MCP Server

```bash
# Via fastmcp
python -m fastmcp run b00t_grok_guru.server:mcp

# Or use the combined server
python -m b00t_grok_guru.server:create_guru_server
```

### Environment Variables

```bash
export QDRANT_URL="https://your-qdrant-instance.com:6333"
export QDRANT_API_KEY="your-api-key"
```

## API Endpoints

### REST API

- `GET /health` - Health check
- `POST /digest` - Digest content into chunks
- `POST /ask` - Search knowledgebase  
- `POST /learn` - Learn from content

### MCP Tools

- `grok_digest(topic, content)` - Create knowledge chunk
- `grok_ask(query, topic?, limit?)` - Search knowledgebase
- `grok_learn(content, source?)` - Learn from content
- `grok_status()` - Get system status

## Example Usage

### Python API

```python
from b00t_grok_guru import GrokGuru

guru = GrokGuru()
await guru.initialize()

# Digest content
result = await guru.digest("rust", "Rust is memory safe")
print(f"Created chunk: {result.chunk.id}")

# Search
results = await guru.ask("memory safety")
for chunk in results.results:
    print(f"Found: {chunk.content}")

# Learn from content
learned = await guru.learn("""
    Chapter 1: Introduction to Rust
    
    Rust is a systems programming language.
    
    Chapter 2: Memory Safety
    
    Rust prevents common bugs.
""", source="rust-book.md")
print(f"Created {learned.chunks_created} chunks")
```

### MCP Integration

Configure in your AI assistant's MCP settings:

```json
{
  "mcpServers": {
    "b00t-grok": {
      "command": "python",
      "args": ["-m", "b00t_grok_guru.server"],
      "env": {
        "QDRANT_URL": "https://your-qdrant.com:6333",
        "QDRANT_API_KEY": "your-key"
      }
    }
  }
}
```

Then in your AI session:

```
Use grok_digest to store this information about Docker:
"Docker containers provide lightweight virtualization..."

Use grok_ask to search for information about "containerization"

Use grok_learn to process this documentation file...
```

## Development

```bash
# Install development dependencies
pip install -e ".[dev]"

# Run tests
pytest

# Format code
black .
isort .

# Type checking
mypy .
```

## Integration with b00t-mcp

The guru server can be registered with b00t-mcp for easy deployment:

```bash
# Add to b00t-mcp registry
b00t-mcp add '{
  "name": "grok-guru",
  "command": "python",
  "args": ["-m", "b00t_grok_guru.server"],
  "capabilities": ["rag", "knowledgebase", "vector-search"],
  "description": "b00t grok RAG knowledgebase with vector search"
}'

# Install to Claude Code
b00t-mcp install grok-guru claudecode
```

## Status

- 🟢 **Core API**: Implemented with mock backend
- 🟡 **PyO3 Integration**: Partial, depends on Rust module
- 🟡 **Vector Search**: Planned for Qdrant integration
- 🟢 **MCP Protocol**: Fully implemented via fastmcp
- 🟡 **Content Chunking**: Basic implementation

## License

MIT License - see LICENSE file for details.