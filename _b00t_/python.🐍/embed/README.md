# 🧠 embed_anything Integration for b00t Semantic Search

Rust-native embedding pipeline for semantic search across b00t datums, replacing deprecated RAG approaches.

## Architecture

```
┌─────────────────────────────────────────┐
│ embed_anything (Rust Core)             │
│ - Candle ML framework                   │
│ - Zero PyTorch dependency               │
│ - GPU acceleration (optional)           │
│ - Memory-efficient streaming            │
└─────────────────────────────────────────┘
           │
           ├─ PyO3 Bindings
           │
┌─────────────────────────────────────────┐
│ semantic_datum_search.py                │
│ - Load datums from ~/.b00t/_b00t_       │
│ - Generate embeddings with HF models    │
│ - Cosine similarity search              │
│ - Cache embeddings as JSON              │
└─────────────────────────────────────────┘
```

## Features

- **Multimodal embeddings**: Dense, sparse, late-interaction
- **Local inference**: No cloud dependencies
- **Low memory**: Streaming architecture
- **Fast**: Rust concurrency + true multithreading
- **Flexible models**: Any HuggingFace model via Candle

## Installation

```bash
# Install embed_anything
just install

# Or manually
uv pip install embed-anything-gpu  # With CUDA
uv pip install embed-anything      # CPU only
```

## Usage

### Quick Start

```bash
# Semantic search (generates embeddings on-the-fly)
just search "how to install kubernetes" 5

# Pre-compute embeddings for faster searches
just embed-all datum_embeddings.json

# Search with cached embeddings
just search "rust deployment tools" 3
```

### Python API

```python
from semantic_datum_search import DatumEmbedder

# Initialize
embedder = DatumEmbedder(
    b00t_path="~/.b00t/_b00t_",
    model_id="sentence-transformers/all-MiniLM-L6-v2"
)

# Load and embed datums
datums = embedder.load_datums()
embedded = embedder.embed_datums(datums)

# Semantic search
results = embedder.semantic_search(
    query="how to setup postgres",
    embedded_datums=embedded,
    top_k=5
)

for datum, score in results:
    print(f"{datum.name}: {score:.3f}")
```

### Advanced Models

```bash
# Use Jina embeddings for better semantic understanding
just search-model "container orchestration" "jinaai/jina-embeddings-v2-base-en" 5

# Sparse embeddings with Splade (hybrid search)
just search-model "AI deployment" "naver/splade-cocondenser-ensembledistil" 3
```

## Supported Models

| Model | Type | Context | Use Case |
|-------|------|---------|----------|
| `sentence-transformers/all-MiniLM-L6-v2` | Dense | 256 | Fast, general purpose |
| `sentence-transformers/all-MiniLM-L12-v2` | Dense | 512 | Better quality |
| `jinaai/jina-embeddings-v2-base-en` | Dense | 8192 | Long context |
| `naver/splade-cocondenser-ensembledistil` | Sparse | 512 | Hybrid search |

## Integration with b00t

### Datum Configuration

Created `~/.b00t/_b00t_/embed-anything.cli.toml`:
- Version detection
- GPU/CPU installation
- HuggingFace model caching
- Usage examples

### Workflow

1. **Index datums** (one-time or periodic):
   ```bash
   cd ~/.b00t && just embed-all
   ```

2. **Semantic search** when needed:
   ```bash
   just search "how do I debug kubernetes pods" 5
   ```

3. **Update index** when datums change:
   ```bash
   just clean && just embed-all
   ```

## Performance

- **Embedding generation**: ~10ms per datum (MiniLM-L6-v2, CPU)
- **Search**: <1ms with cached embeddings
- **Memory**: ~50MB model + embeddings

## Comparison to Previous Approach

| Feature | Previous (🌇 deprecated) | embed_anything |
|---------|-------------------------|----------------|
| Dependency | PyTorch | Candle (Rust) |
| Memory | ~2GB | ~50MB |
| Speed | Slow | Fast (Rust) |
| GPU Support | Limited | Full CUDA |
| Model Flexibility | Low | High (any HF) |
| Production Ready | No | Yes |

## Future Enhancements

- [ ] Vector DB integration (Qdrant, Milvus)
- [ ] Incremental re-indexing
- [ ] Multi-language support
- [ ] Cross-modal search (datum diagrams, screenshots)
- [ ] Reranking with late-interaction models

## Troubleshooting

**ImportError: embed_anything**
```bash
just install
```

**CUDA errors**
```bash
# Fallback to CPU version
pip uninstall embed-anything-gpu
pip install embed-anything
```

**Slow embedding generation**
- Use smaller models (MiniLM-L6-v2 vs L12-v2)
- Pre-compute embeddings with `just embed-all`
- Consider GPU acceleration

## References

- [embed_anything GitHub](https://github.com/StarlightSearch/EmbedAnything)
- [embed_anything docs](https://docs.rs/embed_anything)
- [HuggingFace models](https://huggingface.co/models?library=sentence-transformers)
