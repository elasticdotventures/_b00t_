# Ralph story BD-001 — Agent-Doctor census (ONE iteration, then stop)

You are a local-model coding agent. Context is scarce. Do exactly this, nothing more.

Working dir: /home/brianh/.b00t/.claude/worktrees/hive-watchdog  (branch ralph/hive-watchdog)

## Task
Create `scripts/b00t-agent-doctor.py` (Python 3.11+, stdlib only — `tomllib`, `pathlib`, `json`, `argparse`, `urllib`).

`--check` (default): iterate `_b00t_/*.agent.toml`. For each agent print one row:
`<name>  <class>  <verdict>` where
- `class` = `local` if the file's text contains `ch0nky` or `qwen` or `local` or `hive_profile` matches `inference-`, else `claude` if `model` contains `claude`/`sonnet`/`frontier`, else `stub`.
- `verdict` = `SKIP` for `claude`/`stub`; for `local`: GET `http://127.0.0.1:8001/health` (2s timeout) → `PASS` on HTTP 200, else `FAIL`.
Exit 1 if any `local` agent is `FAIL`, else 0.

Keep it under ~70 lines. No external packages. No network except the health GET.

## Verify (run these, all must pass)
- `python3 -m py_compile scripts/b00t-agent-doctor.py`
- `python3 scripts/b00t-agent-doctor.py --check` prints one line per `_b00t_/*.agent.toml` and exits 0 or 1 (not a traceback).

## Commit
`git -c core.hooksPath=/dev/null add scripts/b00t-agent-doctor.py && git -c core.hooksPath=/dev/null commit -m "feat: [BD-001] - Agent-Doctor census"`

Then reply `<promise>COMPLETE</promise>`.
