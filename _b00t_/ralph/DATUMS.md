# Ralph Datum Ontology

Datum ontology for ralph autonomous agent capability integration with b00t.

## Datum Types

Ralph capability is composed of multiple datum types that work together:

### 1. Agent Datum (`ralph.agent.toml`)

**Location**: `_b00t_/ralph.agent.toml`

**Purpose**: Agent coordination metadata for b00t agent system

**Key Sections**:
- `[b00t.agent]` - Agent identity, skills, personality, role
- `[b00t.agent.ipc]` - Inter-process communication config
- `[b00t.agent.crew]` - Multi-agent crew role
- `[b00t.agent.executor]` - Ralph-specific executor config
- `[b00t.agent.mcp]` - MCP server capabilities
- `[b00t.env]` - Environment variables

**Integration**: Used by `b00t-cli agent start` command

### 2. CLI Datum (`ralph.cli.toml`)

**Location**: `_b00t_/ralph.cli.toml`

**Purpose**: CLI tool installation and version management

**Key Sections**:
- `[b00t.cli]` - Command, version detection, installation
- `[b00t.capabilities]` - Feature flags and requirements
- `[b00t.env]` - Environment variables

**Integration**: Used by `b00t-cli install ralph` and `b00t-cli up`

### 3. MCP Datum (`ralph.mcp.toml`)

**Location**: `_b00t_/ralph.mcp.toml`

**Purpose**: MCP server configuration for different transports

**Key Sections**:
- `[b00t.mcp]` - Protocol version
- `[b00t.mcp.server.stdio]` - Standard I/O transport
- `[b00t.mcp.server.http]` - HTTP transport
- `[b00t.mcp.capabilities]` - Tools, prompts, resources
- `[b00t.install]` - Installation dependencies

**Integration**: Used by `b00t-cli mcp install ralph <target>`

### 4. Stack Datum (`ralph-stack.stack.toml`)

**Location**: `_b00t_/ralph-stack.stack.toml`

**Purpose**: Complete capability stack definition

**Key Sections**:
- `[b00t.stack]` - Stack description and category
- `[b00t.stack.components]` - Component datums and requirements
- `[b00t.stack.features]` - Feature flags
- `[b00t.install]` - Installation order

**Integration**: Used by `b00t-cli stack install ralph-stack`

## Datum Relationships

```
ralph-stack (stack)
├── ralph.agent (agent) ← Agent coordination
├── ralph.cli (cli) ← CLI installation
├── ralph.mcp (mcp) ← MCP server
├── b00t job/backlog (runtime) ← Primary task source
├── taskmaster.cli (cli) ← Legacy compatibility
├── taskmaster.mcp (mcp) ← Legacy optional dependency
├── python.cli (cli) ← Runtime requirement
├── uv.cli (cli) ← Package manager
└── [executor].cli (cli) ← Optional: amp|claude|codex|opencode
```

## Skills Ontology

Ralph agent provides the following skills, mapped to datum capabilities:

| Skill | Datum | Capability |
|-------|-------|------------|
| backlog-planning | ralph.cli | Backlog CRUD operations |
| autonomous-loop | ralph.cli | Self-directed iteration |
| prd-parsing | ralph.cli | Requirements → tasks |
| workflow-execution | ralph.cli | Task implementation |

## MCP Tools Ontology

Ralph MCP server exposes:

| Tool | Capability | Parameters |
|------|------------|------------|
| ralph_run | Start autonomous loop | tool, max-iterations, filter |
| ralph_status | Check execution status | - |
| ralph_list_tasks | List Ralph backlog tasks | filter |

| Prompt | Purpose |
|--------|---------|
| /ralph-prd | Generate tasks from PRD |
| /ralph | Convert tasks to ralph format |

| Resource | Access |
|----------|--------|
| ralph://tasks | Ralph task data |
| ralph://progress | Execution progress log |

## Role-Based Configuration

Ralph can be configured for different roles via `.agent.toml`:

### Executor Role (Default)

```toml
[b00t.agent.crew]
role = "executor"
captain = false
```

- Receives delegated tasks
- Reports completion to captain
- Autonomous within task scope

### Captain Role (Advanced)

```toml
[b00t.agent.crew]
role = "captain"
captain = true
```

- Delegates tasks to other agents
- Coordinates multi-agent workflows
- Makes strategic decisions

### Specialist Role

```toml
[b00t.agent.crew]
role = "specialist"
captain = false

[b00t.agent]
skills = ["backlog-planning", "prd-parsing"]
```

- Focused on specific capabilities
- Can be consulted by other agents
- Expert in skill domain

## Installation Patterns

### Pattern 1: Full Stack

```bash
b00t-cli stack install ralph-stack
```

Installs all components in dependency order.

### Pattern 2: CLI Only

```bash
b00t-cli install ralph
```

Installs ralph CLI without agent/MCP integration.

### Pattern 3: MCP Integration

```bash
b00t-cli mcp install ralph claudecode
```

Installs ralph MCP server to Claude Code.

### Pattern 4: Agent Coordination

```bash
b00t-cli agent start _b00t_/ralph.agent.toml
```

Starts ralph as coordinated agent in multi-agent crew.

## Environment Variables

Ralph respects the following environment variables from datums:

| Variable | Source | Purpose |
|----------|--------|---------|
| RALPH_HOME | All datums | Ralph installation path |
| AGENT_ID | ralph.agent.toml | Agent identifier |
| AGENT_SKILLS | ralph.agent.toml | Comma-separated skills |

## Extension Points

Ralph capability can be extended via:

1. **Custom Executors** - Add new executor tools to `[b00t.agent.executor]`
2. **Additional Skills** - Add skills to `[b00t.agent.skills]`
3. **MCP Tools** - Extend `[b00t.mcp.capabilities.tools]`
4. **Stack Components** - Add datums to `[b00t.stack.components]`

## Datum Discovery

B00t discovers ralph datums via:

1. **File Pattern**: `_b00t_/ralph*.toml`
2. **Type Field**: `type = "agent"|"cli"|"mcp"|"stack"`
3. **Name Field**: `name = "ralph"|"ralph-stack"`

## Related Datums

Ralph integrates with these existing datums:

- `taskmaster-ai.mcp.toml` - Legacy TaskMaster MCP compatibility datum
- `alpha.agent.toml` - Example agent configuration
- `beta.agent.toml` - Example agent configuration
- `ai-dev-stack.stack.toml` - Example stack configuration

---

🥾 Generated via b00t gospel alignment
🤓 Ralph datum ontology provides comprehensive capability mapping
