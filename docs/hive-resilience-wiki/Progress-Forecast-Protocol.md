# Progress-Forecast Protocol

Every hive agent / loop reports **regular timestamped progress + an ETA**, and
**registers the ETA forecast and its later accuracy with ledgrrr**. Calibration
is a first-class artifact, not a vibe.

## The library

`scripts/lib/agent-progress.sh` — source it, then:

| call | when | effect |
|---|---|---|
| `pr_forecast <exp_id> <predicted_secs> [service]` | once, at task start | `.b00t/forecasts.jsonl` row + queues a ledgrrr FOCUS `variant=forecast` (`billed_cost = predicted_secs`) + NATS `b00t.hive.mesh.forecast.<exp>` |
| `pr_progress <task> <pct> [eta_secs] [note]` | every cycle (≤60 s apart) | `.b00t/agent-progress.jsonl` row (`ts, agent, task, pct, eta_secs, eta_ts, note`) + NATS `b00t.hive.mesh.progress.<task>` |
| `pr_settle <exp_id> <actual_secs> [service]` | once, at task end | `.b00t/forecasts.jsonl` settle row with `abs_err_secs` + `pct_err`; queues FOCUS `variant=actual`; prints the accuracy line |

## ledgrrr wiring

The MCP surface exposed here is `mcp__ledgrrr__ledgerr_focus` only
(`append_focus_record` / `compute_focus_delta` / `experiment_score`). A forecast
is an experiment: two rows under one `experiment_id` —
`variant=forecast` (predicted secs as `billed_cost`) and `variant=actual`
(actual secs). `compute_focus_delta --exp <id>` then yields predicted-vs-actual.

Bash can't call MCP, so `pr_forecast`/`pr_settle` append the intended calls to
`.b00t/ledgrrr-focus-queue.jsonl`. An MCP-capable agent (or the orchestrator)
drains that queue:

```
# for each line in .b00t/ledgrrr-focus-queue.jsonl:
mcp__ledgrrr__ledgerr_focus(action=append_focus_record, billing_account_id=b00t-hive,
  service_name=…, agent_id=…, experiment_id=…, variant=…, billed_cost=…, effective_cost=…)
```

Milestone forecasts (model downloads, swaps, long Ralph runs) are registered
directly via the MCP tool at the time, not only queued.

## Wired today

- `scripts/ralph-poller.sh` → `pr_progress hive.gpu <util> — "heat=… pending=… power=…"` every 20 s
- `scripts/gpu-heater.sh` → `pr_progress gpu.heater` per job
- `scripts/dl-forecast-*` / the qwen38 wait-and-swap → `pr_forecast`/`pr_settle` for the GGUF download (`eta:qwen38-gguf-download`)

## Rule for Ralph agents

At the top of an iteration: `pr_forecast iter:<story-id> <your-estimate-secs>`.
Before the commit: `pr_settle iter:<story-id> <elapsed-secs>` and paste the
accuracy line into `progress.txt`. A forecast you never settle is a bug.

Back to [[Home]] · [[Review-Gate]].
