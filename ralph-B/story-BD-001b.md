# Ralph story BD-001b — sharpen Agent-Doctor (ONE iteration, then stop)

Local-model agent. Context scarce. Working dir:
/home/brianh/.b00t/.claude/worktrees/hive-watchdog (branch ralph/hive-watchdog)

`scripts/b00t-agent-doctor.py` already exists (written by the pi harness). Sharpen it,
do not rewrite from scratch. Only touch that one file.

## Changes
1. `classify()` over-matches: a bare substring `"local"` anywhere flags an agent
   `local`. Instead: `local` only if text has `ch0nky` OR `qwen` OR a
   `hive_profile = "inference-..."` line OR `local/ch0nky` / `local/sm0l` in a
   `model = ...` line. Keep `claude` = `model` line contains `claude`/`sonnet`/`frontier`.
2. Add `--json`: emit a JSON array of `{name, class, verdict}` instead of the TSV table.
3. Keep stdlib-only, keep it under ~90 lines, keep the exit-1-on-any-local-FAIL contract.

## Verify (all must pass)
- `python3 -m py_compile scripts/b00t-agent-doctor.py`
- `python3 scripts/b00t-agent-doctor.py --check`  → one line per `_b00t_/*.agent.toml`, exits 0 or 1
- `python3 scripts/b00t-agent-doctor.py --json | python3 -c 'import json,sys; a=json.load(sys.stdin); assert isinstance(a,list) and a and {"name","class","verdict"} <= set(a[0]); print("json ok", len(a))'`

## Commit
`git -c core.hooksPath=/dev/null add scripts/b00t-agent-doctor.py && git -c core.hooksPath=/dev/null commit -m "feat: [BD-001b] - sharpen Agent-Doctor classify + --json"`
Then reply `<promise>COMPLETE</promise>`.
