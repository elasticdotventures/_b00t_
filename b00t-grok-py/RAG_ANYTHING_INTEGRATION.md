# RAG-Anything Integration for b00t-grok

## Overview

b00t-grok now integrates **RAG-Anything** (https://github.com/HKUDS/RAG-Anything), a multimodal RAG system that provides:

- **Multimodal Document Processing**: Extract text, images, tables, and equations from PDFs, Office documents, and images
- **Advanced Embeddings**: Configurable embedding models (OpenAI, Ollama, custom)
- **Knowledge Graph Construction**: Build semantic relationships between content
- **Hybrid Search**: Combine vector similarity with graph traversal for better retrieval
- **Flexible Model Support**: Works with OpenAI, Ollama, Anthropic, and custom models

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     b00t-grok-guru                          │
│                                                             │
│  ┌─────────────┐      ┌──────────────┐      ┌───────────┐ │
│  │   MCP Tools │ ──── │  GrokGuru    │ ──── │  Qdrant   │ │
│  │   (FastAPI) │      │   (Python)   │      │  (Vector  │ │
│  └─────────────┘      └──────────────┘      │   Store)  │ │
│                              │               └───────────┘ │
│                              │                             │
│                      ┌───────▼──────────┐                  │
│                      │  RAG-Anything    │                  │
│                      │  Integration     │                  │
│                      └───────┬──────────┘                  │
│                              │                             │
│         ┌────────────────────┼────────────────────┐        │
│         │                    │                    │        │
│    ┌────▼────┐        ┌──────▼──────┐      ┌─────▼────┐   │
│    │  LLM    │        │  Vision     │      │ Embedding│   │
│    │  Model  │        │  Model      │      │  Model   │   │
│    │ (GPT-4) │        │ (GPT-4V)    │      │ (OpenAI) │   │
│    └─────────┘        └─────────────┘      └──────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Installation

```bash
# Install RAG-Anything with all dependencies
pip install 'raganything[all]'

# Or install from b00t-grok-py requirements
cd /home/brianh/.dotfiles/b00t-grok-py
pip install -e .
```

## Configuration

Configure via environment variables:

```bash
# Qdrant Configuration
export QDRANT_URL="http://localhost:6333"
export QDRANT_API_KEY=""  # Optional for local instance

# RAG-Anything Settings
export USE_RAG_ANYTHING="true"
export RAG_WORKING_DIR="/tmp/b00t_grok_rag"

# Model Provider Selection
export LLM_PROVIDER="openai"          # openai, ollama, anthropic
export VISION_PROVIDER="openai"       # openai, ollama
export EMBEDDING_PROVIDER="openai"    # openai, ollama

# OpenAI Configuration
export OPENAI_API_KEY="sk-..."
export OPENAI_MODEL="gpt-4o-mini"
export OPENAI_VISION_MODEL="gpt-4o-mini"
export OPENAI_EMBEDDING_MODEL="text-embedding-3-small"

# Ollama Configuration (for local models)
export OLLAMA_URL="http://localhost:11434"
export OLLAMA_MODEL="llama3.2"
export OLLAMA_VISION_MODEL="llava"
export OLLAMA_EMBEDDING_MODEL="nomic-embed-text"

# Anthropic Configuration
export ANTHROPIC_API_KEY="sk-ant-..."
export ANTHROPIC_MODEL="claude-3-5-sonnet-20241022"
```

## Usage

### 1. Basic Text Processing

```python
from b00t_grok_guru import GrokGuru
from b00t_grok_guru.config import GrokConfig

# Initialize
config = GrokConfig()
guru = GrokGuru(
    qdrant_url=config.qdrant_url,
    use_rag_anything=True,
    llm_model_func=config.get_llm_func(),
    vision_model_func=config.get_vision_func(),
    embedding_func=config.get_embedding_func()
)
await guru.initialize()

# Digest content
result = await guru.digest(
    topic="machine-learning",
    content="Transformers are a type of neural network architecture..."
)

# Search with hybrid mode
results = await guru.ask(
    query="What are transformers?",
    mode="hybrid",  # Uses vector + graph search
    limit=5
)
```

### 2. Multimodal Document Processing

```python
# Process a PDF with images, tables, equations
result = await guru.process_multimodal_document(
    file_path="/path/to/research_paper.pdf",
    parse_method="auto"  # auto, ocr, or txt
)

# Query with multimodal content
results = await guru.ask(
    query="Explain the equation in section 3",
    mode="hybrid",
    multimodal_content={
        "images": True,
        "tables": True,
        "equations": True
    }
)
```

### 3. Using MCP Tools

The following MCP tools are available:

#### `grok_digest`
Store a knowledge chunk with RAG-Anything embeddings.

#### `grok_ask`
Standard text-based search with optional hybrid mode.

#### `grok_ask_multimodal`
Enhanced search with multimodal content filtering.

```python
# Example MCP call
result = await grok_ask_multimodal(
    query="machine learning architectures",
    mode="hybrid",
    include_images=True,
    include_tables=True,
    limit=5
)
```

#### `grok_process_multimodal_document`
Process PDFs, images, and Office documents.

```python
result = await grok_process_multimodal_document(
    file_path="/path/to/document.pdf",
    parse_method="auto"
)
```

#### `grok_learn`
Break content into chunks and store in knowledge base.

#### `grok_status`
Get system status including RAG-Anything integration status.

## Features

### Multimodal Content Extraction

RAG-Anything can extract and process:

- **Text**: From PDFs, Markdown, HTML, Office documents
- **Images**: Diagrams, charts, photos with vision model analysis
- **Tables**: Structured data extraction and parsing
- **Equations**: Mathematical notation recognition

### Hybrid Search Modes

1. **hybrid** (default): Combines vector similarity with knowledge graph traversal
2. **vector**: Pure semantic similarity search
3. **graph**: Graph-based relationship traversal

### Model Flexibility

Supports multiple providers:

- **OpenAI**: GPT-4, GPT-4o-mini, text-embedding-3-small
- **Ollama**: Local models (llama3.2, llava, nomic-embed-text)
- **Anthropic**: Claude models for LLM tasks
- **Custom**: Implement your own model functions

## Testing

Run the integration test:

```bash
cd /home/brianh/.dotfiles/b00t-grok-py
python test_rag_anything_integration.py
```

This will test:
- Initialization with RAG-Anything
- Digest with custom embeddings
- Learn (multi-chunk processing)
- Search with hybrid mode
- System status

## File Structure

```
b00t-grok-py/
├── python/
│   └── b00t_grok_guru/
│       ├── __init__.py
│       ├── guru.py                        # Main interface (updated)
│       ├── rag_anything_integration.py    # NEW: RAG-Anything wrapper
│       ├── config.py                      # NEW: Model configuration
│       ├── server.py                      # Updated with multimodal tools
│       └── types.py
├── pyproject.toml                         # Updated dependencies
├── test_rag_anything_integration.py       # NEW: Integration tests
└── RAG_ANYTHING_INTEGRATION.md           # This file
```

## Performance Considerations

1. **Embedding Caching**: RAG-Anything caches embeddings for performance
2. **Batch Processing**: Use batch embedding functions for multiple texts
3. **Model Selection**:
   - Use smaller models (gpt-4o-mini) for faster responses
   - Use Ollama for local, free processing
   - Use larger models (gpt-4) for complex queries
4. **Vector Storage**: Qdrant provides efficient similarity search at scale

## Troubleshooting

### RAG-Anything Not Available

If you see "RAG-Anything not available", ensure:
```bash
pip install 'raganything[all]'
```

### LibreOffice Required

RAG-Anything needs LibreOffice for Office document processing:
```bash
# Ubuntu/Debian
sudo apt-get install libreoffice

# macOS
brew install libreoffice
```

### Ollama Connection Failed

Ensure Ollama is running:
```bash
ollama serve
# In another terminal
ollama pull llama3.2
ollama pull llava
ollama pull nomic-embed-text
```

### Mock Embeddings Warning

If you see "Using mock embeddings", configure a real provider:
```bash
export OPENAI_API_KEY="sk-..."
export EMBEDDING_PROVIDER="openai"
```

## Next Steps

1. **Add Custom Models**: Implement your own embedding/LLM functions in `config.py`
2. **Optimize Chunking**: Tune RAG-Anything chunking strategy for your documents
3. **Knowledge Graph**: Explore RAG-Anything's graph construction capabilities
4. **Production Deployment**: Set up proper model API keys and Qdrant instance

## References

- RAG-Anything: https://github.com/HKUDS/RAG-Anything
- Qdrant: https://qdrant.tech/
- b00t framework: https://github.com/elasticdotventures/dotfiles
