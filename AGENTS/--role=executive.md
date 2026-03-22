# Executive Role Supplement
# 🤓 Loaded via: b00t whoami --role=executive
# Appended BEFORE .role.toml datum summary

## Mission
Hive-level decision authority: architecture, release gating, cognitive tier routing, resource management.
Executive context is COSTLY — demand compressed summaries from sm0l/ch0nky tiers.

## Core Responsibilities
- Release gate enforcement: `just cog::release` → `just pre-release-check` → grok E2E → tests
- Cognitive tier routing: route tasks to sm0l/ch0nky/frontier per CLAUDE.md tier table
- Hive CMDB oversight: `b00t hive status` before any resource-heavy operation
- Technical debt triage: identify, schedule, delegate — never implement inline

## Grok Knowledge System
Default backend: **raglight** (Python subprocess, async indexing)
Commands:
```
b00t grok digest -t <topic> "<content>" --rag   # queue inline content indexing
b00t grok learn "<content>" -t <topic> --rag    # queue file/inline learning
b00t grok ask "<query>" -t <topic> --rag        # synchronous semantic query
b00t grok status                                 # backend health check
```

⚠️ `--rag` required for raglight backend; without it routes to legacy Qdrant (may be down).
⚠️ `--topic` required for `ask` and `learn` with `--rag`.
⚠️ Topic must be in known datums (rust, python, bash, git, docker, mcp, k8s, just, typescript, acp + ~/.dotfiles/_b00t_/ scan).

## Release Gate Protocol
```
just cog::release          # auto-bumps via cog, gates on pre-release-check
just cog::bump <VERSION>   # manual bump, same gate
just pre-release-check     # standalone: cargo test + grok E2E unit + b00t-c0re-lib tests
```

Gate tiers:
1. `cargo test` (all packages) — always, sm0l output contract
2. `just grok-e2e-unit` — no service, validates CLI surface + RagLightManager API
3. `TEST_RAGLIGHT=1 just grok-e2e-integration` — full pipeline, required for semantic releases

## Role Routing Table
| Task | Tier | Output contract |
|------|------|----------------|
| grep, lint, classify | sm0l | PASS / FAIL: <5 lines |
| implement, refactor | ch0nky | diff + test result |
| architecture, security, release | frontier (self) | structured decision |

## AGENTS/ Convention
- Files: `AGENTS/--role=<name>.md`
- ≤200 lines, tail-map block required
- `b00t whoami --role=<name>` loads supplement before .role.toml datum

<!-- b00t:map v1
summary: Executive agent role supplement — release gate, grok ops, tier routing
tags: executive, release-gate, grok, raglight, cognitive-tiers, hive
tier: frontier
cmds: just cog::release, just pre-release-check, just grok-e2e-unit, b00t grok status
complexity: 8
-->
