# b00t Orchestrator - Debugging Notes

**Date:** 2025-11-10
**Status:** Implementation Complete, Runtime Testing Phase

---

## Root Cause Analysis: Why `b00t grok learn` Was Failing

### Issue #1: Missing OLLAMA_API_URL
**Error:**
```
RuntimeError: Environment variable 'OLLAMA_API_URL' not set or invalid
ERROR:    Application startup failed. Exiting.
```

**Root Cause:**
The b00t-grok Rust module requires `OLLAMA_API_URL` for embeddings, but:
1. Ollama was not installed/running
2. No orchestration datum existed for Ollama
3. grok-guru.mcp didn't declare Ollama as a dependency

**Resolution:**
- Created `ollama.docker.toml` datum with proper Docker configuration
- Added `ollama.docker` to `grok-guru.mcp.toml` dependencies
- Updated `grok.stack.toml` to include ollama

### Issue #2: Qdrant Connection Refused
**Error:**
```
WARNING:root:RAG-Anything initialization failed: [Errno 111] Connection refused
```

**Root Cause:**
Qdrant was stopped during testing but orchestrator wasn't starting it.

**Resolution:**
Orchestrator correctly configured to start qdrant.docker as dependency.

---

## Architecture Corrections: DRY Environment Variables

### Problem
Environment variables were duplicated across:
- Individual datums
- Stack definitions
- Application code

This violated DRY principle - changing a service URL required updates in multiple places.

### Solution: Datum as Source of Truth

**Before (Wrong):**
```toml
# grok.stack.toml
[b00t.env]
QDRANT_URL = "http://localhost:6333"       # ❌ Duplicated
OLLAMA_API_URL = "http://localhost:11434"  # ❌ Duplicated
```

**After (Correct):**
```toml
# ollama.docker.toml - SOURCE OF TRUTH
[b00t.env]
OLLAMA_API_URL = "http://localhost:11434"

# qdrant.docker.toml - SOURCE OF TRUTH
[b00t.env]
QDRANT_URL = "http://localhost:6333"

# grok.stack.toml - PASSES THROUGH
[b00t.env]
OLLAMA_API_URL = "${OLLAMA_API_URL}"  # ✅ Pass through from ollama.docker
QDRANT_URL = "${QDRANT_URL}"          # ✅ Pass through from qdrant.docker

# grok-guru.mcp.toml - INHERITS
[b00t]
depends_on = ["qdrant.docker", "ollama.docker"]
# ✅ Automatically inherits environment from dependencies
```

**Benefits:**
1. **DRY:** Each URL defined once in its datum
2. **Flexible:** Datum can point to remote server without changing stack
3. **Composable:** `${ENV}` syntax allows variable mapping if needed
4. **Maintainable:** Update URL in one place, propagates everywhere

---

## Orchestrator Implementation Status

### ✅ Completed
- `orchestrator.rs` (200 lines) - Core module
- Docker/Podman detection and startup
- Dependency resolution from `depends_on` field
- Silent operation with `B00T_DEBUG` mode
- Idempotent service management
- Container readiness polling
- Integration with grok command

### ⏳ Testing In Progress
- Cold start: Services not running → orchestrator starts them
- Warm start: Services already running → orchestrator skips
- Debug mode: `B00T_DEBUG=1` shows orchestration actions
- Dependency chain: qdrant + ollama → grok-guru → grok command

---

## Embedding Model Requirements

The grok system requires an **OpenAI-compatible embedding API**:

### Supported Providers
1. **Ollama** (local, preferred)
   - Datum: `ollama.docker.toml`
   - Model: `nomic-embed-text`
   - Endpoint: `http://localhost:11434`

2. **vLLM** (not yet implemented)
   - Would need `vllm.docker.toml`

3. **LiteLLM** (not yet implemented)
   - Would need `litellm.docker.toml`

4. **OpenAI** (remote, requires API key)
   - Config via `OPENAI_API_KEY`

### Current Configuration
```python
# b00t-grok-py/python/b00t_grok_guru/config.py
self.embedding_provider = os.getenv("EMBEDDING_PROVIDER", "openai")  # Default
self.ollama_embedding_model = os.getenv("OLLAMA_EMBEDDING_MODEL", "nomic-embed-text")
```

---

## Next Steps

### Immediate
1. Pull Ollama Docker image: `docker pull ollama/ollama:latest`
2. Test orchestrator cold start
3. Verify both Qdrant and Ollama start automatically
4. Test grok learn command end-to-end

### Short Term
- Add vLLM and LiteLLM datum support
- Implement `${ENV}` variable substitution in stack loader
- Add health checks for service readiness
- Test with huggingface_cli model downloads

### Documentation
- Update ORCHESTRATOR_DESIGN.md with environment variable patterns
- Document embedding model requirements
- Create troubleshooting guide

---

## Commands for Testing

```bash
# Stop all services for cold start test
docker stop qdrant ollama 2>/dev/null

# Test orchestrator with debug
B00T_DEBUG=1 b00t grok learn "test content" -t test

# Expected output:
# 🚀 Started dependencies: qdrant.docker, ollama.docker
# 📚 Learning from source...
# ✅ Success

# Verify services running
docker ps | grep -E "(qdrant|ollama)"

# Warm start test (services already running)
b00t grok learn "test 2" -t test
# Expected: No "Started dependencies" message, immediate execution
```

---

## Lessons Learned

1. **Environment variables must be DRY** - Datum is source of truth
2. **Dependencies must be complete** - Missing Ollama caused silent failure
3. **Orchestrator architecture is sound** - Just needed proper datums configured
4. **Error messages were clear** - "OLLAMA_API_URL not set" pointed directly to issue

---

**Status:** Ready for integration testing with Ollama Docker image
