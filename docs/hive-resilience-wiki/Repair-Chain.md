# E · Repair-Chain — deterministic doctor, then register

"Use each agent to fix the next agent" — but **deterministic**, not an LLM
proposing patches (a bad fix must not propagate). Decision recorded 2026-09-03.

## Flow

```
for agent in topological_order(_b00t_/*.agent.toml):
    b00t-agent-doctor --fix   <agent>     # known-safe edits only ([[Agent-Doctor]] BD-003)
    b00t-agent-doctor --check <agent>     # gate: must PASS or SKIP
    b00t-agent-doctor --register <agent>  # nats presence on the mesh (BD-004)
    append .b00t/repair-chain.jsonl {agent, before, after, result}
```

## Stories

- **RC-001 — order** compute a safe order: stubs first, then Claude-backed,
  then local-inference agents, then the coordinators (`executive`, `b00t-comms`).
  Emit the order as JSON; no mutation.
- **RC-002 — drive** `scripts/b00t-repair-chain.sh` runs the flow above,
  stops on the first required-FAIL that `--fix` couldn't resolve, writes the
  JSONL trail. Idempotent (re-run = no-op when all PASS).
- **RC-003 — guard** register it as a `just repair-chain` recipe and add a
  [[Watchdog]]-style note so it's discoverable; do NOT auto-run it from
  `hive activate` (operator triggers it).

## Explicitly rejected

Unsupervised `agent[i]` committing `agent[i+1]`'s datum. The novelty was cute;
the blast radius wasn't worth it. [[Review-Gate]] still applies to every commit
the chain makes.
