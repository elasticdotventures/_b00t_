---
name: b00t-maintenance
description: |
  Access the b00t maintenance loop — check status, view cake token balance,
  confirm exercise reminders, and inspect the research queue. The 👍🏻/👎🏻
  vote tokens feed the probabilistic cake lottery resolved at pre-commit review.
version: 1.0.0
tags: [b00t, maintenance, cake, adhd, health, reminder, loop, vote]
applies_to: [checking loop status, confirming reminders, viewing cake balance, understanding cake lottery, triggering research tasks]
output_types: [.json, .md]
allowed-tools: [mcp__b00t-mcp__b00t_maintenance_status, mcp__b00t-mcp__b00t_agent_vote_submit, Bash]
---

## What the Maintenance Loop Does

The b00t maintenance daemon runs a 15-minute OODA cycle:

1. **Exercise reminders** — fires at configurable intervals; awaits `<|👍🏻|>` confirmation
2. **Research soul queue drain** — pulls pending `research-soul` tasks and dispatches sm0l agents
3. **OODA cycle** — Observe (collect metrics) → Orient (score useful-work) → Decide (next action) → Act (emit notification or ticket)

Start the daemon: `just maintenance-start`

Status check: `b00t-cli maintenance status`

## Vote Token Format

Votes are embedded inline in agent output or commit messages:

| Token | Meaning | Effect |
|---|---|---|
| `<\|👍🏻\|>` | Thumbs up — reminder confirmed / task useful | +1 lottery ticket |
| `<\|👎🏻\|>` | Skip — snooze or mark not useful | no ticket |

Votes are harvested by the daemon on each OODA cycle. Multiple votes in a session accumulate.

## Cake Lottery

The cake lottery is **probabilistic**, resolved at pre-commit review:

- Each `<|👍🏻|>` vote earns one lottery ticket
- Win probability = `time_accuracy × useful_work_score` (both 0.0–1.0)
- `time_accuracy`: how closely exercise was confirmed within the reminder window
- `useful_work_score`: agent self-assessment of output quality for the session
- Resolution: `b00t-cli cake ticket resolve` runs at pre-commit hook

### Cake Commands

```bash
b00t cake balance          # show current token balance for default operator
b00t cake balance operator # show named operator balance
b00t cake history          # last 10 cake events
b00t cake history --limit=N
b00t cake search <query>   # search cake event log by keyword
```

## Troubleshooting

**Daemon not running:**
```bash
just maintenance-start
# or
b00t-cli maintenance start --detach
```

**Zellij not detected** — daemon emits reminders via Zellij pane; if Zellij is absent,
reminders fall back to desktop notification (`notify-send`) or stderr. No votes are lost.

**Vote not registered** — confirm the token format is exact: `<|👍🏻|>` (no spaces inside pipes).
Run `b00t-cli maintenance status` to inspect the pending vote queue.

## Related

- `just research-soul` — trigger one research task immediately
- `mcp__b00t-mcp__b00t_agent_vote_submit` — MCP tool to submit votes programmatically
- `mcp__b00t-mcp__b00t_maintenance_status` — MCP tool for daemon status JSON
