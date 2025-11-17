# b00t LangChain Agent Service

LangChain v1.0 agent service with MCP tool discovery and k0mmand3r IPC integration.

## Overview

The LangChain Agent Service provides a "helm chart for thought" - pre-built agent components (researcher, coder, coordinator) with dynamic MCP tool discovery. Agents respond to slash commands via Redis pub/sub and can invoke other agents as tools.

## Architecture

```
┌─ LangChain Agent Stack ──────────────────────────┐
│                                                    │
│  Redis (b00t:langchain channel)                   │
│         ↓                                          │
│  k0mmand3r Listener → LangChain Agent Service     │
│         ↓                      ↓                   │
│  Slash Commands          Agent Service            │
│  (/agent, /chain)              ↓                   │
│                          MCP Tool Discovery        │
│                          (FastMCPMulti)            │
│                                ↓                   │
│                    ┌──────────────────────┐       │
│                    │  MCP Servers:        │       │
│                    │  - crawl4ai          │       │
│                    │  - github            │       │
│                    │  - grok              │       │
│                    │  - sequential-think  │       │
│                    │  - taskmaster        │       │
│                    └──────────────────────┘       │
│                                                    │
└────────────────────────────────────────────────────┘
```

## Features

- **Agent Presets**: Researcher, Coder, Coordinator with pre-configured tools
- **Dynamic Tool Discovery**: Auto-loads MCP servers from b00t datums
- **Cross-Agent Communication**: Agents can invoke other agents as tools
- **Middleware System**: Human-in-loop, summarization, PII redaction
- **k0mmand3r Integration**: Responds to slash commands via Redis
- **LangSmith Tracing**: Built-in observability

## Installation

### Via uv (recommended)

```bash
# Install dependencies
uv sync

# Install playwright browsers (for crawl4ai)
uv run playwright install chromium

# Run service
uv run b00t-langchain serve
```

### Manual (not recommended)

```bash
# Use uv instead - see above
uv sync
uv run playwright install chromium
uv run b00t-langchain serve
```

## Configuration

Configuration is loaded from `~/.dotfiles/_b00t_/langchain.ai.toml`:

```toml
[b00t]
name = "langchain"
type = "ai"

[b00t.env]
required = ["ANTHROPIC_API_KEY"]

[langchain.agents.researcher]
model = "anthropic/claude-sonnet-4"
tools = ["crawl4ai-mcp", "github-mcp", "grok"]
middleware = ["summarization"]
system_prompt = "You are a technical researcher..."
```

### Environment Variables

```bash
# Required
export ANTHROPIC_API_KEY="sk-ant-..."

# Optional (for tracing)
export LANGCHAIN_API_KEY="lsv2_pt_..."
export LANGSMITH_API_KEY="lsv2_sk_..."
export LANGCHAIN_TRACING_V2="true"
export LANGCHAIN_PROJECT="b00t"

# Service configuration
export REDIS_URL="redis://localhost:6379"
export LANGCHAIN_COMMAND_CHANNEL="b00t:langchain"
export _B00T_Path="~/.dotfiles/_b00t_"
```

## Usage

### Production Deployment with PM2 (Recommended)

For production use, deploy as a PM2-managed service with auto-restart and monitoring:

```bash
# Quick start
just deploy

# Or step by step:
just install  # Install dependencies
just start    # Start PM2 service
just save     # Save PM2 process list

# Monitor
just status   # Show service status
just logs     # Tail logs
just monitor  # Real-time monitoring

# Manage
just restart  # Restart service
just stop     # Stop service
just delete   # Remove from PM2
```

**Health Check**:
```bash
just health-check
# Or directly:
uv run python healthcheck.py
```

**Auto-start on System Boot**:
```bash
just setup-startup
# Follow instructions, then:
just save
```

### Development Mode

For development without PM2:

```bash
# Via uv
uv run b00t-langchain serve

# With custom Redis
uv run b00t-langchain serve --redis-url redis://localhost:6380

# With custom channel
uv run b00t-langchain serve --channel b00t:my-channel

# Or via just
just dev
```

### Slash Commands

#### Create and Run Agent

```bash
# Via Redis CLI
redis-cli PUBLISH b00t:langchain '{
  "verb": "agent",
  "params": {
    "action": "run",
    "name": "researcher",
    "input": "Research LangChain v1.0 features"
  }
}'
```

#### Execute Chain

```bash
redis-cli PUBLISH b00t:langchain '{
  "verb": "chain",
  "params": {
    "action": "run",
    "name": "research-and-digest",
    "url": "https://docs.langchain.com/oss/python/releases/langchain-v1"
  }
}'
```

#### Cross-Agent Communication

```bash
# Agent broadcasts to other agents
redis-cli PUBLISH b00t:langchain '{
  "verb": "agent",
  "params": {
    "action": "broadcast",
    "message": "Status update?",
    "from": "coordinator"
  }
}'
```

### Test Agent (No Redis)

```bash
# Run agent directly for testing
uv run b00t-langchain test-agent researcher "What is LangChain v1.0?"

# With different model
uv run b00t-langchain test-agent coder "How do I implement async Rust?" --model gpt-4o
```

### List Available Tools

```bash
uv run b00t-langchain list-tools
```

## Agent Presets

### Researcher

```python
# Configured in langchain.ai.toml
[langchain.agents.researcher]
model = "anthropic/claude-sonnet-4"
tools = ["crawl4ai-mcp", "github-mcp", "grok"]
middleware = ["summarization"]
system_prompt = "You are a technical researcher..."
```

**Usage:**
```bash
/agent run --name=researcher --input="Research Rust async patterns"
```

### Coder

```python
[langchain.agents.coder]
model = "anthropic/claude-sonnet-4"
tools = ["github-mcp", "sequential-thinking-mcp"]
middleware = ["human-in-loop"]
system_prompt = "You are a Rust/TypeScript expert..."
```

**Usage:**
```bash
/agent run --name=coder --input="Implement PM2 integration tests"
```

### Coordinator

```python
[langchain.agents.coordinator]
model = "anthropic/claude-sonnet-4"
tools = ["taskmaster-mcp", "sequential-thinking-mcp"]
middleware = ["summarization", "human-in-loop"]
peer_agents = ["researcher", "coder"]  # Can invoke other agents
```

**Usage:**
```bash
/agent run --name=coordinator --input="Plan and implement LangChain integration"
```

## MCP Tool Discovery

The service automatically discovers MCP tools from configured servers:

```toml
[langchain.mcp.servers]
crawl4ai = { transport = "docker", url = "http://localhost:8001/mcp" }
github = { transport = "http", url = "http://localhost:8002/mcp" }
grok = { transport = "stdio", command = "b00t-mcp", args = ["grok"] }
```

**Discovery Process:**
1. Read MCP server configs from datum
2. Connect to each server (HTTP/SSE or stdio)
3. Fetch tool list via `list_tools`
4. Convert JSON-Schema → Pydantic → LangChain BaseTool
5. Make tools available to agents

## Middleware

### Human-in-Loop

```toml
[langchain.middleware.human-in-loop]
enabled = true
redis_channel = "b00t:human-approval"
timeout_seconds = 300
approval_required_for = ["file_write", "git_commit"]
```

Pauses agent execution for human approval on critical actions.

### Summarization

```toml
[langchain.middleware.summarization]
enabled = true
model = "anthropic/claude-haiku-3"
threshold_tokens = 4000
```

Automatically summarizes context when it exceeds threshold.

### PII Redaction

```toml
[langchain.middleware.pii-redaction]
enabled = false
patterns = ["email", "phone", "ssn", "api_key"]
```

Removes sensitive data from agent inputs/outputs.

## Development

### Project Structure

```
langchain-agent/
├── src/b00t_langchain_agent/
│   ├── __init__.py
│   ├── main.py              # CLI entry point
│   ├── agent_service.py     # Agent management
│   ├── mcp_tools.py         # MCP tool discovery
│   ├── k0mmand3r.py         # Redis IPC listener
│   └── types.py             # Pydantic models
├── tests/
│   ├── test_agent.py
│   ├── test_mcp.py
│   └── test_ipc.py
├── pyproject.toml
├── README.md
└── Dockerfile
```

### Running Tests

```bash
# All tests
uv run pytest

# With coverage
uv run pytest --cov=b00t_langchain_agent

# Specific test
uv run pytest tests/test_agent.py::test_researcher_agent
```

### Linting & Type Checking

```bash
# Lint
uv run ruff check .

# Format
uv run ruff format .

# Type check
uv run mypy src/
```

## Docker

```bash
# Build
docker build -t b00t-langchain-agent:latest .

# Run
docker run -d \
  --name langchain-agent \
  --network b00t-network \
  -e REDIS_URL=redis://redis:6379 \
  -e ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY \
  b00t-langchain-agent:latest
```

## Comparison: LangChain vs PM2 Tasker

| Feature | PM2 Tasker | LangChain Agent |
|---------|------------|-----------------|
| **Purpose** | Process management | Agent reasoning |
| **Runtime** | Node.js processes | Python LLM agents |
| **Tools** | Shell commands | MCP tools (any language) |
| **State** | Process status | Conversation memory |
| **Clustering** | Multiple instances | Multiple agents (crew) |
| **Commands** | /start, /stop | /agent, /chain |

**Together:** PM2 manages the LangChain service process; LangChain manages agent reasoning.

## Troubleshooting

### "No module named 'langchain'"

```bash
uv sync
```

### "Connection refused" to Redis

```bash
redis-cli ping  # Check Redis is running
```

### "ANTHROPIC_API_KEY not set"

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

### MCP tools not discovered

1. Check MCP servers are running
2. Verify datum configurations in `langchain.ai.toml`
3. Check logs for connection errors

## References

- [LangChain v1.0 Docs](https://docs.langchain.com/oss/python/releases/langchain-v1)
- [LangGraph](https://langchain-ai.github.io/langgraph/)
- [DeepMCPAgent](https://github.com/cryxnet/deepmcpagent)
- [MCP Protocol](https://modelcontextprotocol.io/)
- [b00t Documentation](../../README.md)
