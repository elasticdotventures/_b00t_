# Ralph Python CLI justfile commands

# Run ralph with specified tool and iterations (10 iterations default)
ralph TOOL ITERATIONS='10':
    uv run ralph run --tool {{TOOL}} --max-iterations {{ITERATIONS}}

# Run ralph tests
ralph-test:
    uv run pytest tests/test_ralph*

# Type check and lint ralph module
ralph-check:
    uv run mypy --strict ralph/ && uv run ruff check ralph/

# Format ralph code with ruff
ralph-format:
    uv run ruff format ralph/

# Run all quality checks (format, lint, type check, test)
ralph-all: ralph-format ralph-check ralph-test
    @echo "✅ All quality checks passed!"

# Run ralph with amp tool
ralph-amp ITERATIONS='10':
    uv run ralph run --tool amp --max-iterations {{ITERATIONS}}

# Run ralph with claude tool
ralph-claude ITERATIONS='10':
    uv run ralph run --tool claude --max-iterations {{ITERATIONS}}

# Run ralph with codex tool
ralph-codex ITERATIONS='10':
    uv run ralph run --tool codex --max-iterations {{ITERATIONS}}

# Run ralph with opencode tool
ralph-opencode ITERATIONS='10':
    uv run ralph run --tool opencode --max-iterations {{ITERATIONS}}

# Show current task status
ralph-status:
    uv run ralph status

# List all tasks
ralph-tasks:
    uv run ralph list-tasks

# List pending tasks only
ralph-tasks-pending:
    uv run ralph list-tasks --filter pending

# List in-progress tasks only
ralph-tasks-active:
    uv run ralph list-tasks --filter in-progress

# Run ralph in dry-run mode (no actual execution)
ralph-dry-run TOOL='amp' ITERATIONS='10':
    uv run ralph run --tool {{TOOL}} --max-iterations {{ITERATIONS}} --dry-run

# Run ralph as MCP server (stdio transport)
ralph-mcp:
    uv run ralph --mcp

# Run ralph as MCP server (HTTP transport)
ralph-mcp-http PORT='8000':
    uv run ralph --mcp --transport http --port {{PORT}}
