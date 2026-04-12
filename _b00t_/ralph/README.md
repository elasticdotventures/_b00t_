# Ralph

Ralph is an autonomous agent loop runner for Amp, Claude Code, Codex, and OpenCode.
Each iteration runs in a fresh agent context until all tasks are done or max iterations are reached.

Operational details (CLI, MCP mode, and config) live in `OPERATIONS.md`.

Repository: [PromptExecution/b00t-wiggums](https://github.com/PromptExecution/b00t-wiggums)

## Prerequisites

- Python 3.11+
- `uv` installed
- One agent CLI installed/authenticated (`amp`, `claude`, `codex`, or `opencode`)
- A git repository for your project

## Install b00t-wiggums

```bash
git clone https://github.com/PromptExecution/b00t-wiggums.git
cd b00t-wiggums
uv sync
```

## Backlog Format

Ralph now reads work from `TODO-next.md` at the target repo root.
Use a markdown checklist as the primary backlog:

```bash
cat > TODO-next.md <<'EOF'
# Next
- [ ] Add schema support for feature X
- [ ] Implement API for feature X
- [ ] Verify behavior and update tests
EOF
```

⚠️ Legacy compatibility remains for `.taskmaster/tasks/tasks.json`, but that path is no longer the recommended setup.

## Optional Legacy Compatibility

If an older repo still uses TaskMaster, Ralph will fall back to `.taskmaster/tasks/tasks.json`.
New repos SHOULD NOT initialize TaskMaster just to run Ralph.

Example legacy MCP client config:

```json
{
  "mcpServers": {
    "taskmaster": {
      "command": "task-master",
      "args": ["mcp", "start"]
    },
    "ralph": {
      "command": "uv",
      "args": ["run", "ralph", "--mcp", "--transport", "stdio"]
    }
  }
}
```

## Install Skills (`/ralph-prd` and `/ralph`)

The PRD skill is now in `skills/ralph-prd/` and is invoked as `/ralph-prd`.

### Amp

```bash
mkdir -p ~/.config/amp/skills
cp -r skills/ralph-prd ~/.config/amp/skills/ralph-prd
cp -r skills/ralph ~/.config/amp/skills/ralph
```

### Claude Code

```bash
mkdir -p ~/.claude/skills
cp -r skills/ralph-prd ~/.claude/skills/ralph-prd
cp -r skills/ralph ~/.claude/skills/ralph
```

### Codex

```bash
mkdir -p ~/.codex/skills
cp -r skills/ralph-prd ~/.codex/skills/ralph-prd
cp -r skills/ralph ~/.codex/skills/ralph
```

## Quick Start

1) Generate backlog:

```text
Use the /ralph-prd skill to create a `TODO-next.md` backlog for [feature description]
```

2) Verify backlog exists:

```bash
cat TODO-next.md
```

3) Run Ralph:

```bash
uv run ralph run --tool codex --max-iterations 10
# or wrapper:
./ralph.sh --agent codex 10
```

4) Monitor progress:

```bash
uv run ralph status
uv run ralph list-tasks --filter pending
```

## MCP Mode (Ralph Server)

Run Ralph as an MCP server:

```bash
uv run ralph --mcp --transport stdio
# or HTTP:
uv run ralph --mcp --transport http --host 127.0.0.1 --port 8000
```

## Key Files

- `ralph.sh` - Wrapper with preflight checks (`uv sync`, backlog validation, gitignore checks)
- `TODO-next.md` - Primary backlog file used by Ralph
- `ralph/` - Python implementation and CLI
- `OPERATIONS.md` - Operational reference
- `.taskmaster/tasks/tasks.json` - Legacy compatibility backlog format
- `skills/ralph-prd/` - Source for the `/ralph-prd` skill
- `skills/ralph/` - Source for the `/ralph` conversion skill
- `flowchart/` - Interactive visualization source

## Flowchart

Flowchart source lives in `flowchart/`:

```bash
cd flowchart
npm install
npm run dev
```

Project home: [PromptExecution/b00t-wiggums](https://github.com/PromptExecution/b00t-wiggums)
