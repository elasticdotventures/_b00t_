# B · Agent-Doctor — a checked path to inference for every agent

18 `_b00t_/*.agent.toml` today; transports are all over the map (msgpack sock,
http+acp, rpc+stdio, nats+acp, mcp+stdio, stubs). Only ~6 actually reach local
inference and none declare it uniformly. Give them one contract + a verifier.

## The contract (new datum block)

```toml
[b00t.agent.inference]
endpoint = "http://127.0.0.1:8001/v1"   # OpenAI-compatible base
health   = "http://127.0.0.1:8001/health"
model    = "qwen36-local/ch0nky"
protocol = "openai"                      # openai | acp | rpc
required = true                          # false ⇒ Claude-backed / stub ⇒ doctor SKIPs
```

## Ralph stories (PRD)

- **BD-001 — Census** `scripts/b00t-agent-doctor.py` (`uv` shebang, `tomllib`):
  list every `_b00t_/*.agent.toml`; classify local-inference (`hive_profile`
  matches `inference-*`, or `model` contains `ch0nky|qwen|local`) vs Claude/stub.
  `--check` prints a `PASS|FAIL|SKIP` table. Verify: runs clean, table has a row per agent.
- **BD-002 — Probe** `--check` also GETs `health` (expect 200) and POSTs a
  1-token `/v1/chat/completions`; FAIL any required agent that can't answer.
  Exit non-zero on any required FAIL. Verify: with `:8001` up, the ~6 real
  agents PASS; stubs SKIP.
- **BD-003 — Fix** `--fix` injects the standard `[b00t.agent.inference]` block
  (via `b00t-cli patch apply`) into local-inference agents that lack it; rewrites
  stale `after = […inference-qwen36-27b.service]` → `…-mtp-podman.service`.
  Verify: re-running `--check` after `--fix` shows no "missing inference block".
- **BD-004 — Register** `--register`: for each PASS agent, `nats pub`
  `b00t.hive.mesh.discovery.presence` `{agent_id, endpoint, model, ts}` (creds
  from `~/.b00t/secrets/hive-nats.env`). Verify: `nats sub` sees one presence
  message per healthy agent.
- **BD-005 — Wire** `b00t.just`: `agent-doctor`, `agent-doctor-fix`,
  `agent-register`. Verify: `just agent-doctor` exits 0.

## Harness

Do BD-001/003/005 with the [[Harness-Notes|opencode Ralph]] (file-heavy edits);
BD-002/004 with the pi Ralph (short, network-y). [[Review-Gate]] before each commit.

Feeds [[Repair-Chain]] (which orchestrates `--fix` + `--register` across the hive)
and [[Health-Metrics]] (presence + health on the same `b00t.hive.mesh.*` bus).
