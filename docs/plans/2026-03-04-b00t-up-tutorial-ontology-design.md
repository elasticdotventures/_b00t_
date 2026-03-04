# b00t up / Tutorial / Ontology — Design Document
**Date:** 2026-03-04
**Branch:** codemacine/dev
**Status:** Approved — pending implementation plan

---

## Problem Statement

b00t needs three cohesive capabilities:
1. **`b00t up`** — outer REPL loop that launches and manages a ralph agent session, enabling agent self-termination and restart
2. **Tutorial mode** — tracks per-datum installation/validation progression, role-aware, surfaces "what to install next"
3. **Ecosystem ontology** — live query of what is installed → what is feasible, injectable into agent context

These must be achieved with **minimal new files**, preferring existing datum TOML infrastructure and session memory.

---

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Outer-loop runtime | Rust (`b00t up` command) | Type-safe, consistent with arch |
| IPC shell | `b00t.sh` (existing ralph loop) | Already the agent loop; reuse not rebuild |
| Tutorial state storage | Session memory (existing `~/.b00t/sessions/`) | Zero new files |
| New config files | NONE | Operator requirement: resist config file proliferation |
| Role data | Added field in existing datum TOMLs | DRY, single source of truth |
| Restart signal | Exit code (75=TEMPFAIL=restart, 0=done, 1=error) | POSIX convention, no new IPC file |
| Memory provider priority | Copaw (if detected) → Redis → session file | Copaw as preferred provider |
| Sync mechanism | b00t-ipc heartbeat on state changes | Distributed proof-of-life |
| Codemachine | Out of scope | .gitignore config, skip |

---

## Architecture

```
┌──────────────────────────────────────────────────┐
│  b00t up [--tool claude|amp|codex] [--role <r>]  │
│  Rust command in b00t-cli (commands/up.rs)       │
│                                                  │
│  1. Query ontology (live datum TOML scan)        │
│  2. Export B00T_ONTOLOGY=<json> env var          │
│  3. Exec b00t.sh --tool <tool> <max_iter>        │
│  4. Wait on child process                        │
│  5. Read exit code:                              │
│     75 (TEMPFAIL) → increment counter → restart  │
│     0             → graceful done               │
│     other         → log error → exit 1          │
│  6. Emit IPC heartbeat (b00t-ipc)               │
│  7. Update session memory: tutorial progress     │
└──────────────────────────────────────────────────┘
                    │ exec
┌───────────────────▼──────────────────────────────┐
│  b00t.sh (existing ralph loop — unchanged)       │
│  Reads B00T_ONTOLOGY from environment            │
│  Runs claude/amp/codex agent loop                │
│  Agent exits with code 75 to request restart     │
└──────────────────────────────────────────────────┘
```

---

## Components

### 1. `b00t up` — Rust Command (`src/commands/up.rs`)

**CLI:**
```
b00t up [--tool <amp|claude|codex>] [--max-iter <N>] [--role <role>] [--max-restarts <N>]
```

**Behavior:**
- Default: `--tool claude --max-iter 10 --max-restarts 5`
- Builds live ontology from datum TOMLs before each spawn
- Passes `B00T_ONTOLOGY` + `B00T_ROLE` as env vars to `b00t.sh`
- Exit code 75 = POSIX TEMPFAIL = agent requests restart (self-termination protocol)
- Writes session memory keys: `up.restart_count`, `up.last_exit`, `up.tool`
- On final done/error: runs `b00t tutorial next` to surface next step

**Note:** `b00t install` as alias for `b00t up` is additive to existing `b00t install` (which runs `just install`).
Consider: `b00t up` is the new REPL entry point; `b00t install` keeps existing behavior for `just install`.

### 2. Tutorial Progression — Session Memory Extension

**No new files.** Extends existing `SessionMemory` struct in `src/session_memory.rs`.

New fields added to session:
```toml
# Written into existing session toon file
[tutorial]
role = "developer"
completed = ["git", "gh", "just", "rustc"]   # validated datums
skipped = ["azure-ai-foundry"]
path = ["git", "gh", "just", "rustc", "context7", "taskmaster-ai"]
last_next = "context7"
```

**Commands:**
- `b00t tutorial status` — renders role-path completion table from session + datum TOMLs
- `b00t tutorial next` — first unvalidated required datum for current role
- `b00t tutorial skip <datum>` — marks datum skipped in session
- `b00t tutorial validate <datum>` — runs datum's validate command, updates session

**Validation:** Each datum TOML gets one optional new field:
```toml
# Added to existing *.cli.toml, *.mcp.toml, etc.
[validate]
command = "git --version"
regex = "git version \\d+"

[roles]
required_for = ["developer", "orchestrator"]
optional_for = ["analyst"]
```

### 3. Ontology Query — Derived, Not Stored

`b00t ontology query [--role <role>] [--format json|table]`

Reads existing datum TOMLs in real-time. No new state files.

Output categories:
- **available** — installed + validated; agent can use NOW
- **installable** — not installed, no blockers; `b00t cli install <name>` works
- **blessings** — credentials/auth detected (api keys, tokens in env/keychain)
- **blocked** — requires sudo or missing prerequisite

`B00T_ONTOLOGY` env var format (injected by `b00t up` into ralph):
```json
{
  "role": "developer",
  "available": ["git", "gh", "just", "context7"],
  "installable": ["k9s", "argo-cli"],
  "blessings": ["ANTHROPIC_API_KEY", "GITHUB_TOKEN"],
  "timestamp": "2026-03-04T..."
}
```

### 4. MemoryProvider — Copaw Preferred

Detection order (runtime, no config):
1. Check `copaw` datum state in session: if `validated` → use copaw MCP
2. Check redis: `redis-cli ping` → if PONG → use redis
3. Fallback: session file (always available)

Copaw MCP tools used: `copaw.memory.read`, `copaw.memory.write`
Interface: minimal trait, 3 methods (read/write/sync).

---

## Ralph Integration Changes

`b00t.sh` gains awareness of two env vars (backward-compatible, ignored if absent):
- `B00T_ONTOLOGY` — JSON blob; ralph injects into agent CLAUDE.md preamble
- `B00T_ROLE` — string; used to filter relevant skills/datums in agent context

Agent self-termination protocol: `exit 75` from within ralph agent subprocess signals `b00t up` to restart. Agent writes intent to `progress.txt` before exiting so restart picks up context.

---

## Scope Boundaries

**In scope (Phase 1 MVP):**
- `b00t up` Rust command with ralph spawn + restart loop
- `b00t tutorial status|next|skip|validate` commands
- `b00t ontology query` command
- `B00T_ONTOLOGY` env injection into `b00t.sh`
- `[validate]` + `[roles]` fields in existing datum TOMLs (additive, backward-compatible)
- Session memory extension for tutorial fields
- MemoryProvider trait with copaw/redis/file impls
- IPC heartbeat on `b00t up` state changes

**Out of scope:**
- Codemachine integration
- New standalone config files
- MCP-native tutorial server (Approach C)
- Azure AI Foundry orchestration changes

---

## Phased Rollout

| Phase | Deliverable | Test |
|-------|-------------|------|
| P1 | `b00t up` command + ralph spawn + exit-code restart | integration: spawn mock agent, assert restart on 75 |
| P2 | `[validate]` + `[roles]` fields in 10 core datums | unit: parse existing toml + new fields |
| P3 | `b00t tutorial status|next|validate` | unit: mock session, assert path computation |
| P4 | `b00t ontology query` + `B00T_ONTOLOGY` injection | unit: scan datum dir, assert json output |
| P5 | MemoryProvider trait + copaw impl | integration: copaw MCP mock |
| P6 | IPC heartbeat on state changes | unit: mock ipc, assert event emission |

---

## Success Criteria

- `b00t up --tool claude` successfully launches ralph, injects ontology, and restarts on exit 75
- `b00t tutorial next` returns the next unvalidated datum for detected role
- `b00t ontology query --format json` emits valid JSON with available/installable/blessings
- Zero new config files introduced (validated by `git diff --name-only | grep -v '.toml$'`)
- All existing `b00t` tests still pass
- Copaw used as memory provider when `copaw` datum is validated
