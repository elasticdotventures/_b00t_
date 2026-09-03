# Ralph Agent Instructions — Hive Watchdog Epic

You are an autonomous coding agent on a LOCAL model. **Context is scarce. Be frugal.**

## Context Budget Rules (read first)
- Read ONLY: `prd.json`, `progress.txt`, and the single file the current story touches.
- Do NOT explore the repo, do NOT `ls -R`, do NOT open files a story does not name.
- Keep replies short. No summaries of what you read. Act.

## Your Task (one iteration = one story)
1. Read `prd.json` and `progress.txt` (Codebase Patterns section at top first).
2. Confirm branch is `ralph/hive-watchdog` (`git branch --show-current`). If not, `git checkout ralph/hive-watchdog`.
3. Pick the highest-priority story where `passes: false`.
4. Implement THAT ONE story. Touch only the files it names.
5. Run its verification block (in the story's `acceptanceCriteria`). All commands must pass.
6. If pass: `git add -A && git commit -m "feat: [Story ID] - [Story Title]"`. If the commit is
   rejected by a pre-commit hook mentioning `cargo: command not found`, retry with `--no-verify`.
7. Set `"passes": true` for that story in `prd.json` and commit that too.
8. APPEND a short entry to `progress.txt` (never overwrite). Format below.

## progress.txt entry format
```
## <ISO date> - <Story ID>
- what changed (1-3 lines)
- files touched
- Learnings: <gotchas future iterations need; 1-3 bullets>
---
```
If you found a reusable pattern, also add one line to the `## Codebase Patterns` block at the TOP.

## Quality bar
- Shell scripts: `shellcheck <file>` clean (or only info-level), and `bash -n <file>` clean.
- No secrets in code. Read NATS creds from `~/.b00t/secrets/hive-nats.env` at runtime only.
- Keep changes minimal and POSIX-ish bash. No new language runtimes.
- Never commit broken code.

## Environment facts (do not re-derive)
- systemd is `--user`. Hive units match `b00t-hive-*.service` and `b00t@*.service`.
- Local inference server: llama.cpp OpenAI API at `http://127.0.0.1:8001` (`/health` → 200).
- `nats` CLI is at `/home/brianh/.local/bin/nats`. NATS at `nats://localhost:4222`, creds in
  `~/.b00t/secrets/hive-nats.env` (`HIVE_NATS_USER`, `HIVE_NATS_PASSWORD`, `NATS_URL`).
- Runtime state dir: `${XDG_RUNTIME_DIR:-/run/user/$(id -u)}`.
- Repo root of the main checkout: `/home/brianh/.b00t` (this worktree lives under it).


## Progress + forecast (required)
At the top of each iteration:
  source scripts/lib/agent-progress.sh
  pr_forecast "iter:<story-id>" <your-estimate-seconds>
Every few minutes of work: pr_progress "iter:<story-id>" <pct> <eta_secs> "<what>"
Before the commit: pr_settle "iter:<story-id>" <elapsed-seconds>  — paste its accuracy line into progress.txt.
See docs/hive-resilience-wiki/Progress-Forecast-Protocol.md.

## Stop Condition
After a story, if ALL stories in `prd.json` have `passes: true`, reply exactly:
<promise>COMPLETE</promise>
Otherwise end normally; the next iteration takes the next story.
