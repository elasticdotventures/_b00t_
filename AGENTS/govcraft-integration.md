# Govcraft Integration Strategy — Executive Agent Brief
# Role: executive | Skills: kaizen, TRIZ, MECE
# Generated: 2026-06-22 via parallel assimilate sub-agents + consensus synthesis
#
# Usage: b00t compile-agent --role=executive --supplement=govcraft-integration

## Ideal Final Result (TRIZ IFR)

The b00t hive emits its own type graph, capability manifest, and flowchart topology
as compile-time build artifacts. `holon-viz/src/gen.rs::generated_seed()` is deleted.
Every downstream consumer — Govcraft actor binding, MCP capability discovery, interactive
flowcharts — reads the same derived source of truth. Zero hand-maintenance.

---

## Govcraft Component Decision Matrix

Source: github.com/Govcraft (Roland Rodriguez, roland@govcraft.ai)
Stack spine: mti → acton-reactive → acton-service → acton-ai → talon

### ADOPT IMMEDIATELY (non-breaking, high leverage)

| Component | Stars | Why | Action |
|---|---|---|---|
| `rust-docs-mcp-server` | 282 | rmcp 0.1.5, live docs.rs→embeddings→MCP; highest-impact GC project | Install as companion MCP alongside b00t-mcp |
| `mti` | 14 | TypeID-spec (UUID v7 + prefix); crates.io published; replace ad-hoc string IDs in BootDatum | `cargo add mti`; wrap DatumId newtype |
| `agent-skills` | 16 | Open YAML skill standard + CLI validator; b00t .skill.toml conforms — validate it | `cargo install agent-skills-cli`; run against `_b00t_/*.skill.toml` |
| `agent-uri-rs` | 1 | `agent://` URI scheme for topology-independent A2A routing; replaces raw Redis string keys | Add to b00t-mcp capability announce |
| `beankeeper` | 9 | Double-entry accounting + sha2 transaction integrity; complements ledger-core | Add to vendor/l3dg3rr as workspace dep |
| `ofx-rs` | 2 | OFX parser; feeds beankeeper from bank exports | Companion to beankeeper |

### EVALUATE (medium-term, design gate required)

| Component | Why | Gate condition |
|---|---|---|
| `emergent` | Sources→Handlers→Sinks event engine; rusqlite state; maps to b00t pipeline stages | After b00t task scheduler stabilizes; evaluate as replacement |
| `acton-ai` | landlock/seccompiler sandbox + libsql persistence fills MECE layer (c) gap exactly | After DatumStore trait (Chalk Interner pattern) is implemented |
| `acton-service` | axum + tonic gRPC + OTEL + Cedar + NATS + Postgres; fills b00t public API surface gap | After tier-routing is stable; evaluate as ledgerr-mcp gRPC layer |
| `schemaforge` | DSL → Postgres/Cedar/REST/migrations; overlaps existing sqlx+Cedar patterns | Post-stabilization only; overlap risk high |

### SKIP (explicit rejection with rationale)

| Component | Reason |
|---|---|
| `acton-reactive` (standalone) | b00t HiveProfile + podman CDI + Redis already covers actor supervision; competing runtime primitive |
| `talon` | Multi-channel bot; not in current scope |
| `pressure-field-experiment` | Research-only, not production-ready (stigmergy multi-agent coordination) |
| `ntangler` | Git auto-commit watcher; conflicts with b00t's TDD-first commit discipline |
| `acton-dx` | HTMX wrapper; b00t has no HTMX surface |

---

## MECE Gap Map — Five Layers

| Layer | b00t Coverage | Gap | Govcraft Fill |
|---|---|---|---|
| **(a) Runtime/actor** | HiveProfile + podman CDI; no typed actor lifecycle primitive | spawn/stop/supervise contract | `acton-reactive` (EVALUATE gate) |
| **(b) Message routing** | b00t_agent_message/notify MCP; Redis-backed | No typed envelopes, no dead-letter, no backpressure | `acton-reactive` message bus (EVALUATE) |
| **(c) State persistence** | DatumStore aspirational (Chalk Interner); Redis TTL only | Agents have no durable state | `acton-ai` libsql + `emergent` rusqlite |
| **(d) Observability/viz** | holon-viz Cytoscape; arc-kit-au evidence graph; gen.rs hand-seeded | No live topology from running agents; no trace↔type correlation | Auto-derive via proc-macro (see Kaizen below) |
| **(e) AI/LLM tooling** | b00t-mcp, skill system, cognitive tier routing | No structured prompt-schema linkage; skills are markdown not typed | `agent-skills` YAML standard ADOPT |

---

## Kaizen — Ordered Implementation Backlog

**Principle: standardize first, then iterate. Each step unlocks the next.**

### K1 — Immediate (this sprint, non-breaking)
```bash
# 1. Install rust-docs-mcp-server as companion MCP
b00t mcp add rust-docs-mcp-server

# 2. Validate existing skill datums against agent-skills open standard
cargo install agent-skills-cli
agent-skills validate _b00t_/*.skill.toml

# 3. Add mti to b00t-cli — replace ad-hoc string IDs in DatumId
# In b00t-cli/Cargo.toml:
#   mti = "0.3"
# Wrap: pub struct DatumId(mti::MagicTypeId<"dat">);

# 4. Add beankeeper + ofx-rs to vendor/l3dg3rr workspace
# vendor/l3dg3rr/Cargo.toml members: add "crates/beankeeper-bridge"
```

### K2 — Structural (next sprint, single PR)
```
TRIZ P10+P25: proc-macro HolonEmit on DatumType

New crate: b00t-reflect/
  #[proc_macro_derive(HolonEmit)]
  → for each DatumType variant emits TypeRelationshipGraph::add_node() call
  → replaces holon-viz/src/gen.rs::generated_seed() entirely
  → extends canonical_viz_dsl_map() to include tax domain types (au_rd, us_rdc, crypto)
    currently missing HasVisualization impls

Files touched:
  b00t-cli/Cargo.toml            → b00t-reflect path dep
  b00t-cli/src/b00t.rs           → #[derive(HolonEmit)] on DatumType enum
  holon-viz/src/gen.rs           → delete generated_seed(); add manifest_loader()
  ledger-core/src/au_rd.rs       → impl HasVisualization for AuRdActivity, AuRdOffset
  ledger-core/src/us_rdc.rs      → impl HasVisualization for QreActivity, UsRdcCredit
  ledger-core/src/crypto.rs      → impl HasVisualization for CryptoTx, CryptoWallet
```

### K3 — Integration (after K2 lands)
```
agent-uri integration:
  Replace b00t_agent_capability string keys with agent:// URIs
  b00t-mcp/src/agent.rs: parse AgentUri from acton-agent-uri-rs
  Backwards compat: accept both string and agent:// during transition

emergent evaluation:
  Prototype: b00t task scheduler → emergent Sources→Handlers→Sinks
  Gate: only adopt if .b00t/tasks.json latency > 50ms at 1k tasks
```

### K4 — Architecture gate (executive decision required)
```
DatumStore trait implementation (Chalk Interner pattern):
  b00t learn chalk-interner → derive feasibility
  Backends: TOML (current) | SQLite (libsql/acton-ai) | Qdrant (irontology)
  Same API, swappable backends — prerequisite for acton-ai ADOPT

acton-service gRPC evaluation:
  Gate: ledgerr-mcp needs external consumers beyond Claude MCP
  If yes: wrap existing TaxArgs dispatch as tonic service
```

---

## Flowchart Generation — Implementation Path

Built on the rhai_dsl / HasVisualization analysis from this session:

```
HasVisualization.rhai_dsl  = node annotation (hover tooltip, training label)
arc-kit-au EdgeType        = edge topology (directed, typed)
Satisfies<C> impls         = constraint arc (Entity → Constraint → SatisfiesResult)
holon-viz/gen.rs           = graph seed (currently manual → target: derived via K2)

Emitter targets (all read from same derived graph after K2):
  Mermaid LR/TD            → mdbook build artifacts (static docs, auto-updates)
  Cytoscape JSON           → CDP server (interactive, clickable evidence nodes)
  DOT/Graphviz             → b00t-cli ontology export --format=dot
  b00t-cli                 → b00t diagram <type> --format=mermaid|cytoscape|dot

Training material update loop:
  cargo build → HolonEmit proc-macro fires → type-graph.json emitted
  mdbook build → mdbook-rhai-mermaid plugin → canonical_viz_dsl_map() called
  → Mermaid diagrams regenerated in-place → docs always current
```

---

## Delegation Contracts

### K1 task (sm0l tier — delegate to ch0nky)
```toml
[delegate]
goal          = "Add mti crate, wrap DatumId, add beankeeper+ofx-rs to workspace"
constraints   = ["no breaking API changes", "tests must pass", "never push to main"]
return_format = "SCORE: PASS|FAIL:<result>\nEXIT_SIGNAL: true|false"
budget        = { tokens = 6000, tool_calls = 15, wall_time_s = 120, max_depth = 2 }
```

### K2 task (frontier tier — executive oversight)
```toml
[delegate]
goal          = "Implement b00t-reflect proc-macro HolonEmit; delete gen.rs seed; add HasVisualization to tax types"
constraints   = ["proc-macro must be in separate crate", "gen.rs seed fn deleted not commented", "all existing holon-viz tests pass"]
return_format = "SCORE: PASS|FAIL:<result>\nEXIT_SIGNAL: true|false"
budget        = { tokens = 12000, tool_calls = 25, wall_time_s = 300, max_depth = 3 }
```

---

## Sharp Corners

🤓 `rust-docs-mcp-server` uses `rmcp 0.1.5` (older than current rmcp 0.8.x in l3dg3rr CLAUDE.md).
   Pin version or patch before installing — API surface may differ.

🤓 `agent-skills` YAML standard and b00t `.skill.toml` TOML standard are isomorphic but not
   identical serialization formats. Converter needed, not a drop-in validator.

⚠️ `acton-service` pulls tonic 0.14 + prost; if b00t workspace already pins a different tonic
   version this will cause dep conflict. Check with `cargo tree -d` before adopting.

🤓 `mti` TypeID prefix is snake_case ≤8 chars. Current DatumType variant names are PascalCase.
   Prefix map required: `Cli→"cli"`, `Skill→"skl"`, `Role→"rol"`, `Mcp→"mcp"`, `Agent→"agt"`, etc.

---

## Epiphanies

🤓 The Govcraft stack is b00t's mirror image in actor-model terms: they went actor-runtime-first
   (acton-reactive) then layered AI; b00t went AI-skill-first then layered runtime (HiveProfile).
   Integration boundary is at the HiveProfile service contract, not at the actor model.

🤓 `emergent` Sources→Handlers→Sinks is structurally identical to b00t pipeline stages
   (Ingested→Validated→Classified→Reconciled→Committed). If emergent's plugin registry
   (`emergent-registry`) maps to b00t DatumType, the two systems could co-evolve without merger.

🤓 `pressure-field-experiment` (stigmergy) is the only novel coordination primitive in the entire
   Govcraft org. Worth watching — if it matures it would replace Redis pub/sub in b00t A2A.

<!-- b00t:map v1
summary: Govcraft integration strategy — ADOPT/EVALUATE/SKIP matrix, MECE gap map, Kaizen ordered backlog (K1-K4), TRIZ IFR proc-macro HolonEmit, flowchart generation path, delegation contracts
tags: govcraft, acton, mti, agent-skills, agent-uri, beankeeper, emergent, TRIZ, MECE, kaizen, holon-viz, HolonEmit, proc-macro, flowchart, executive, integration
tier: frontier
cmds: b00t mcp add rust-docs-mcp-server, agent-skills validate, cargo add mti, b00t compile-agent --role=executive --supplement=govcraft-integration
complexity: 9
-->
