---
name: b00t-rhai-bridge
description: |
  Bridge b00t primitives (datum, grok, hive, task, agent) to opencode via RHAI scripts.
  RHAI provides the idiomatic abstraction layer connecting applications.
version: 1.0.0
allowed-tools: All
---

## What This Skill Does

Expose b00t primitives to opencode through RHAI script runner:
- Execute b00t RHAI scripts from opencode
- Create custom RHAI scripts for opencode workflows
- Bridge b00t capabilities to opencode agents

## When It Activates

- Task needs b00t datum queries
- Workflow needs grok knowledge base
- Resource management via hive profiles
- Custom automation via RHAI scripts

## b00t Primitives → RHAI Bridge

### Available RHAI Scripts

```bash
# List available scripts
b00t script list

# Run a script
b00t script run <script-name>

# Run RHAI code directly
b00t script eval "<rhai code>"
```

### Key RHAI Scripts

| Script | Purpose | Bridge |
|--------|---------|--------|
| operator-agent | Autonomous agent loop | opencode agent delegation |
| setup | System bootstrap | opencode initialization |
| github-auth-validate | GitHub auth check | MCP credential validation |
| kubernetes_detect | k8s detection | opencode k8s context |

### RHAI in opencode Workflow

```
1. b00t script run operator-agent → Execute autonomous loop
2. b00t script eval "<code>" → Run inline RHAI
3. Delegate to b00t agent → Full agent capability
```

## RHAI Code Examples

### Check System Status

```rhai
let status = b00t::hive_status();
print(status);
```

### Query Knowledge Base

```rhai
let answer = b00t::grok_ask("pattern TypeScript");
print(answer);
```

### Delegate to Agent

```rhai
let result = b00t::agent_delegate("worker", "task-id", "description");
print(result);
```

## Integration with opencode

### Via b00t-mcp

When b00t-mcp is configured:

```javascript
// Direct CLI invocation
b00t_mcp_b00t_hive_status()

// Available in opencode agents
```

### Via shell commands in opencode

```bash
# In opencode agent, execute:
b00t script run github-auth-validate
b00t grok ask "<query>"
b00t task list
```

## Custom RHAI Scripts for opencode

Create in `_b00t_/scripts/` for opencode integration:

```rhai
// opencode-init.rhai
print("Initializing opencode context...");

// Load opencode standards
let standards = datum_load("opencode-standards");
print(standards);
```

## Dependencies

- b00t-cli installed
- RHAI scripts in `_b00t_/scripts/`
- b00t-mcp (for MCP bridge)

## Version

1.0.0 - Initial RHAI bridge skill