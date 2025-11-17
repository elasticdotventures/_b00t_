# LangChain Integration Architecture

## Overview

LangChain v1.0 + MCP + k0mmand3r integration for b00t ecosystem. Provides "helm chart for thought" - pre-built agent components with dynamic MCP tool discovery.

## Architecture

```
┌─ b00t LangChain Agent Stack ─────────────────────┐
│                                                    │
│  Redis (b00t:langchain channel)                   │
│         ↓                                          │
│  k0mmand3r Listener → LangChain Agent Service     │
│         ↓                      ↓                   │
│  Slash Commands          LangGraph Runtime        │
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

## Key Components

### 1. LangChain v1.0 Features

**create_agent Abstraction:**
```python
from langchain.agents import create_agent

agent = create_agent(
    model=ChatAnthropic(model="claude-sonnet-4"),
    tools=mcp_tools,  # Dynamically discovered from MCP servers
    middleware=[
        HumanInLoopMiddleware(),
        SummarizationMiddleware(),
        PIIRedactionMiddleware(),
    ]
)
```

**Middleware System:**
- Human-in-the-loop: Pause for approval on critical actions
- Summarization: Condense long contexts
- PII Redaction: Remove sensitive data

**LangGraph Integration:**
- Built on LangGraph runtime for stateful agents
- ReAct pattern (think-act-observe)
- Checkpoint/resume support

### 2. MCP Tool Discovery (from deepmcpagent)

**Dynamic Tool Loading:**
```python
from deepmcpagent import FastMCPMulti, MCPToolLoader, build_deep_agent

# Connect to multiple MCP servers
mcp_client = FastMCPMulti([
    HTTPServerSpec(name="crawl4ai", url="http://localhost:8001/mcp"),
    HTTPServerSpec(name="github", url="http://localhost:8002/mcp"),
    StdioServerSpec(name="grok", command="b00t-mcp", args=["grok"]),
])

# Auto-discover and convert tools
tools = await MCPToolLoader(mcp_client).load_tools()

# Build agent with discovered tools
agent = build_deep_agent(
    model=chat_model,
    tools=tools,
    system_prompt="You are b00t agent with MCP superpowers"
)
```

**Tool Type Flow:**
```
JSON-Schema (from MCP)
    → Pydantic Models (validated)
    → LangChain BaseTool (executable)
```

### 3. k0mmand3r IPC Integration

**Slash Commands:**
```bash
# Create and run agent
/agent create --name=researcher --model=claude-sonnet-4 --tools=crawl4ai,github

# Execute chain
/chain run --name=research --input="Analyze LangChain v1.0 docs"

# Agent-to-agent communication
/agent broadcast --message="What's the status?" --to=all

# Cross-agent tool invocation
/agent call --agent=researcher --tool=crawl --url=https://docs.langchain.com
```

**Redis Message Format:**
```typescript
{
  "verb": "agent" | "chain",
  "params": {
    "action": "create" | "run" | "broadcast" | "call",
    "name": "agent-name",
    "model": "claude-sonnet-4",
    "tools": "crawl4ai,github,grok",
    "input": "user input",
    "env_LANGCHAIN_API_KEY": "...",
    "middleware": "human-in-loop,summarization"
  },
  "content": "optional prompt/instructions",
  "agent_id": "b00t-agent-123"
}
```

### 4. Datum-Based Configuration

**LangChain Datum (`langchain.ai.toml`):**
```toml
[b00t]
name = "langchain"
type = "ai"
hint = "LangChain v1.0 - Build agents with LLMs + external tools"
desires = "1.0.0"

[b00t.env]
required = ["LANGCHAIN_API_KEY", "LANGSMITH_API_KEY"]
defaults = { LANGCHAIN_TRACING_V2 = "true" }

[langchain]
# Agent presets
[langchain.agents.researcher]
model = "anthropic/claude-sonnet-4"
tools = ["crawl4ai", "github", "grok"]
middleware = ["summarization"]
system_prompt = "You are a technical researcher"

[langchain.agents.coder]
model = "anthropic/claude-sonnet-4"
tools = ["github", "sequential-thinking"]
middleware = ["human-in-loop"]
system_prompt = "You are a Rust/TypeScript expert"

[langchain.chains.research]
steps = [
  { tool = "crawl4ai", input = "${url}" },
  { tool = "grok", action = "digest", content = "${crawl_result}" },
  { tool = "grok", action = "ask", query = "${question}" }
]
```

**Agent Datum (`research-agent.ai_model.toml`):**
```toml
[b00t]
name = "research-agent"
type = "ai_model"
hint = "LangChain research agent with web crawling"

[ai_model]
provider = "langchain"
langchain_agent_type = "create_agent"
base_model = "anthropic/claude-sonnet-4"
tools = ["crawl4ai-mcp", "github-mcp", "grok"]
middleware = ["summarization", "pii-redaction"]

[ai_model.parameters]
temperature = 0.7
max_iterations = 10
timeout_seconds = 300

[ai_model.metadata]
cost_per_1k_input = 0.003
cost_per_1k_output = 0.015
```

## Implementation Plan

### Phase 1: LangChain Service Core
```
langchain-agent/
├── package.json           # Python dependencies (uv-managed)
├── pyproject.toml         # Project metadata
├── src/
│   ├── __init__.py
│   ├── main.py           # Entry point with Redis listener
│   ├── agent_service.py  # LangChain agent management
│   ├── mcp_tools.py      # MCP tool discovery & loading
│   ├── k0mmand3r.py      # k0mmand3r IPC listener
│   └── types.py          # Pydantic models
├── tests/
│   ├── test_agent.py
│   ├── test_mcp.py
│   └── test_ipc.py
├── Dockerfile
├── docker-compose.yml    # Redis + LangChain service
└── README.md
```

### Phase 2: MCP Server Integration

Leverage existing b00t MCP servers:
- ✅ crawl4ai-mcp (web scraping)
- ✅ github-mcp (code analysis)
- ✅ grok (RAG/knowledge)
- ✅ sequential-thinking (planning)
- ✅ taskmaster (task tracking)

### Phase 3: Cross-Agent Communication

**Peer Agents Pattern:**
```python
# Agent A can invoke Agent B as a tool
researcher = create_agent(model, tools=[...])
coder = create_agent(model, tools=[...])

# Make coder available to researcher
researcher_with_coder = attach_peer_agent(
    researcher,
    peer=coder,
    name="coder"
)

# Now researcher can call: ask_agent_coder("How do I implement X?")
```

**Broadcast Pattern:**
```python
# Send message to all agents in crew
await agent_crew.broadcast("Status update?")

# Collect responses
responses = await agent_crew.gather_responses(timeout=10)
```

### Phase 4: Middleware Integration

**Human-in-Loop:**
```python
from langchain.middleware import HumanInLoopMiddleware

middleware = HumanInLoopMiddleware(
    redis_client=redis,
    channel="b00t:human-approval",
    timeout=300  # 5min approval window
)
```

**Summarization:**
```python
from langchain.middleware import SummarizationMiddleware

middleware = SummarizationMiddleware(
    model="claude-haiku-3",  # Fast, cheap summarization
    threshold=4000,  # Summarize if context > 4k tokens
)
```

## Comparison: LangChain vs PM2 Tasker

| Feature | PM2 Tasker | LangChain Agent |
|---------|------------|-----------------|
| **Purpose** | Process management | Agent reasoning |
| **Runtime** | Node.js processes | Python LLM agents |
| **Tools** | Shell commands | MCP tools (any language) |
| **State** | Process status | Conversation memory |
| **Clustering** | Multiple instances | Multiple agents (crew) |
| **IPC** | Redis pub/sub | Redis pub/sub |
| **Commands** | /start, /stop | /agent, /chain |

**Together:** PM2 manages the LangChain service process; LangChain manages agent reasoning.

## Benefits

1. **"Helm Chart for Thought"** - Pre-built agent components
2. **Dynamic Tool Discovery** - Auto-load MCP servers
3. **Type Safety** - JSON-Schema → Pydantic → BaseTool
4. **Cross-Agent** - Agents can invoke other agents
5. **Middleware** - Composable behaviors (approval, summarization, PII)
6. **Datum-Based** - Configuration as code (TOML)
7. **Observable** - LangSmith tracing built-in

## References

- LangChain v1.0: https://docs.langchain.com/oss/python/releases/langchain-v1
- DeepMCPAgent: https://github.com/cryxnet/deepmcpagent
- LangGraph: https://langchain-ai.github.io/langgraph/
- MCP Protocol: https://modelcontextprotocol.io/
