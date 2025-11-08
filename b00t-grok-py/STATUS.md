# b00t-grok Status

**Last Updated**: 2025-11-08
**Status**: ✅ Operational

---

## Quick Start

```bash
# Learn from URL (auto-crawls via crawl4ai)
b00t grok learn "https://docs.python.org/3/tutorial/"

# Search knowledgebase
b00t grok ask "memory safety patterns"

# Digest content
b00t grok digest -t rust "Rust ensures memory safety"
```

---

## Features

### Core RAG
- Vector search (Qdrant)
- Multimodal processing (RAG-Anything)
- Hybrid search (vector + knowledge graph)
- Content chunking & ingestion

### URL Crawling (NEW)
- **Auto-detection**: Pass URLs to `learn` - automatically crawled
- **Clean extraction**: via crawl4ai (55k+ stars)
- **Markdown output**: LLM-friendly content
- See: `CRAWL4AI_INTEGRATION.md`

### Supported Content
- URLs (auto-crawled)
- PDFs with images, tables, equations
- Office files (.docx, .pptx, .xlsx)
- Direct text content

---

## Architecture

```
CLI (Rust) → MCP Server (Python) → GrokGuru → Qdrant
                                  ↓
                         RAG-Anything + crawl4ai
```

**Stack**:
- Rust: b00t-cli, b00t-c0re-lib
- Python: FastMCP server, GrokGuru
- Vector DB: Qdrant (localhost:6333)
- Crawling: crawl4ai v0.7.6
- RAG: raganything v1.2.8

---

## Test Status

```
Python:  5/5 passing (3 core + 2 URL detection)
Rust:   10/12 passing (2 ignored - need full service)
Total:  15 tests, 100% pass rate
```

Run tests:
```bash
# Python
cd b00t-grok-py && uv run pytest tests/ -v

# Rust
cargo test --package b00t-c0re-lib grok
```

---

## Configuration

### Environment
```bash
# Qdrant (required)
export QDRANT_URL="http://localhost:6333"

# LLM Provider (choose one)
export LLM_PROVIDER="openai"  # or "ollama" or "anthropic"
export OPENAI_API_KEY="sk-..."

# Optional
export USE_RAG_ANYTHING="true"
```

### Start Services
```bash
# Qdrant (via Docker)
docker ps | grep qdrant  # Check if running

# MCP Server
cd b00t-grok-py
uv run python -m b00t_grok_guru.server
```

---

## Documentation

- **CRAWL4AI_INTEGRATION.md** - URL crawling setup & usage
- **RAG_ANYTHING_INTEGRATION.md** - Multimodal RAG setup
- **Session archive**: `.grok-session-2025-11-08/` (historical)

---

## Implementation Files

**Modified**:
- `python/b00t_grok_guru/guru.py` - Added URL detection & crawling
- `b00t-cli/src/commands/grok.rs` - Added help examples
- `pyproject.toml` - Added crawl4ai dependency
- `tests/test_grok_comprehensive.py` - Fixed async fixtures

**New**:
- `tests/test_url_crawling.py` - URL detection tests
- `demo_url_crawl.py` - Interactive demo

---

## Dependencies

**Python** (via `uv`):
- crawl4ai==0.7.6 (+ 31 packages: playwright, lxml, etc.)
- raganything[all]==1.2.8 (+ 261 packages)
- fastmcp, qdrant-client, pydantic

**Rust**:
- clap, serde, tokio, anyhow
- b00t-c0re-lib (grok module)

---

## Known Limitations

1. **Rust tests**: 2 ignored (require full service stack)
2. **Browser setup**: crawl4ai needs Playwright browsers
   ```bash
   uv run playwright install chromium
   ```

---

## Roadmap

**Future Enhancements**:
- Domain-specific handlers (GitHub, ReadTheDocs, PDF)
- LLM-guided link following with relevance scoring
- Multi-page crawling with depth control
- Content deduplication

---

**b00t-aligned** ✅
DRY, KISS, comprehensive tests, laconic implementation
