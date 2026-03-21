# 🥾 b00t — Agentic Hive OS

[![Release](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml/badge.svg)](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/b00t-cli.svg)](https://crates.io/crates/b00t-cli)

> **"Tell me what I'm running on, what tools are available, what I'm allowed to do, what goals I should optimize for, and where the boundaries are."**

**b00t** is a context-aware agentic operating layer: tool discovery, version management, tribal knowledge, multi-agent coordination, and MCP integration — batteries included.

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
```

Provides `/b00t` skill, context-aware tool dispatch, and all available b00t skills.

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
