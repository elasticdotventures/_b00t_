# b00t vs Compound-Engineering-Plugin — Capability Mapping & Gap Analysis

## Architecture Comparison

| Dimension | Compound-Engineering (Every) | b00t (PromptExecution) |
|-----------|------------------------------|------------------------|
| **Runtime** | Claude Code / Codex plugin (TypeScript) | Rust CLI + Hermes MCP |
| **Distribution** | npm `@every-env/compound-plugin` | cargo install, GHCR, npm |
| **Skills** | 38 `.md` skill files | 40+ skills in `~/.hermes/skills/` |
| **Agents** | 43 specialized reviewer personas | Sub-agents via `delegate_task` |
| **State** | SQLite + worktrees | `b00t soul` + task-queue JSONL |
| **Health** | `/ce-product-pulse` (time-windowed) | `b00t doctor check` + `/api/admin/health` |
| **Compound** | `/ce-compound` → markdown notes | `b00t lfmf` + `grok digest` → vector DB |

## Workflow Mapping

| Compound-Engineering | b00t Equivalent | Parity | Gap |
|---------------------|-----------------|--------|-----|
| `/ce-strategy` | `b00t soul set mission` + `whoami --role=executive` | 🟡 | b00t: no STRATEGY.md generation |
| `/ce-ideate` | `b00t plan` (writing-plans) + `autoresearch` | 🟡 | CE: ranked ideation artifact |
| `/ce-brainstorm` | `b00t plan` + `grok ask` | 🟡 | CE: interactive Q&A loop |
| `/ce-plan` | `b00t plan` + `task add` | 🟢 | Feature-comparable |
| `/ce-work` | `b00t task next/done` + `delegate_task` | 🟢 | b00t: stronger (typed composition) |
| `/ce-debug` | `systematic-debugging` skill | 🟢 | 4-phase root cause |
| `/ce-code-review` | `github-code-review` skill + `requesting-code-review` | 🟡 | CE: 43 specialized personas |
| `/ce-compound` | `b00t lfmf` + `grok digest` | 🟢 | b00t: vector DB, richer |
| `/ce-doc-review` | No direct equivalent | 🔴 | Gap — b00t has no doc review skill |
| `/ce-product-pulse` | `b00t doctor check --json` + `/api/admin/health` | 🟡 | CE: time-windowed, browseable |

## Key Gaps (CE has, b00t lacks)

1. **Adversarial review personas** (43 agents) — b00t's sub-agents are generic, CE has domain-specific reviewers (security, coherence, UX, architecture, performance)
2. **Interactive Q&A brainstorming** (`/ce-brainstorm`) — b00t plans are one-shot, CE has iterative dialog
3. **Ranked ideation** (`/ce-ideate`) — b00t has no formal ideation pipeline
4. **Documentation review** (`/ce-doc-review`) — b00t has no doc review skill
5. **Time-windowed pulse reports** — b00t health is point-in-time, CE has windowed trends

## Key Strengths (b00t has, CE lacks)

1. **Typed composable pipelines** — `PipelineNode<I,O>` with FOL contracts, state machines
2. **WASM codegen** — Rust types → WAT/Cython/Mermaid diagrams
3. **Digital twin simulation** — `DigitalTwin<T>` with tick/rollback/subscribe
4. **Container stack** — Podman quadlet + kube play deployment
5. **Vector DB memory** — `lfmf` + `grok digest` persist to irontology/NeumannStore
6. **Admin server** — HTTP/SSE with health metrics, process visualization
7. **Role-based identity** — `b00t whoami --role=executive` with blessing manifests

## Recommendation

**b00t already has its own equivalent** — the capabilities are distributed across skills, CLI commands, and MCP tools rather than unified as a single plugin. The architecture is different (Rust monolith vs TypeScript plugin) but the workflow coverage is ~80%.

### Do NOT integrate CE plugin
- CE is TypeScript/Claude Code specific — b00t is Rust/Hermes native
- b00t's distributed architecture serves different agent runtimes (Claude Code, Codex, Hermes)
- CE's 43 personas could be replicated as b00t skills if needed

### Close the 3 gaps

| Gap | Solution | Effort |
|-----|----------|--------|
| Doc review | Add `doc-review` skill using b00t's existing pipeline types | Small |
| Ranked ideation | Add `ideate` command to b00t-cli (uses grok dual-backend) | Medium |
| Time-windowed pulse | Extend `/api/admin/health` to accept `?window=7d` | Small |

### Compound the learning
```bash
b00t lfmf --tool b00t "compound-engineering parity: b00t covers 80% of CE plugin workflow. Gaps: doc-review skill, ideate command, time-windowed health. b00t strengths CE lacks: typed pipelines, WASM codegen, twin simulation, container stack. Architecture is Rust CLI vs TS plugin — complementary, not competitive."
```
