---
name: b00t-integration
description: |
  Integration layer for b00t capabilities within opencode workflows.
  Provides access to b00t datum system, hive management, grok knowledge,
  session management, and task tracking directly from opencode.
version: 1.0.0
allowed-tools: All
---

## What This Skill Does

Exposes b00t capabilities to opencode agents:

- **datum-system** - Query and manage datums (*.datum files)
- **hive** - Resource management and profile switching  
- **grok** - Knowledge base queries (RAG)
- **session** - Session lifecycle management
- **task** - Task tracking and delegation
- **agent** - Agent messaging and delegation

## When It Activates

- User mentions "b00t" or "use b00t"
- Task involves datum lookup, hive status, grok queries
- Need session/task management within opencode workflows
- Delegation to b00t agents

## b00t CLI Integration

### Core Commands

```bash
# Hive management (resource gates)
b00t hive status           # RAM/GPU/CPU snapshot
b00t hive list             # Available profiles
b00t hive plan=<profile>   # Dry-run resource check
b00t hive activate=<profile>  # Switch system profile

# Datum queries
b00t advice <query>         # Get advice for errors
b00t lfmf <lesson>          # Memoize lesson learned

# Grok (knowledge base)
b00t grok ask <query>        # Query knowledge base
b00t grok learn <topic>      # Learn from URL/content

# Session management  
b00t session status          # Current session info
b00t session end           # End session gracefully

# Agent messaging
b00t agent discover        # Find available agents
b00t agent delegate       # Delegate to agent
```

### MCP Tools Available

When b00t-mcp is connected via opencode mcp:

- `b00t-mcp_b00t_hive_status` - System resource status
- `b00t-mcp_b00t_advice` - Get advice for patterns
- `b00t-mcp_b00t_grok_ask` - Query knowledge base
- `b00t-mcp_b00t_grok_learn` - Learn new content

## b00t Workflow Patterns

### Resource-Aware Execution

Before running heavy operations, check hive status:

```
1. b00t hive status → Get RAM/GPU availability
2. If insufficient → Request profile switch or warn
3. Proceed with execution
```

### Knowledge-Assisted Coding

When unsure about patterns:

```
1. b00t grok ask "<pattern> <language>" → Get learned examples
2. Apply pattern from knowledge base
3. b00t lfmf if novel lesson learned
```

### Datum-Backed Configuration

When configuring providers/models:

```
1. Query available datums
2. Load required environment variables
3. Validate configuration
```

## Integration Points

### opencode config (~/.config/opencode/opencode.json)

```json
{
  "mcp": {
    "b00t-mcp": {
      "enabled": true,
      "type": "local", 
      "command": ["/home/brianh/.local/bin/b00t-mcp", "stdio"]
    }
  }
}
```

### Context Files

- `.opencode/context/core/standards/code-quality.md` - b00t coding standards
- `.opencode/context/core/workflows/task-delegation-basics.md` - Delegation patterns

## Skills

- `datum-system` - Work with b00t datums
- `executive-role` - Agent orchestration
- `dry-philosophy` - DRY patterns

## Dependencies

- b00t-mcp MCP server (must be configured)
- b00t-cli installed
- b00t datums in datums/

## Version

1.0.0 - Initial integration skill