# Ralph (Python Package)

Ralph is a Python CLI tool that runs AI coding agents (amp, claude, codex, opencode) in an iterative loop until they complete their tasks or reach a maximum iteration limit. It uses `TODO-next.md` as the primary backlog, with legacy TaskMaster compatibility retained for older repos.

## Installation

Ralph uses `uv` for Python package management. No manual installation required - just use `uv run`:

```bash
# Run directly with uv
uv run ralph --help

# Or install globally (optional)
uv tool install --editable .
ralph --help
```

## Usage

### Basic Usage

Ralph now uses subcommands for better discoverability:

```bash
# Run agent loop (default: amp, 10 iterations)
uv run ralph run

# Run with specific tool and iteration count
uv run ralph run --tool claude --max-iterations 5
uv run ralph run --tool codex --max-iterations 20
uv run ralph run --tool opencode --max-iterations 10

# Check status
uv run ralph status

# List tasks
uv run ralph list-tasks
uv run ralph list-tasks --filter pending
uv run ralph list-tasks --filter in-progress
uv run ralph list-tasks --filter done

# Dry-run mode (show what would execute)
uv run ralph run --tool amp --max-iterations 5 --dry-run

# Verbose output
uv run ralph run --tool claude --verbose
```

### Using justfile Commands

The project includes convenient `just` commands:

```bash
# Run ralph with amp (default)
just ralph

# Run with specific tool
just ralph-amp
just ralph-claude
just ralph-codex
just ralph-opencode

# Check status and list tasks
just ralph-status
just ralph-tasks

# Dry-run mode
just ralph-dry-run

# Custom tool and iterations
just ralph amp 15
```

### Command-Line Options

```
usage: ralph [-h] [--version] {run,status,list-tasks} ...

Ralph - Autonomous AI agent loop runner

positional arguments:
  {run,status,list-tasks}
    run                 Run Ralph agent loop
    status              Show current task status
    list-tasks          List all tasks

options:
  -h, --help            show this help message and exit
  --version             show program's version number and exit

Examples:
  ralph run --tool amp --max-iterations 10
  ralph status
  ralph list-tasks --filter pending
```

#### Run Subcommand

```
usage: ralph run [-h] [--tool {amp,claude,codex,opencode}] [--max-iterations N]
                 [--task-id TASK_ID] [--dry-run] [--verbose]

options:
  --tool {amp,claude,codex,opencode}
                        Tool to run (default: amp)
  --max-iterations N    Maximum iterations (default: 10)
  --task-id TASK_ID     Run specific task only
  --dry-run             Show what would execute without running
  --verbose             Enable verbose output
```

## How It Works

1. **Initialization**: Ralph reads `TODO-next.md` and `progress.txt` from the project root
2. **Backlog Integration**: Uses `TODO-next.md` first and legacy TaskMaster data as fallback
3. **Visual Progress**: Displays Unicode progress bar and task tree with status icons
4. **Branch Detection**: Checks if git branch has changed since last run
5. **Archival**: If branch changed, archives previous run to `archive/{date}-{branch-name}/`
6. **Task Selection**: Picks highest priority task with `status: "pending"` and no blockers
7. **Status Update**: Sets task to `"in-progress"` before execution
8. **Iteration Loop**: Runs the selected tool (amp/claude/codex/opencode) repeatedly
9. **Completion Detection**: Monitors tool output for `<promise>COMPLETE</promise>` signal
10. **Status Update**: Sets completed task to `"done"`
11. **Progress Tracking**: Updates `progress.txt` after each iteration

## Supported Tools

### amp
Runs `amp --dangerously-allow-all` reading from `prompt.md`

### claude
Runs `claude --model sonnet --dangerously-skip-permissions --print < CLAUDE.md`

### codex
Runs `codex exec` with full configuration (see Environment Variables below)

### opencode
Runs `opencode --model {model} < CLAUDE.md` with configurable model (see Environment Variables below)

## Environment Variables

Ralph supports the following environment variables for configuration:

| Variable | Description | Default |
|----------|-------------|---------|
| `CODEX_PROMPT_FILE` | Prompt file for codex | `CLAUDE.md` |
| `CODEX_MODEL` | Codex model to use | `gpt-5-codex` |
| `CODEX_REASONING_EFFORT` | Reasoning effort level | `high` |
| `CODEX_SANDBOX` | Sandbox mode | `workspace-write` |
| `CODEX_FULL_AUTO` | Enable full automation | `true` |
| `CODEX_EXTRA_ARGS` | Additional codex arguments | (empty) |
| `OPENCODE_MODEL` | OpenCode model to use | `opencode-default` |
| `OPENCODE_EXTRA_ARGS` | Additional opencode arguments | (empty) |
| `TASKMASTER_URL` | Legacy TaskMaster server URL (if using MCP) | (empty, uses file-based) |

### Example: Custom Codex Configuration

```bash
export CODEX_MODEL="gpt-4-turbo"
export CODEX_REASONING_EFFORT="medium"
export CODEX_FULL_AUTO="false"
uv run ralph --tool codex 15
```

## Backlog Integration

Ralph is backlog-first. The preferred operator format is `TODO-next.md` with markdown checklist items.

```markdown
# Next
- [ ] Add schema support
- [ ] Implement API
- [ ] Validate behavior
```

Ralph still supports [TaskMaster-AI](https://github.com/taskmaster-ai/taskmaster) data for older repos.

### Primary Task Format

Checklist items in `TODO-next.md` become Ralph tasks automatically. Each unchecked item is pending; each checked item is done.

### Legacy TaskMaster Format

Older repos may still use `tasks.json` with the following structure:

```json
{
  "tasks": [
    {
      "id": "task-001",
      "title": "Short task title",
      "description": "Full user story: As a [role], I need [capability], so that [benefit]...",
      "status": "pending",
      "priority": 1,
      "acceptanceCriteria": ["Criterion 1", "Criterion 2"],
      "dependsOn": ["task-000"],
      "blockedBy": [],
      "subtasks": [],
      "notes": ["2026-02-01: Progress note"],
      "createdAt": "2026-02-01T10:00:00Z",
      "updatedAt": "2026-02-01T10:00:00Z"
    }
  ],
  "metadata": {
    "project": "MyProject",
    "branchName": "feature/my-feature",
    "description": "Feature description",
    "taskMasterVersion": "1.0"
  }
}
```

### Task Status Lifecycle

Tasks progress through three states:
1. **pending** - Not started, ready to work on
2. **in-progress** - Currently being implemented
3. **done** - Successfully completed

Ralph automatically manages status transitions:
- Sets task to `in-progress` before starting work
- Updates to `done` after successful completion
- Only one task is `in-progress` at a time

### Dependency Management

Use `dependsOn` to express task dependencies:

```json
{
  "id": "task-003",
  "title": "Add UI for feature",
  "dependsOn": ["task-001", "task-002"],
  "status": "pending"
}
```

Ralph will skip tasks with unmet dependencies (dependencies not yet `done`).

Use `blockedBy` to track active blockers:

```json
{
  "id": "task-005",
  "title": "Deploy feature",
  "blockedBy": ["Need production credentials"],
  "status": "pending"
}
```

Ralph skips tasks with non-empty `blockedBy` arrays.

### Visual Progress Display

Ralph shows rich terminal output with Unicode box-drawing characters:

```
