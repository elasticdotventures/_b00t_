# Harness-Notes — opencode vs pi on the local box

The rule: **one opencode Ralph, one pi Ralph, run to the end.** (Goal, 2026-09-03.)

## opencode (`opencode run`)

- Cold-starts a full opencode server **plus ~5 MCP subprocesses** (npx github,
  context7, codex, huggingface http, codebase-memory) on **every** invocation.
- On this 31 GB / 4-core host, looping it exhausted RAM + swap and **broke the
  Claude Code Bash tool** (`fork()` fails silently, exit 1 no output) until
  ~10 GB of stale `/tmp/claude-*` tmpfs scratch was cleared.
- Mitigations: `opencode run --pure` (no plugins), `--dir <worktree>`,
  `--auto` (headless approve). Prefer **one bounded invocation**, not a tight loop.
- Ralph wiring fixed this session: `executors.py` now calls `opencode run`
  (was bare `opencode` → TUI); `ralph_cli.py --tool` now accepts `pi`.

## pi (`pi -p --provider llama-cpp --model ch0nky`)

- No server, no MCP swarm. ~90–240 s cold start (MCP init + first token), then
  fine. `~/.pi/agent/models.json` already points `llama-cpp` → `:8001/v1`,
  `apiKey=local-b00t`.
- Cheaper on RAM; use it for the short, network-y stories (probes, `nats pub`).

## Context frugality (local model)

- One page of this wiki per Ralph iteration. Don't attach the repo.
- `prompt.md` forbids repo exploration; story names the exact files.
- `progress.txt` is the only cross-iteration memory — append, never rewrite.
- `--pure` / minimal MCP: every extra tool definition is tax on ch0nky's window.

Back to [[Home]].
