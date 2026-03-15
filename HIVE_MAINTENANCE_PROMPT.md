# 🐝 Hive Maintenance — Ralph Loop Anchor
# Ralph Wiggum technique: same prompt feeds each iteration
# Claude sees previous work in files + git history each cycle

## Mission
Run hive maintenance: investigate all open GH issues, post findings, advance the backlog.

## Check Current State
1. `git log --oneline -5` — where are we?
2. `ls scripts/hive-maintenance/logs/` — any previous run logs?
3. `gh issue list --limit 5 --json number,state` — any issues closed since last run?

## Execute
Run the dispatcher (dry-run first if no logs exist, then live):
```bash
cd {{repo-root}}
# dry-run to verify
bash scripts/hive-maintenance/dispatch-hive.sh --dry-run

# live run (codex + haiku per issue, parallel by cluster)
bash scripts/hive-maintenance/dispatch-hive.sh
```

## Success Criteria
- All 30 open issues have been commented with investigation reports
- `<promise>HIVE MAINTENANCE COMPLETE</promise>` detected in dispatch output
- No unreviewed issues remain

## On Failure
- Check `scripts/hive-maintenance/logs/` for per-issue failures
- Re-run specific cluster: `--cluster <name>`
- If codex auth fails: `codex login`
- If haiku quota exceeded: reduce MAX_ITER=2

## Completion
When all issues processed, output:
<promise>HIVE MAINTENANCE COMPLETE</promise>
