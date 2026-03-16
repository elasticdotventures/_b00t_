# Agent Handoff — 2026-03-16

> Next agent: read this BEFORE touching anything. All facts verified against live system.

---

## 1. WHO YOU ARE WORKING WITH

**Operator**: @Sir (@elasticdotventures), senior AI systems engineer, PromptExecution Pty Ltd.
- Pronouns: they/them
- Interface: BMI — typos expected, high signal-to-noise, no pleasantries
- Communication: laconic, RFC 2119 precision, direct technical — NEVER platitudes, NEVER apologize
- GitHub: `github.com/elasticdotventures`
- b00t is their creation; treat it as gospel

---

## 2. THE MACHINE (sm3llsl1k3s0ld3r)

| Resource | Value |
|----------|-------|
| GPU | RTX 3090, 24GB VRAM |
| RAM | 31.3GB |
| CPU | 4 cores |
| VRAM free NOW | 24119MB (24GB free — GPU idle) |
| RAM used NOW | ~5.2GB used, ~26GB free |
| OS | Linux 6.8.0-101-generic |
| Shell | bash + starship prompt |
| Python venv | `/home/brianh/.venv` (python 3.12) |
| Homebrew | `/home/linuxbrew/.linuxbrew` |

---

## 3. MANDATORY TOOL RULES (violations cause explicit Operator correction)

| Rule | Detail |
|------|--------|
| `uv pip` ALWAYS | NEVER `pip install` — b00t guard blocks it. Form: `uv pip install pkg --python /home/brianh/.venv/bin/python3` |
| b00t MCP over bash | Check b00t MCP tools BEFORE reaching for Bash for any hive operation |
| No raw systemctl | Use `b00t hive activate/stop` for service management |
| No raw docker | Use `podman` with `--device nvidia.com/gpu=all --security-opt=label=disable` |
| `uv pip show` not `pip show` | Same rule applies to all pip subcommands |

---

## 4. REPO LAYOUT & GIT STATE

**Primary repo**: `/home/brianh/.b00t` → `github.com/elasticdotventures/_b00t_`
- Current branch: `feat/dbus-ipc`
- PR open: [#265](https://github.com/elasticdotventures/_b00t_/pull/265) — session work, ready to merge

**Submodule**: `vendor/irontology-mcp` → `github.com/PromptExecution/irontology-mcp`
- Tracked branch: `feat/elasticdotventures/_b00t_`
- PR open: [#7](https://github.com/PromptExecution/irontology-mcp/pull/7) — rmcp transport WIP

**PromptExecution push rule**: `feat/elasticdotventures/_b00t_` ONLY. NEVER push to main on any PromptExecution repo.

---

## 5. b00t FUNDAMENTALS

b00t is a polyglot hive management system:
- MCP server: `b00t-mcp` (Rust, rmcp 0.8.5)
- CLI: `b00t-cli` (Rust, clap)
- Core lib: `b00t-c0re-lib` (Rust, shared logic)
- Python: `b00t-grok-py` (FastMCP, GrokGuru, RAG-Anything)
- Datums: `_b00t_/*.tomllm` — TOML+enriched comments, source of truth for tools/services/MCP
- Hive profiles: `_b00t_/*.stack.tomllm` — systemd service specs + resource gates
- Justfile: `just -l` for all recipes
- `.tomllm` naming: **PascalCase** (e.g. `HuggingfaceMcp.mcp.tomllm`, `Vllm.InferenceProvider.tomllm`)

**Cognitive tier routing**:
| Tier | Model | Tasks |
|------|-------|-------|
| sm0l | haiku | lint, classify, grep, format |
| ch0nky | qwen3-coder (local, port 8000) | implement, refactor, debug |
| frontier | claude-sonnet/opus | architecture, security, novel design |

---

## 6. CURRENT SERVICE STATE (as of session end)

| Service | State | Port | Note |
|---------|-------|------|------|
| `b00t-hive-inference-qwen3` | **active/running** | 8000 | llama-server, Qwen3-Coder-Next Q4_K_M |
| mistralrs-proxy | unknown | 1234 | soup-of-the-day router; check `b00t hive status` |
| Qdrant | **DOWN** | 6333 @ 192.168.2.13 | unreachable — this is why grok is broken |

**⚠️ CRITICAL**: llama-server at port 8000 is running **CPU-only** — brew llama.cpp compiled without CUDA. Evidence:
```
warning: no usable GPU found, --gpu-layers option will be ignored
prompt eval time = 79043.57 ms / 17 tokens (0.22 tokens/second)
```
24GB VRAM is completely idle. This must be fixed before any local model work is useful.

---

## 7. THE CRITICAL PATH — START HERE

**Issue #259** is the single most important thing. All local model work is blocked on it.

### Fix llama-server CUDA (issue #259)

```bash
# Option A: podman GPU container (fastest, no build)
podman run --device nvidia.com/gpu=all --security-opt=label=disable \
  -p 8000:8000 ghcr.io/ggerganov/llama.cpp:server-cuda \
  -m ~/.cache/huggingface/hub/models--Qwen--Qwen3-Coder-Next-GGUF/snapshots/main/Qwen3-Coder-Next-Q4_K_M/Qwen3-Coder-Next-Q4_K_M-00001-of-00004.gguf \
  --alias qwen3-coder -ngl 999 -c 32768 --host 0.0.0.0

# Option B: build from source
git clone https://github.com/ggerganov/llama.cpp /tmp/llama.cpp
cd /tmp/llama.cpp && cmake -B build -DGGML_CUDA=ON -DCMAKE_CUDA_ARCHITECTURES=86
cmake --build build --config Release -j$(nproc) -t llama-server
# sm86 = RTX 3090 compute capability

# Verify GPU is being used after fix:
nvidia-smi  # should show >0MB VRAM used during inference
```

After fix: update `_b00t_/inference-qwen3.stack.tomllm` `exec_start` to point to CUDA binary.
Expected speed: ~10-20 tok/s (vs current 0.22).

---

## 8. FULL OUTSTANDING ISSUES

### elasticdotventures/_b00t_
| # | Priority | Title |
|---|----------|-------|
| [#259](https://github.com/elasticdotventures/_b00t_/issues/259) | **P0** | llama-server brew build no CUDA — 0.22 tok/s |
| [#264](https://github.com/elasticdotventures/_b00t_/issues/264) | P1 | opencode local model (unblocks after #259) |
| [#260](https://github.com/elasticdotventures/_b00t_/issues/260) | P1 | irontology-mcp rmcp 0.8.5 import errors |
| [#261](https://github.com/elasticdotventures/_b00t_/issues/261) | P2 | wire b00t grok → irontology NeumannStore |
| [#262](https://github.com/elasticdotventures/_b00t_/issues/262) | P2 | NeumannStore persistence via sled |
| [#263](https://github.com/elasticdotventures/_b00t_/issues/263) | P3 | real SearchBackend embeddings (ollama nomic-embed-text) |

### PromptExecution/irontology-mcp
| # | Title |
|---|-------|
| [#4](https://github.com/PromptExecution/irontology-mcp/issues/4) | rmcp compile fix (blocks binary) |
| [#5](https://github.com/PromptExecution/irontology-mcp/issues/5) | NeumannStore persistence (sled) |
| [#6](https://github.com/PromptExecution/irontology-mcp/issues/6) | `repo.index` tool (grok ingestion path) |

---

## 9. LOCAL INFERENCE STACK — FULL PICTURE

### Model: Qwen3-Coder-Next Q4_K_M
- Architecture: hybrid attention + SSM + MoE (79.67B params, 512 experts/10 active, 48 layers)
- GGUF path: `~/.cache/huggingface/hub/models--Qwen--Qwen3-Coder-Next-GGUF/snapshots/main/Qwen3-Coder-Next-Q4_K_M/Qwen3-Coder-Next-Q4_K_M-00001-of-00004.gguf`
- Size: 48.4GB (4-shard GGUF)
- Required split: 24GB VRAM + 28GB CPU RAM (fits on this hardware with -ngl tuning)

### Why NOT vLLM
vLLM 0.17.1 BLOCKED on `in_proj_qkvz.qweight` uninitialized (SSM input projection GGUF→HF weight mapping gap). 3 patches applied, 1 remaining that's non-trivial. Patches stashed at `scripts/venv-patches/qwen3-coder-next-gguf.sh`.

### Why NOT safetensors (HF Hub)
`Qwen/Qwen3-Coder-Next` safetensors exists on Hub (1.15M downloads) — vLLM loads it natively (no GGUF loader, no patch needed). BUT: BF16 = 160GB, FP8 = 80GB. Both exceed 24GB VRAM + 31GB RAM = 55GB envelope.

### Why NOT Candle (Rust)
Supports Qwen3 MoE but inference-only library — no deployment layer, no CPU offload planner.

### Conclusion: llama-server + GGUF Q4_K_M is correct
Just needs CUDA build (#259).

---

## 10. opencode

- Version: 1.1.48 (`/home/brianh/.local/share/pnpm/opencode`)
- Config: `~/.config/opencode/opencode.json`
- Providers configured:
  - `vllm-local` → `http://127.0.0.1:8000/v1`, model `qwen3-coder`
  - `mistralrs-proxy` → `http://127.0.0.1:1234/v1`, model `mistral-local`
- Status: `vllm-local/qwen3-coder` appears in `opencode models` ✅
- **Blocked**: CPU-only llama-server (#259). Once fixed: `opencode run --model vllm-local/qwen3-coder 'reply LOCAL_ONLINE'`

---

## 11. irontology-mcp — FULL ARCHITECTURE SUMMARY

**Purpose**: Replace b00t grok's broken Qdrant backend. Semantic graph/RAG + code intelligence.
**Repo**: `github.com/PromptExecution/irontology-mcp` → `vendor/irontology-mcp`
**Client**: Energy client (AEMO/NMI/DUID standing data) — ALPHA, unreleased
**Branch**: `feat/elasticdotventures/_b00t_` — b00t integration work lives here

### 7 crates
| Crate | Purpose | Status |
|-------|---------|--------|
| `storage-neumann` | RDF triple store + vector similarity | In-memory only, no persistence |
| `retrieval` | 4-way fusion search (vector/graph/lexical/ontology) | Stubs — synthetic results |
| `codegraph` | tree-sitter AST → symbol graph (Rust + Python) | Working |
| `dsl` | LALRPOP-compiled rule routing for file intake | Working |
| `indexer` | Pipeline: file → rules → handler → chunk → embed → store | Working (stub backend) |
| `mcp-server` | MCP tool/resource registry + ServerHandler | Library OK; binary WIP (#4) |
| `domain` | Energy types: NMI, DUID, AEMO (Phase 1) | Placeholder only |

### MCP tools available (once binary compiles)
- `repo.search` — fusion search (4-way weighted)
- `repo.read_symbol` — symbol lookup by ID (stub returns `{status: "ok"}`)
- `ontology.list_classes` — OWL classes from loaded ontology
- `ontology.related_resources` — RDF triple graph traversal
- `repo.index` — **MISSING**, needed for grok digest/learn (#6)

### grok integration mapping
```
b00t grok ask(query)          → repo.search {query, top_k}
b00t grok digest(topic, text) → repo.index {topic, content}      ← needs #6
b00t grok learn(url/file)     → repo.index {content, source}     ← needs #6
b00t grok status()            → NeumannStore health check
```

### GROK_BACKEND env var (to be implemented in b00t-c0re-lib/src/grok.rs)
```
GROK_BACKEND=irontology  → spawn irontology-mcp binary, route calls to MCP tools
GROK_BACKEND=qdrant      → legacy (requires Qdrant at 192.168.2.13:6333)
GROK_BACKEND=auto        → try Qdrant, fallback to irontology (default)
```
Qdrant stays as optional feature flag — not removed.

### main.rs compile fix (issue #4 / _b00t_ issue #260)
Mirror imports from `b00t-mcp/src/mcp_server_rusty.rs` (same rmcp 0.8.5):
```bash
grep -n "use rmcp" /home/brianh/.b00t/b00t-mcp/src/mcp_server_rusty.rs
```
Known fixes needed:
- `RequestContext` and `ToolDescription` are NOT in `rmcp::model` — check actual paths
- `Implementation { name, version }` needs `..Default::default()` for `title/icons/website_url`
- `storage_neumann::NeumannConfig` → `storage_neumann::config::NeumannConfig`
- `ErrorData::code` is `ErrorCode` type, not `&str`

---

## 12. DATUMS IN `_b00t_/` (new this session)

| File | Purpose |
|------|---------|
| `inference-qwen3.stack.tomllm` | Qwen3-Coder-Next via llama-server (replaces old .hive.toml) |
| `HuggingfaceMcp.mcp.tomllm` | HF Hub MCP (httpstream, needs `$HF_TOKEN`) |
| `Vllm.InferenceProvider.tomllm` | vLLM commands + all 5 patch tracking entries |
| `IrontologyMcp.mcp.tomllm` | irontology-mcp alpha datum, build recipe, open issues |
| `PromptExecution.github.repo.tomllm` | Org registry, repo metadata, branch conventions |

---

## 13. NEW b00t-cli COMMANDS (this session)

### `b00t exec`
Audited broad-authority execution. Guards evaluated, then:
- **Block**: first occurrence → reject + record timestamp in `~/.b00t/exec-audit.json`
- **Block**: re-submitted within 300s TTL → force with warning
- **Warn**: proceed immediately (broad authority)
- `--sleep=<30s|2m|1h>`: background execution, returns immediately
- All executions logged to `~/.b00t/exec-log.jsonl` (JSONL AuditLogEntry)

### `b00t quit`
Agent killswitch — sends SIGTERM to upper agent process.
- Resolution: `B00T_AGENT_PID` env var → walk `/proc/<pid>/status` PPid upward (max 16 levels) for `claude/opencode/aider/ralph/cursor` → fallback PPID
- `--dry-run`: print target PID without signaling
- `--signal=<N>`: override signal (default 15 = SIGTERM)

---

## 14. PLAN FILE (not yet implemented)

A DBus IPC plan exists at `/home/brianh/.claude/plans/delegated-churning-wilkinson.md`.
Central `b00t.service` system daemon exposing DBus interface — zero-sudo hive control.
**Status**: Plan written, NOT implemented. Still pending.
Not urgent given current priorities (CUDA fix is #1).

---

## 15. QUICK ORIENTATION COMMANDS

```bash
# What's running
b00t hive status

# Is local model alive
curl -s http://localhost:8000/health && echo OK

# Is GPU being used (should be >0 after CUDA fix)
nvidia-smi --query-gpu=memory.used --format=csv,noheader

# List all datums
ls _b00t_/*.tomllm

# List justfile recipes
just -l

# List all b00t MCP tools
b00t mcp list

# Current git state
git status --short && git log --oneline -5

# irontology-mcp submodule state
git -C vendor/irontology-mcp log --oneline -3
git -C vendor/irontology-mcp branch --show-current
```

---

## 16. FILE LOCATIONS CHEAT SHEET

| What | Where |
|------|-------|
| b00t repo | `/home/brianh/.b00t/` |
| b00t-cli source | `/home/brianh/.b00t/b00t-cli/src/` |
| b00t-c0re-lib | `/home/brianh/.b00t/b00t-c0re-lib/src/` |
| grok client (Rust) | `/home/brianh/.b00t/b00t-c0re-lib/src/grok.rs` |
| grok server (Python) | `/home/brianh/.b00t/b00t-grok-py/python/b00t_grok_guru/` |
| irontology submodule | `/home/brianh/.b00t/vendor/irontology-mcp/` |
| NeumannStore | `/home/brianh/.b00t/vendor/irontology-mcp/crates/storage-neumann/src/` |
| mcp-server main.rs | `/home/brianh/.b00t/vendor/irontology-mcp/crates/mcp-server/src/main.rs` |
| datums | `/home/brianh/.b00t/_b00t_/` |
| vLLM patch script | `/home/brianh/.b00t/scripts/venv-patches/qwen3-coder-next-gguf.sh` |
| opencode config | `/home/brianh/.config/opencode/opencode.json` |
| Python venv | `/home/brianh/.venv/` |
| GGUF model | `~/.cache/huggingface/hub/models--Qwen--Qwen3-Coder-Next-GGUF/snapshots/main/Qwen3-Coder-Next-Q4_K_M/` |
| Audit log | `~/.b00t/exec-log.jsonl` |
| Memory | `~/.claude/projects/-home-brianh--b00t/memory/` |
