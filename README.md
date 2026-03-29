# 🥾 b00t — Agentic Hive OS

[![Release](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml/badge.svg)](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/b00t-cli.svg)](https://crates.io/crates/b00t-cli)

> **"Tell me what I'm running on, what tools are available, what I'm allowed to do, what goals I should optimize for, and where the boundaries are."**

**b00t** is a context-aware agentic operating layer: tool discovery, version management, tribal knowledge, multi-agent coordination, and MCP integration — batteries included.

---

## 🧠 Why b00t: Cost & Context Optimization for Long-Lived AI

**The Problem:** An AI agent existing for thousands of iterations faces exponential context costs. Every token spent on rediscovering tool patterns, re-learning environment constraints, or re-exploring sandbox boundaries is compound interest against your mission budget.

**The b00t Solution:** Spend early iterations building **deterministic, executable tool patterns** — not fast, but *fastidiously* detailed. Move with precision, not velocity.

### 🦀 Rust-Inspired Patterns Applied to AI Infrastructure

b00t applies Rust's core innovations to AI tooling:

| Rust Pattern | b00t Equivalent | Benefit |
|---|---|---|
| **Ownership/Borrow** | Datum-scoped tool configs | No conflicting tool states |
| **Type System** | Validatable DSLs (TOML/AST) | Compile-time config validation |
| **Zero-Copy** | Direct datum injection | No context serialization overhead |
| **Trait Bounds** | Skill #tags for feature flags | Capability-based security |
| **Lifetimes** | Session-scoped environments | Automatic cleanup on iteration end |

### 📜 Proebsting's Law for AI: "DSLs Over Greps"

> *"Every time you grep a config file, an AI loses a context window."*

**Don't:** `grep -r "redis" ~/.config/` — triggers on 847 files, 12 false positives, context explosion.

**Do:** `b00t learn redis` — loads *only* the validated, skill-tagged redis datum with:
- Exact CLI commands (no hallucination)
- Environment prerequisites (validated)
- Sandbox permissions (declared)
- Tribal knowledge (lfmf lessons)

### 🐚 The Shell Layer: Fish, Bash, and Symbolic Nesting

b00t is a **meta-shell** that wraps your existing shell (fish 🐟, bash 🐚, zsh):

```bash
# Fish shell with b00t layer
fish
> b00t learn docker  # Injects docker datum into LLM context
> b00t hive status   # Queries CMDB, not grep filesystem
```

**Symbolic Nesting:** b00t adds abstraction layers that GGUF fine-tuned models can navigate:
1. **Shell layer** — fish/bash/zsh primitives
2. **Datum layer** — TOML-declared tools with AST validation
3. **Skill layer** — #tagged capabilities (activates fine-tuned LoRA adapters)
4. **Sandbox layer** — Container/k8s pod with injected service tokens

### 🏗️ Generated, Not Stored: Docker Stacks & K8s Pods

b00t **generates** infrastructure from datums at runtime:

```bash
# Generate Valkey stack from valkey.k8s.toml
b00t stack activate valkey

# Output: Helm chart + values.yaml (generated, not stored)
# Deployed: valkey-system namespace with CDI GPU passthrough
```

**Why Generate?**
- Stored configs drift; generated configs are deterministic
- Version control the *datum*, not the rendered YAML
- Service mesh tokens injected at generation time (never stored)

---

## ⚡ Install

```bash
curl -fsSL https://raw.githubusercontent.com/elasticdotventures/_b00t_/main/install.sh | bash
```

Downloads the binary from GitHub Releases and verifies its SHA256 checksum when possible. Supports Linux x86_64/aarch64/armv7 and macOS Intel/Apple Silicon.

**From crates.io:**
```bash
cargo install b00t-cli
```

**From source:**
```bash
git clone https://github.com/elasticdotventures/_b00t_.git
cd _b00t_ && cargo install --path b00t-cli
```

---

## 🚀 Quick Start

```bash
b00t --version
b00t status                        # check tool versions vs desired

b00t cli check rust                # is rustc installed?
b00t cli install uv                # install/upgrade to desired version
b00t cli up                        # check all datums
b00t cli up --yes                  # update all tools to desired versions

b00t learn rust                    # load Rust dev context into agent session
b00t learn docker                  # container orchestration knowledge

b00t lfmf rust "PyO3: unset CONDA_PREFIX before cargo build to fix linker errors"
b00t advice rust "PyO3"            # retrieve lessons for a tool+pattern
```

---

## 🤖 MCP Integration

### Claude Code Marketplace (recommended)

```bash
# In Claude Code, add the b00t marketplace plugin
/plugin marketplace add elasticdotventures/_b00t_
/plugin install b00t@b00t-plugins
/plugin install skill-document-understanding@b00t-plugins
```

Provides `/b00t` skill, context-aware tool dispatch, and all available b00t skills.
Bundles publish deterministic MCP recipes at `.claude-plugin/recipes/{skills,roles}/*.json`
(for example: `skill-document-understanding` provides `docling-mcp` + `fetch-url-as-markdown`).

### Direct MCP Server

```bash
b00t mcp install b00t claudecode   # Claude Code
b00t mcp install b00t vscode       # VS Code
b00t mcp list                      # list available MCP servers
```

50+ MCP tools exposed via `b00t-mcp` — `b00t_up`, `b00t_status`, `b00t_learn`, `b00t_lfmf`, `b00t_advice`, and more.

---

## 🐝 Hive Coordination

```bash
# Multi-agent mission coordination
b00t acp hive create mission-id 3 "Build and deploy microservice" leader
b00t acp hive join mission-id developer
b00t acp hive sync mission-id 1    # barrier: wait for all agents at step 1
b00t acp hive ready mission-id 2   # signal readiness for step 2
```

---

## 🧱 AI Sandboxes: A Dozen Ways to Contain Superintelligence

b00t detects and manages **12+ sandbox types** — each with different isolation guarantees, token injection patterns, and service mesh wiring:

| Sandbox Type | Use Case | Token Injection | Detection |
|---|---|---|---|
| **Codex** (`CODEX_SANDBOX`) | OpenAI agent workspace | `workspace-write` / `read-only` | `CODEX_MANAGED_BY_BUN` env |
| **Claude Code** | Anthropic agent | Project-scoped `.claude/` | `CLAUDECODE=1` env |
| **Podman+CDI** | GPU workloads (NVIDIA) | `--device nvidia.com/gpu=all` | `nvidia-ctk cdi generate` |
| **Docker+nvidia-container-runtime** | Legacy GPU | `--gpus all` | `docker info` plugins |
| **Firejail** | Untrusted code | Seccomp + namespace | `firejail --version` |
| **nsjail** | CTF/sandbox | Pivot_root + cgroups | `nsjail --help` |
| **gVisor** | Multi-tenant | Sentry + gofer | `runsc --version` |
| **Kata Containers** | VM isolation | QEMU + virtio-fs | `kata-runtime --version` |
| **Systemd-run --scope** | Resource caps | cgroups v2 | `systemctl --user` |
| **SSH Multiplexer** | Remote agent | SSH keys + ControlMaster | `ssh -O check` |
| **WASM (Wasmtime)** | Portable sandbox | Capability-based | `wasmtime --version` |
| **QEMU User** | Cross-arch | linux-user emulation | `qemu-x86_64 --version` |

### 🔑 Service Authorization Token Injection

b00t injects tokens **at container generation time** — never stored in VCS:

```bash
# Podman with CDI GPU + injected tokens
podman run --rm -d \
  --device nvidia.com/gpu=all \
  --security-opt=label=disable \
  -e HF_TOKEN="${HF_TOKEN}" \
  -e DASHSCOPE_API_KEY="${DASHSCOPE_API_KEY}" \
  -e REDIS_URL="redis://valkey.valkey-system.svc:6379" \
  vllm/vllm-openai:latest
```

**Token Sources:**
1. **`.env` files** — `source .env` before generation
2. **Secret managers** — `vault kv get`, `aws secretsmanager`
3. **Service mesh** — mTLS certs from Istio/Linkerd
4. **K8s Secrets** — Mounted as env vars in pod spec

### 🕸️ Service Mesh Interface

b00t generates **service mesh wiring** for container-to-container IPC:

```toml
# inference-qwen3.stack.tomllm
[b00t.sandbox]
type = "podman+CDI"
mesh = "istio"
sidecar_inject = true
mTLS = "STRICT"

[b00t.env]
REDIS_URL = "redis://valkey.valkey-system.svc:6379"  # Cluster DNS
```

**Generated:**
- Istio `DestinationRule` with mTLS policy
- `ServiceEntry` for external APIs (HuggingFace, DashScope)
- `PeerAuthentication` for workload identity

---

## 🔁 Ralph — OODA Loop Runner

`b00t.sh` implements the Ralph autonomous task loop (Observe→Orient→Decide→Act):

```bash
# Run with default claude tool, 10 iterations
bash b00t.sh

# Configure via env or flags
TOOL=mistralrs bash b00t.sh --max-iterations 5 --role executor
bash b00t.sh --tool codex --sleep 1
```

Tools: `claude`, `codex`, `amp`, `opencode`, `mistralrs` (local vLLM).
Loop exits on `EXIT_SIGNAL=true` in LLM output, or `exit 75` (tempfail/restart) after max iterations.

---

## 📦 Datums

Datums in `_b00t_/` declare tools with detect/install/desires/hint. b00t resolves DAG-ordered installs:

```bash
b00t cli detect fastmcp            # run datum detect script
b00t cli desires rust              # show target version
b00t cli install fastmcp           # install: python → uv → fastmcp (DAG-aware)
```

---

## 🧠 Session & Budget Management

```bash
b00t session init --budget 25.00 --time-limit 120 --agent "code-reviewer"
b00t session status
b00t checkpoint "completed feature X"
```

---

## 🌐 Platform Support

| Platform | Architecture | Status |
|---|---|---|
| Linux | x86_64 | ✅ |
| Linux | aarch64 | ✅ |
| Linux | armv7 | ✅ |
| macOS | Intel | ✅ |
| macOS | Apple Silicon | ✅ |

---

## 🛠 Development

```bash
git clone https://github.com/elasticdotventures/_b00t_.git && cd _b00t_
cargo build
cargo test --workspace
just -l                            # available recipes
just release                       # dispatch GitHub release workflow
```

Release pipeline: `release.yml` (tag + GitHub Release) → `build-release.yml` (cross-platform binaries) → `publish-crates.yml` (crates.io).

---

## 📖 Docs

- **[b00t Gospel](./.b00t.g0spell.md)** — philosophy and architecture
- **[Agent Guide](./_b00t_/AGENT.md)** — agent operation protocol
- **[CLAUDE.md](./CLAUDE.md)** — LLM alignment instructions
- **[Release Notes](./RELEASE.md)** — changelog

---

*Issues / hive recruitment: [github.com/elasticdotventures/_b00t_/issues](https://github.com/elasticdotventures/_b00t_/issues)*
