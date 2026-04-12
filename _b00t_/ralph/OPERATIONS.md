# Ralph Operations

This repo implements the Ralph loop runner. Ralph runs an agent repeatedly until it emits the completion signal:

`<promise>COMPLETE</promise>`

Ralph is intentionally context-scoped: each iteration is a fresh agent invocation. Long-term memory MUST live in repo state (git history, `progress.txt`, and the checked-in backlog).

## Prereqs

- `uv` installed
- One agent CLI installed/authenticated:
  - `amp`
  - `claude`
  - `codex`
- A git repo (Ralph resolves the project root by walking up to `.git`)

## Tasks (Primary Backlog)

Ralph reads tasks from:

`TODO-next.md`

Use markdown checklist items:

```markdown
# Next
- [ ] Task one
- [ ] Task two
```

If `TODO-next.md` is missing/empty/invalid, `./ralph.sh` exits early and prints a copy/paste prompt to generate tasks (nothing runs until tasks exist).

## Tasks (Legacy Compatibility)

Ralph also supports legacy TaskMaster data at:

`.taskmaster/tasks/tasks.json`

Schema reference:

`schemas/taskmaster-schema.json`

### Generating Tasks (Recommended)

Run your designated agent and instruct it to use the `prd` skill to produce a checklist backlog at:

`TODO-next.md`

Requirements for generated tasks:

- MUST use markdown checklist items as the primary format
- MUST include 3-7 small tasks with verifiable acceptance criteria
- MUST use IETF 2119 language (MUST/SHOULD/MAY) in acceptance criteria
- SHOULD keep titles imperative and implementation-scoped

## Running Ralph (CLI)

Preferred (uses the packaged entrypoint):

```bash
uv run ralph --agent codex 3
uv run ralph --agent amp 10
uv run ralph --agent claude 5
```

Wrapper (runs preflight + delegates to `uv run ralph ...`):

```bash
./ralph.sh --agent codex 3
```

Script entrypoint (equivalent behavior):

```bash
uv run --script ralphython.py --agent codex 3
```

Notes:
- `--agent` is required (unless using `--mcp`)
- default iterations is 10

## Running Ralph (MCP Server)

stdio:

```bash
uv run ralph --mcp --transport stdio
```

http:

```bash
uv run ralph --mcp --transport http --host 127.0.0.1 --port 8000
```

MCP tools/resources (current):
- Tools: `run_ralph_iteration`, `get_ralph_status`, `get_task_status`
- Resources: `ralph://tasks`, `ralph://progress`

## Configuration

Legacy TaskMaster model support:

- `RALPH_TASKMASTER_MODEL` (default: `gpt-5-codex`) for older TaskMaster-backed repos only

Codex:
- `CODEX_MODEL`
- `CODEX_REASONING_EFFORT`
- `CODEX_SANDBOX`
- `CODEX_EXTRA_ARGS`

## Sandboxed Environments

`ralph.sh` sets a repo-local uv cache by default:

- `UV_CACHE_DIR=$GIT_ROOT/.uv-cache`

If your environment needs a different location, set `UV_CACHE_DIR` explicitly.
