# Codex Integration Setup for b00t

## Overview
Complete integration of OpenAI Codex (GPT-5/GPT-5-Codex) with b00t operator and Claude Code workspace.

## Components Installed

### 1. Codex CLI
- **Command**: `codex`
- **Version**: codex-cli 0.57.0
- **Location**: Global installation via npm
- **Purpose**: Execute GPT-5 code analysis, refactoring, and editing tasks

### 2. Codex MCP Servers (2 servers)

#### a) Official Codex MCP Server
- **Name**: `codex-gpt5`
- **Command**: `codex mcp-server`
- **Purpose**: Native Codex MCP protocol server
- **Tools**:
  - `codex`: Start new Codex session
  - `codex-reply`: Continue existing conversation
- **Registration**: `b00t mcp register codex-gpt5 -- codex mcp-server`
- **Installed to**: Claude Code via `b00t mcp install codex-gpt5 claudecode`

#### b) Third-Party Codex MCP Tool
- **Name**: `codex-mcp-tool`
- **Package**: `@trishchuk/codex-mcp-tool@1.2.0`
- **Command**: `npx -y @trishchuk/codex-mcp-tool`
- **Purpose**: Alternative MCP integration with additional capabilities
- **Registration**: `b00t mcp register codex-mcp-tool -- npx -y @trishchuk/codex-mcp-tool`
- **Installed to**: Claude Code via `b00t mcp install codex-mcp-tool claudecode`

### 3. Codex Skill for Claude Code
- **Source**: https://github.com/skills-directory/skill-codex
- **Location**: `/home/brianh/promptexecution/app4dog/skill-codex/`
- **Installation Target**: `~/.claude/skills/codex/`
- **Files**:
  - `SKILL.md`: Operational instructions with YAML frontmatter
  - `README.md`: User-facing documentation

## Skill Installation

```bash
# Clone to temporary location
git clone --depth 1 git@github.com:skills-directory/skill-codex.git /tmp/skills-temp

# Install to Claude Code skills directory
mkdir -p ~/.claude/skills
cp -r /tmp/skills-temp/ ~/.claude/skills/codex
rm -rf /tmp/skills-temp
```

## Usage Patterns

### Via Codex CLI
```bash
# Direct execution
codex exec -m gpt-5-codex \
  --config model_reasoning_effort="high" \
  --sandbox read-only \
  --full-auto \
  --skip-git-repo-check \
  "Analyze codebase" 2>/dev/null

# Resume session
echo "Continue analysis" | codex exec --skip-git-repo-check resume --last 2>/dev/null
```

### Via MCP (Official Server)
```json
{
  "tool": "codex",
  "parameters": {
    "prompt": "Analyze authentication system",
    "approval-policy": "on-failure",
    "sandbox": "read-only",
    "model": "gpt-5-codex"
  }
}
```

### Via Claude Code Skill
User prompt: "Use codex to analyze this repository"

Skill activates and:
1. Asks for model choice (gpt-5-codex or gpt-5)
2. Asks for reasoning effort (low, medium, high)
3. Selects appropriate sandbox mode
4. Executes with filtered output (stderr suppressed by default)

## Sandbox Modes

| Mode | Use Case | Risk Level |
|------|----------|------------|
| `read-only` | Analysis, review, documentation | Low |
| `workspace-write` | Local edits, refactoring | Medium |
| `danger-full-access` | Network access, external APIs | High |

## Model Options

- **gpt-5-codex**: Optimized for code tasks (recommended)
- **gpt-5**: General reasoning with code capabilities

## Reasoning Effort Levels

- **low**: Fast, basic analysis (~30s-1min)
- **medium**: Balanced thoroughness (~1-3min)
- **high**: Deep analysis with extensive reasoning (~3-10min)

## Key Flags

- `--full-auto`: Non-interactive execution
- `--skip-git-repo-check`: Bypass git validation
- `2>/dev/null`: Suppress thinking tokens (stderr)
- `-C <DIR>`: Execute in specific directory
- `--config model_reasoning_effort="<level>"`: Set reasoning depth

## Integration with b00t

### Agent Communication Pattern

Codex can be invoked by b00t agents using:

1. **Direct CLI execution** via bash tools
2. **MCP tools** via b00t-mcp codex servers
3. **Agent Coordination Protocol (ACP)** messages

### Example: Agent → Codex Workflow

```bash
# Agent sends task via b00t chat
b00t-cli chat send \
  --channel codex-tasks \
  --message '{"type":"PROPOSE","action":"codex-analyze","target":"src/auth/"}'

# Codex worker receives and executes
codex exec --sandbox read-only "Analyze src/auth/" 2>/dev/null

# Worker reports back via ACP
b00t-cli chat send \
  --channel codex-tasks \
  --message '{"type":"STATUS","status":"complete","result":"..."}'
```

## Session Management

### Create Session
```bash
codex exec "Initial task" --sandbox workspace-write
# Session ID returned: e.g., abc123
```

### Resume Session
```bash
# Option 1: Explicit session ID
echo "Continue work" | codex exec resume abc123

# Option 2: Last session
echo "Continue work" | codex exec resume --last
```

### Session Persistence
- Sessions stored in `~/.codex/sessions/`
- Include conversation history, sandbox mode, model settings
- Can be resumed days/weeks later

## Best Practices

1. **Suppress thinking tokens** by default (`2>/dev/null`)
2. **Always use** `--skip-git-repo-check` to avoid validation overhead
3. **Start with read-only** sandbox, escalate only if needed
4. **Use high reasoning effort** for critical analysis
5. **Resume sessions** instead of starting fresh
6. **Capture thread IDs** for long-running workflows
7. **Filter status messages** when using via IPC/pipes

## Troubleshooting

### Codex not found
```bash
# Verify installation
codex --version
# Should show: codex-cli 0.57.0
```

### MCP server not responding
```bash
# Test MCP server directly
npx @modelcontextprotocol/inspector codex mcp-server
# Set timeout to 600000ms (10 minutes)
```

### Skill not activating
```bash
# Verify skill installation
ls -la ~/.claude/skills/codex/
# Should show: SKILL.md, README.md
```

## Architecture Diagram

```
┌─────────────────────────────────────────────────────┐
│                 Claude Code (User)                  │
│                                                     │
│  ┌──────────────────────────────────────────────┐  │
│  │          Codex Skill (SKILL.md)              │  │
│  │  • Asks model/reasoning questions            │  │
│  │  • Builds codex exec commands                │  │
│  │  • Filters stderr output                     │  │
│  └──────────────────────────────────────────────┘  │
│                        ▼                            │
│  ┌──────────────────────────────────────────────┐  │
│  │       MCP Servers (via b00t-mcp)             │  │
│  │  • codex-gpt5 (official)                     │  │
│  │  • codex-mcp-tool (@trishchuk)               │  │
│  └──────────────────────────────────────────────┘  │
│                        ▼                            │
└─────────────────────────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────┐
│              Codex CLI (codex exec)                 │
│  • OpenAI GPT-5 / GPT-5-Codex                      │
│  • Sandbox enforcement                              │
│  • Session management                               │
│  • Thinking tokens on stderr                       │
└─────────────────────────────────────────────────────┘
                         ▼
┌─────────────────────────────────────────────────────┐
│                  Workspace Files                    │
│  • Read-only: Analysis, suggestions                │
│  • Workspace-write: Local edits                    │
│  • Danger-full: Network + filesystem               │
└─────────────────────────────────────────────────────┘
```

## Next Steps: /k0mmand3r Interface

See `/home/brianh/.dotfiles/k0mmand3r_interface.md` for agent command protocol integration.
