# AgentSys vs b00t: Capability Mapping & Gaps

## AgentSys Overview

**Modular runtime system** for orchestrating AI agents across software development workflows. 15 plugins, 35 agents, 32 skills organized around a core principle: code does deterministic work (AST, regex, static checks), AI does reasoning/synthesis.

### Token Efficiency Metric
Achieves **77% fewer tokens** vs multi-agent approaches through strategic separation of concerns.

---

## AgentSys Feature Map

### Core Workflows

| Command | Purpose | Key Pattern |
|---------|---------|------------|
| `/next-task` | End-to-end task completion (discovery → implementation → review → merge) | Gated phases with human approval gates |
| `/ship` | PR creation, CI monitoring, merge automation | Orchestrates CI/CD detection & retry logic |
| `/deslop` | Removes AI artifacts (debug statements, TODOs, console.log) | Certainty-graded: HIGH/MEDIUM/LOW |
| `/drift-detect` | Documentation-code alignment checker | JS collectors gather data, single LLM call analyzes |

### Code Quality & Analysis

| Command | Capability | Scale |
|---------|-----------|-------|
| `/agnix` | Config linter for agents | 342 rules (102 auto-fixable) |
| `/audit-project` | Multi-agent iterative code review | Cross-dimensional analysis |
| `/enhance` | Analyzes prompts, plugins, agents, docs for improvements | Semantic analysis |
| `/repo-map` | AST-based repository mapping | Deterministic structure extraction |
| `/sync-docs` | Keeps documentation synchronized | Bidirectional drift detection |

### Infrastructure & Operations

| Capability | Implementation |
|-----------|---|
| Cross-platform support | Claude Code, OpenCode, Codex CLI, Cursor, Kiro |
| State persistence | JSON files enabling resumable workflows |
| Isolated execution | Git worktrees for parallel task isolation |
| Model matching | Opus (planning), Sonnet (patterns), Haiku (mechanics) |
| CI/CD integration | Platform detection + automated retry logic |

### Cross-Tool Collaboration

- `/consult` - Query other AI tools
- `/debate` - Compare perspectives across tools
- `/learn` - Knowledge sharing between tools
- `/web-ctl` - Browser automation for agents

### Plugin Ecosystem

**15 plugins** as standalone repos under `agent-sh` org, dynamically fetched:
- Workflow management (`next-task`, `ship`)
- Domain-specific (`deslop`, `audit-project`)
- Enhancement (`agnix`, `enhance`)
- Cross-tool (`consult`, `debate`)
- Browser (`web-ctl`)

---

## b00t Feature Map

### Core Systems

| Component | Capability |
|-----------|-----------|
| **Datum System** | TOML-based AI model/provider configuration |
| **direnv Pattern** | Secure environment variable management (direnv → .envrc → .env) |
| **DRY Philosophy** | Library-first development, PyO3 Rust bindings |
| **Polyglot Stack** | Rust + Python integration |

### Skills Framework

3 agent skills currently:
- `datum-system` — Provider/model configuration
- `direnv-pattern` — Secure env var setup
- `dry-philosophy` — Enforce library reuse

### Architecture

- **MCP-based** — Model Context Protocol servers
- **TOML-centric** — Configuration via `.tomllm` format with tribal knowledge annotations
- **Local data** — State in memory or git history
- **Marketplace** — Claude Code plugin distribution

### Missing vs AgentSys

❌ No workflow orchestration (next-task, ship)
❌ No automated code review pipelines
❌ No CI/CD integration or monitoring
❌ No multi-platform support strategy
❌ No artifact cleanup (`deslop`)
❌ No cross-tool collaboration framework
❌ No browser automation
❌ Limited skill count (3 vs 32)

---

## Overlap Analysis

### What Both Have

| Capability | b00t | AgentSys | Status |
|-----------|------|----------|--------|
| Plugin marketplace | ✅ (Claude Code) | ✅ (multi-platform) | b00t is narrow; agentsys is broad |
| Configuration mgmt | ✅ (datum/TOML) | ✅ (plugin.json) | Different formats, same goal |
| Multi-agent support | ✅ (role-based agents) | ✅ (35 agents) | b00t is extensible; agentsys is populated |
| MCP integration | ✅ | ✅ | b00t prioritizes MCP; agentsys uses it for CLI tools |
| State persistence | ✅ (git history) | ✅ (JSON files) | Different approaches |
| Code-first analysis | ✅ (via DRY philosophy) | ✅ (AST/regex separation) | agentsys more systematic |

### Differences

| Aspect | b00t | AgentSys |
|--------|------|----------|
| **Platform reach** | Claude Code only | 5+ platforms (multi-CLI) |
| **Scope** | Agentic framework + governance | SDLC automation end-to-end |
| **Determinism philosophy** | Code is cheap; focus on AI reasoning | Code does deterministic work; AI reasons |
| **Token strategy** | No explicit optimization | 77% reduction via separation |
| **Configuration** | TOML datums with tribal knowledge | JSON plugins with validation rules |

---

## Gaps in b00t (Ideas to Extract from AgentSys)

### 1. **Workflow Orchestration** ⭐⭐⭐

**AgentSys Pattern:** `/next-task` pipeline (discovery → planning → implementation → review → merge)

**b00t Gap:** No end-to-end task orchestration. b00t has agents but no workflow state machine.

**Extract:**
- Gated phases with approval gates
- Task dependency resolution
- Resumable workflows via persistent state
- Branch/worktree management automation

**b00t Opportunity:**
```toml
# _b00t_/workflow.toml
[next-task-pipeline]
phases = ["discover", "plan", "implement", "review", "merge"]
gates = { plan = "human", merge = "ci" }
state_backend = "git_history"  # vs AgentSys JSON
```

---

### 2. **Code Quality Validation Framework** ⭐⭐⭐

**AgentSys Pattern:** `/agnix` (342 rules, 102 auto-fixable) + `/audit-project` (multi-agent review)

**b00t Gap:** No systematic code quality checks. No auto-fixable rule framework.

**Extract:**
- Certainty-graded findings (HIGH/MEDIUM/LOW)
- Composable rule definitions
- Auto-fix suggestions
- Configuration linting at scale

**b00t Opportunity:**
```rust
// b00t-cli/src/quality/rules.rs
pub trait QualityRule {
    fn check(&self) -> Certainty;
    fn auto_fix(&self) -> Option<CodeFix>;
}
```

---

### 3. **Artifact Cleanup (`/deslop`)** ⭐⭐

**AgentSys Pattern:** Three-phase cleanup (regex patterns → analysis → linting)

**b00t Gap:** No built-in cleanup for AI-generated artifacts.

**Extract:**
- HIGH certainty patterns (regex: console.log, debug markers)
- MEDIUM certainty (heuristics: doc ratios, verbosity)
- LOW certainty (language-specific linters)
- Certainty grading framework

**b00t Opportunity:**
```toml
# _b00t_/cleanup.toml
[[artifacts]]
name = "console-logs"
certainty = "HIGH"
pattern = "console\\.log\\("
languages = ["javascript", "typescript"]
```

---

### 4. **Documentation-Code Sync** ⭐⭐

**AgentSys Pattern:** `/sync-docs` + `/drift-detect` (tested on 1000+ repos)

**b00t Gap:** No drift detection between code and documentation.

**Extract:**
- JavaScript collectors for deterministic data gathering
- Single semantic LLM call for analysis
- Bidirectional sync
- Certainty scoring for findings

**b00t Opportunity:** Hook into existing b00t documentation requirements.

---

### 5. **Cross-Platform Distribution Strategy** ⭐⭐

**AgentSys Pattern:**
- Write once (shared `lib/`), deploy everywhere
- 15 standalone repos auto-synced from main
- MCP servers provide unified interface

**b00t Gap:** Claude Code only. No strategy for other AI editors (Cursor, OpenCode, Kiro).

**Extract:**
- Cross-platform library sync pipeline
- Standalone plugin repos with shared core
- Platform abstraction layer

**b00t Opportunity:**
```
_b00t_/
├── b00t-core/        # Shared Rust + Python
├── plugins/
│   ├── b00t-claude/  # Claude Code specific
│   ├── b00t-cursor/  # Cursor specific
│   └── b00t-opencode/ # OpenCode specific
└── lib/              # Auto-synced to all plugins
```

---

### 6. **Multi-Model Strategy** ⭐

**AgentSys Pattern:** Opus (planning), Sonnet (patterns), Haiku (mechanics)

**b00t Gap:** No explicit model matching to task complexity.

**Extract:**
- Capability-to-model mapping
- Token budget awareness
- Fallback strategy when frontier models unavailable

**b00t Opportunity:**
```toml
# _b00t_/model-routing.toml
[task-routing]
planning = "claude-opus"
pattern-matching = "claude-sonnet"
mechanics = "claude-haiku"
determinism-check = "local-coder"  # qwen3-coder vLLM
```

---

### 7. **CI/CD Integration Framework** ⭐

**AgentSys Pattern:** Platform detection + retry logic + deployment validation

**b00t Gap:** No built-in CI/CD orchestration.

**Extract:**
- Platform detection (GitHub Actions, GitLab CI, etc.)
- Transient failure retry logic
- Status polling with backoff
- Deployment validation gates

---

### 8. **Certainty Grading Framework** ⭐⭐

**AgentSys Pattern:** Used across `/deslop`, `/drift-detect`, `/audit-project`

**b00t Gap:** Decisions are binary; no confidence scoring.

**Extract:**
```rust
pub enum Certainty {
    HIGH,     // Deterministic (regex match, static check)
    MEDIUM,   // Heuristic (code analysis, pattern)
    LOW,      // AI judgment (semantic, contextual)
}
```

This enables human review gates based on confidence.

---

## Recommendations for b00t

### Priority 1 (High ROI)
1. **Certainty Grading** — Add to all agent outputs (enables gating)
2. **Code Quality Rules** — Extract agensys framework, adapt to b00t tooling
3. **Workflow Orchestration** — `/next-task` equivalent for b00t agents

### Priority 2 (Medium ROI)
4. **Artifact Cleanup** — Implement `/deslop` for b00t-generated code
5. **Documentation Sync** — Leverage existing b00t doc governance
6. **Multi-Model Routing** — Use with soup-of-the-day architecture

### Priority 3 (Strategic)
7. **Cross-Platform** — Plan Cursor, OpenCode plugins after Claude Code stabilizes
8. **CI/CD Integration** — Add to `/ship`-equivalent workflow

---

## Ideas b00t Can Teach AgentSys

1. **TOML + Tribal Knowledge Annotations** — More maintainable config than agentsys JSON
2. **Datum System** — Generalize beyond AI models to all infrastructure config
3. **DRY Philosophy Skills** — Automated library discovery & recommendations
4. **direnv Pattern** — Elegant solution to secrets management (vs scattered env files)
5. **Polyglot Rust/Python** — More efficient than pure JS (agentsys is JS-only)

---

## Shared Philosophy

Both systems embrace:
- ✅ Code does code work, AI does AI work
- ✅ Modular plugin architecture
- ✅ Multi-agent orchestration
- ✅ Token efficiency
- ✅ Deterministic operations + AI judgment
- ✅ State persistence across sessions

**Key Difference:** AgentSys focuses on *task completion workflows*; b00t focuses on *agent capability frameworks*. They're complementary, not competing.

