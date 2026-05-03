# Ralph MCP

Ralph supports **FastMCP 3.0** with both server and client capabilities. Current task management is backlog-first via `TODO-next.md`; TaskMaster integration is legacy compatibility only.

## Features

### MCP Server Mode
Ralph exposes its autonomous agent capabilities as an MCP server with:

**Tools:**
- `run_ralph_iteration(agent, max_iterations, prd_path)` - Run Ralph for N iterations
- `get_ralph_status()` - Get current execution status from progress.txt
- `get_prd_status(prd_path)` - Get PRD completion metrics

**Resources:**
- `ralph://prd` - Current PRD JSON content
- `ralph://progress` - Current progress.txt log

### Usage

**As CLI (default):**
```bash
# Using new Python CLI with subcommands
uv run ralph run --tool codex --max-iterations 3

# Or using deprecated bash wrapper
./ralph.sh --agent codex 3
```

**As MCP Server (HTTP):**
```bash
uv run --script ralphython.py --mcp --transport http --port 8000
```

**As MCP Server (stdio):**
```bash
uv run --script ralphython.py --mcp --transport stdio
```

### Client Example
```python
import asyncio
from fastmcp import Client

async def check_ralph():
    async with Client("http://localhost:8000/mcp") as client:
        # Get PRD status
        status = await client.call_tool("get_prd_status", {})
        print(f"Completed: {status['completed_stories']}/{status['total_stories']}")
        
        # Run Ralph for 1 iteration
        result = await client.call_tool("run_ralph_iteration", {
            "agent": "codex",
            "max_iterations": 1
        })
        print(f"Exit code: {result['exit_code']}")

asyncio.run(check_ralph())
```

### MCP Client Config
Add to your MCP client settings (e.g., Claude Desktop):

```json
{
  "mcpServers": {
    "ralph": {
      "command": "uv",
      "args": ["run", "--script", "/path/to/ralphython.py", "--mcp"]
    }
  }
}
```

## Implementation Details

- FastMCP 3.0.0b1 (beta)
- PEP 723 inline script dependencies
- Supports both CLI and MCP modes via `--mcp` flag
- Model: gpt-5.2-codex (configurable via `CODEX_MODEL` env var)

## Legacy TaskMaster MCP Integration

Ralph still supports TaskMaster-backed repos via MCP protocol, but new setups SHOULD use `TODO-next.md` and Ralph's built-in file handling instead.

### Setup TaskMaster MCP Server

**Option 1: File-Based (Default)**

No setup required. Ralph reads/writes `TODO-next.md` directly:

```bash
# Works out of the box
uv run ralph status
uv run ralph list-tasks
uv run ralph run --tool amp
```

**Option 2: Legacy TaskMaster MCP Server**

Connect to a TaskMaster MCP server for remote task management:

```bash
# Set TaskMaster server URL for a legacy repo
export TASKMASTER_URL="http://localhost:8080"

# Ralph will use MCP instead of file-based access
uv run ralph status
uv run ralph run --tool amp
```

### Legacy TaskMaster CLI Integration

Ralph also supports the TaskMaster CLI for task operations:

```bash
# Install TaskMaster CLI
npm install -g @taskmaster-ai/cli

# Ralph will detect and use it automatically
uv run ralph status
uv run ralph list-tasks
```

### MCP Client Config for Legacy TaskMaster

Add TaskMaster to your MCP client settings (e.g., Claude Desktop):

```json
{
  "mcpServers": {
    "taskmaster": {
      "command": "task-master",
      "args": ["mcp", "start"]
    },
    "ralph": {
      "command": "uv",
      "args": ["run", "ralph", "mcp", "start"],
      "env": {
        "TASKMASTER_URL": "mcp://taskmaster"
      }
    }
  }
}
```

### Backlog Features in Ralph

1. **Checklist Backlog Management**: `- [ ]` → `- [x]`
2. **Legacy JSON Compatibility**: TaskMaster JSON still loads when present
3. **Visual Progress**: Unicode progress bars and task trees
4. **CLI Commands**: `status`, `list-tasks` with filtering
5. **Hybrid Mode**: `TODO-next.md` first, legacy TaskMaster second

### Example: Using Ralph with a Backlog

```bash
# Check current status
uv run ralph status

# Expected output:
# ==================================================
# Progress: [████████████░░░░░░░░] 60.0%
# Completed: 3/5 | In Progress: 1 | Pending: 1 | Blocked: 0
# ==================================================

# List pending tasks
uv run ralph list-tasks --filter pending

# Run autonomous loop
uv run ralph run --tool amp --max-iterations 10
```

### Migration Notes

Ralph has moved from `prd.json` and TaskMaster-driven setup toward `TODO-next.md` as the primary operator-facing format.
Legacy `.taskmaster/tasks/tasks.json` remains supported for older repos.
