# TODO-next — session 2026-03-16

## Critical path (ordered)

### #259 — llama-server CUDA build ← BLOCKS EVERYTHING
```bash
# brew build has no CUDA; 0.22 tok/s on CPU, 24GB VRAM completely idle
# Fix: build llama.cpp from source with CUDA
cmake -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=86 ..  # sm86 = RTX 3090
make -j$(nproc) llama-server
# OR: podman GPU container (no build needed)
podman run --device nvidia.com/gpu=all --security-opt=label=disable \
  -p 8000:8000 ghcr.io/ggerganov/llama.cpp:server-cuda \
  -m /path/to/model.gguf --alias qwen3-coder -ngl 999 -c 32768
```
Update `_b00t_/inference-qwen3.stack.tomllm` exec_start + prereq CUDA check.

### #264 — opencode + local qwen3-coder (unblocks after #259)
```bash
opencode run --model vllm-local/qwen3-coder 'reply LOCAL_ONLINE only'
# opencode.json already configured correctly — just needs fast llama-server
```

### #260 — irontology-mcp binary (compile fix)
```bash
# main.rs has rmcp 0.8.5 import errors; mirror b00t-mcp/src/mcp_server_rusty.rs imports exactly
grep -n "use rmcp" /home/brianh/.b00t/b00t-mcp/src/mcp_server_rusty.rs
# Fix: RequestContext path, ToolDescription path, Implementation ..Default::default()
# Fix: use storage_neumann::config::NeumannConfig (not root re-export)
cargo build --release --manifest-path vendor/irontology-mcp/Cargo.toml
```

### #261 — wire grok to irontology (unblocks after #260)
```bash
# Edit b00t-c0re-lib/src/grok.rs
# Add GROK_BACKEND env var gate: "irontology" | "qdrant" | "auto"
# Route GrokClient methods to irontology MCP tool calls
# Qdrant stays as feature flag (not removed)
GROK_BACKEND=irontology b00t grok ask "what is b00t"
```

### #262 — NeumannStore persistence (unblocks after #260)
```bash
# Add sled = "0.34" to storage-neumann/Cargo.toml
# Replace in-memory HashMap with sled trees at ~/.b00t/neumann/{namespace}/
# TDD: persist → restart → data survives
```

### #263 — real SearchBackend embeddings (unblocks after #262)
```bash
ollama pull nomic-embed-text  # 384-dim, CPU-friendly
# Implement VectorBackend → ollama /api/embeddings
# OR: use llama-server /v1/embeddings (already running)
```

---

## Key facts learned this session

### Hardware envelope
- RTX 3090: 24GB VRAM | System RAM: 31GB | CPU: available for offload
- Qwen3-Coder-Next Q4_K_M GGUF: 48.4GB → needs -ngl max to use GPU
- ⚠️ brew llama.cpp = CPU-only build; VRAM completely idle right now
- safetensors BF16 = 160GB, FP8 = 80GB — both exceed hardware (GGUF is correct format)

### Model hosting decision
- llama-server (llama.cpp) natively supports Qwen3-Coder-Next GGUF ✅
- vLLM 0.17.1 BLOCKED on in_proj_qkvz.qweight mapping (SSM weight loader gap) — patching exceeded budget
- vLLM patches stashed: `scripts/venv-patches/qwen3-coder-next-gguf.sh`
- Candle: supports Qwen3 MoE but inference-only; no deployment/offload layer
- Unsloth: fine-tuning only, no GGUF→safetensors converter
- Qwen/Qwen3-Coder-Next safetensors exists on HF Hub but requires 160GB → won't fit

### irontology-mcp
- 7-crate Rust workspace; builds clean on Linux ✅
- Library-only; main.rs added in feat/elasticdotventures/_b00t_ but has rmcp import errors
- NeumannStore: in-memory RDF triple store + vector similarity (no persistence yet)
- SearchBackend: 4-way fusion (vector 0.35, graph 0.30, lexical 0.20, ontology 0.15) — stubs only
- KnowledgeStore trait is the right abstraction to replace Qdrant in grok
- Qdrant was semantic RAG only (vector), not a knowledge graph — NeumannStore adds RDF graph traversal

### b00t grok
- Broken: GrokClient hardcodes Qdrant at 192.168.2.13:6333 (unreachable)
- Python stack: GrokGuru → RAG-Anything → Qdrant (all working, just no Qdrant)
- irontology NeumannStore is the right replacement + upgrade
- GROK_BACKEND env var gate is the clean wiring approach

### Conventions established
- tomllm files: PascalCase (HuggingfaceMcp.mcp.tomllm, Vllm.InferenceProvider.tomllm, IrontologyMcp.mcp.tomllm)
- PromptExecution repos: push to feat/elasticdotventures/_b00t_ only, never main
- PromptExecution org: registered in _b00t_/PromptExecution.github.repo.tomllm

### New commands added
- `b00t exec` — audited broad-authority execution; Block TTL cache; --sleep=<duration>; JSONL audit log
- `b00t quit` — killswitch to SIGTERM upper agent (walk /proc tree → B00T_AGENT_PID → PPID)

### opencode
- v1.1.48 installed; opencode.json has vllm-local provider correctly configured
- vllm-local/qwen3-coder appears in `opencode models` ✅
- Blocked on llama-server CUDA (#259) — useless at 0.22 tok/s

---

## OOD (Out-of-date) items to update when returning

| Item | Action |
|------|--------|
| inference-qwen3.stack.tomllm | Update exec_start with CUDA llama-server binary path after #259 |
| IrontologyMcp.mcp.tomllm | Remove "no binary" warning after #260 |
| b00t-c0re-lib/src/grok.rs | Add GROK_BACKEND after #261 |
| MEMORY.md | Add note when grok is actually working |

## Issues filed this session
- #259: llama-server no CUDA (critical)
- #260: irontology-mcp compile fix
- #261: wire grok to irontology
- #262: NeumannStore persistence
- #263: real SearchBackend embeddings
- #264: opencode local model (unblocks after #259)
