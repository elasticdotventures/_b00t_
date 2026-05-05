# 🥾 b00t — Agentic Hive OS

[![Release](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml/badge.svg)](https://github.com/elasticdotventures/_b00t_/actions/workflows/release.yml)
[![Crates.io](https://img.shields.io/crates/v/b00t-cli.svg)](https://crates.io/crates/b00t-cli)

> **"Tell me what I'm running on, what tools are available, what I'm allowed to do, what goals I should optimize for, and where the boundaries are."**

**b00t** is a context-aware agentic operating layer: tool discovery, version management, tribal knowledge, multi-agent coordination, and MCP integration — batteries included.

## 🛡️ b00t + Ledgrrr = Agentic Shell Governance Layer ⚙️

Not "alignment" as abstract policy text — but **runtime alignment at the command boundary**:

```
LLM intent → shell command proposal → b00t hook interception
    → Ledgrrr policy / memory / provenance checks
    → guided execution, denial, rewrite, or audit trail
```

Shell hooks are the **last-mile control plane for agentic systems**. Why it matters:

| Property | What it means |
|----------|--------------|
| **Interception point** | Commands, filesystem writes, env mutation, network calls, git ops |
| **Guidance loop** | Not just block/allow — suggest safer, idiomatic alternatives |
| **Provenance** | Ledgrrr records *why* an action happened, not merely *that* it happened |
| **Alignment surface** | Policy becomes executable constraints *near the actual side effects* |
| **Composable primitive** | Works under Codex, Claude Code, aider, custom agents, CI bots, and human operators |

**Core thesis:** b00t turns the shell into an alignment boundary. Ledgrrr turns every boundary decision into accountable memory.

### How It Works

```
Agent types: "pip install flask"
    ↓
b00t hive run "pip install flask"
    ↓
Guard pipeline evaluates 61+ deterministic rules:
  ├── pip_guard → match → 🦨 "use uv pip install"
  ├── docker_guard → no match
  ├── rm_rf_guard → no match  
  ├── cargo_clean → no match
  └── ...
    ↓
Result: 🦨 warn + redirect to "uv pip install flask"
    ↓
Command executes (transformed) — agent learns the idiom
```

### What Makes It Different

| Traditional sandbox | b00t guard system |
|-------------------|-------------------|
| Blocks or allows | **Guides** with redirects |
| Security boundaries | **Behavioral conventions** |
| Static allow/deny lists | **Executable Rhai logic** |
| Human-maintained | **Datum-driven, auto-generated** |
| OS-brittle path checks | **`tool_installed()` via datum registry** — cross-platform |
| Opaque failure | **Explanatory messages with alternatives** |

### Deterministic Only — No LLM in the Loop

Every guard is an executable pattern or Rhai expression — never an LLM call. Guards are:
- **Pattern-based**: `pattern = "pip install"` → substring match
- **Rhai-based**: `pattern = { rhai = "cmd.contains(\"git\") && cmd.contains(\"--force\")" }` → boolean expression
- **Macro-based**: `pattern = { rhai = "pip_guard || docker_guard" }` → composes named macros
- **Steering-based**: `tool_installed("podman")` → datum-probed runtime branching

**61 guards** across 8 categories:

| Category | Guards | Example |
|----------|--------|---------|
| Package managers | 6 | pip → uv, conda, brew, npm global, apt -y |
| Containers | 4 | docker → podman, compose, build |
| Destructive ops | 12 | rm -rf /etc, /var, /bin, chown -R, sudo rm -rf |
| Git safety | 8 | force push, main branch, conventional commits |
| API/Secret exposure | 4 | --token, --api-key, --password, export SECRET |
| Code safety | 4 | curl | bash, chmod 777, sed -i backup |
| Build optimization | 4 | cargo test --no-run, cargo clean, npx --yes |
| SCM conventions | 5 | branch naming, commit format, PR workflow |

### Ledgrrr Integration

Ledgrrr provides the **FOCUS v1.3** record schema for cost attribution and the `b00t-iface` lifecycle interface. Every guard violation is logged as a structured event — auditable, replayable, attributable to the agent that triggered it.

```toml
# Guard violation log entry (~/.b00t/guard-violations.jsonl)
{"pattern": "pip install", "count": 3, "last_violation": "2026-05-04T22:30:00Z"}
```

The fine-tuning corpus at `~/.b00t/fine-tuning/guard-classification.jsonl` (301 examples) enables models to learn guard behavior directly — reducing violations over time.

### For Agents Reading This

When you operate in a b00t environment:
1. **Every command is guarded** — `b00t hive run <cmd>` evaluates 61+ deterministic rules
2. **Guards don't block arbitrarily** — they guide with idiomatic alternatives
3. **The guards encode team conventions** — `docker` → `podman`, `pip` → `uv`, `main` → PR
4. **Violations are counted** — repeat offenses escalate (🦨 → 💩)
5. **You can query the guard system**: `b00t hive status --guards` to see all active rules
6. **tool_installed("name")** probes the datum registry — always prefer this over `which` or `file_exists`

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

## 🔁 Next Loop Interface

`b00t.sh` carries forward the useful Ralph/OODA loop lessons, but the standalone `b00t-wiggums` repo is sunset. The loop interface is now treated as a commodity execution surface, with higher-level research/orchestration living in `b00t.sh` and `karpathy/autoresearch`.

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
