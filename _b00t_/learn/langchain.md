# LangChain v1.0 - Helm Chart for Thought

Master building agents with LLMs + external tools using LangChain v1.0 stable.

## Overview

LangChain v1.0 is the first stable release (November 2024) providing a production-ready framework for building agents that combine LLMs with external tools. Think of it as a "helm chart for thought" - pre-built components for agent reasoning.

**Key Concepts:**
- **Agents**: Autonomous reasoning systems that use tools
- **Tools**: Functions/APIs agents can invoke (via MCP)
- **Chains**: Predefined workflows (multi-step reasoning)
- **Middleware**: Composable behaviors (approval, summarization, PII redaction)
- **LangGraph**: State machine runtime for agents

## Installation

```bash
# Via uv (b00t standard)
uv add langchain langchain-anthropic langgraph

# Note: uv is ALWAYS used in b00t, never pip directly
```

## Core Concepts

### 1. create_agent Abstraction (New in v1.0)

The fastest way to build an agent:

```python
from langchain.agents import create_agent
from langchain_anthropic import ChatAnthropic

agent = create_agent(
    model=ChatAnthropic(model="claude-sonnet-4"),
    tools=mcp_tools,  # List of BaseTool instances
    system_prompt="You are a helpful assistant",
)

result = await agent.run("Research LangChain v1.0 features")
```

### 2. MCP Tool Integration

LangChain tools integrate seamlessly with MCP:

```python
from deepmcpagent import FastMCPMulti, MCPToolLoader, build_deep_agent
from langchain_anthropic import ChatAnthropic

# Connect to MCP servers
mcp_client = FastMCPMulti([
    HTTPServerSpec(name="crawl4ai", url="http://localhost:8001/mcp"),
    HTTPServerSpec(name="github", url="http://localhost:8002/mcp"),
    StdioServerSpec(name="grok", command="b00t-mcp", args=["grok"]),
])

# Auto-discover and load tools
tools = await MCPToolLoader(mcp_client).load_tools()

# Build agent with discovered tools
agent = build_deep_agent(
    model=ChatAnthropic(model="claude-sonnet-4"),
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

### 3. Middleware System (New in v1.0)

Composable behaviors for agents:

```python
from langchain.middleware import (
    HumanInLoopMiddleware,
    SummarizationMiddleware,
    PIIRedactionMiddleware,
)

agent = create_agent(
    model=chat_model,
    tools=tools,
    middleware=[
        HumanInLoopMiddleware(),       # Pause for approval
        SummarizationMiddleware(),     # Condense long contexts
        PIIRedactionMiddleware(),      # Remove sensitive data
    ]
)
```

#### Human-in-Loop

```python
middleware = HumanInLoopMiddleware(
    redis_client=redis,
    channel="b00t:human-approval",
    timeout=300,  # 5min approval window
    approval_required_for=["file_write", "git_commit", "api_call"]
)
```

Pauses agent execution and waits for human approval on critical actions.

#### Summarization

```python
middleware = SummarizationMiddleware(
    model="claude-haiku-3",  # Fast, cheap model
    threshold=4000,  # Summarize if context > 4k tokens
    strategy="recursive"  # or "map-reduce"
)
```

Automatically summarizes context when it exceeds threshold.

#### PII Redaction

```python
middleware = PIIRedactionMiddleware(
    patterns=["email", "phone", "ssn", "api_key"],
    replacement="[REDACTED]"
)
```

Removes sensitive data from inputs/outputs.

### 4. LangGraph Integration

LangChain v1.0 is built on LangGraph runtime:

```python
from langgraph import StateGraph, END

# Define agent state
class AgentState(TypedDict):
    messages: list[BaseMessage]
    current_tool: str | None

# Build state graph
graph = StateGraph(AgentState)
graph.add_node("agent", agent_node)
graph.add_node("tools", tool_node)
graph.add_edge("agent", "tools")
graph.add_edge("tools", "agent")
graph.set_entry_point("agent")

# Compile and run
app = graph.compile()
result = await app.ainvoke({"messages": [HumanMessage(content="Hello")]})
```

**Benefits:**
- Checkpoint/resume support
- State persistence
- Complex reasoning patterns (ReAct, Plan-and-Execute)

### 5. Cross-Agent Communication

Agents can invoke other agents as tools:

```python
# Create specialist agents
researcher = create_agent(model, tools=[crawl4ai, grok])
coder = create_agent(model, tools=[github, sequential_thinking])

# Coordinator can invoke both
coordinator = create_agent(
    model,
    tools=[],
    peer_agents=[researcher, coder]  # Available as tools
)

# Coordinator can now call:
# - ask_agent_researcher("What's LangChain v1.0?")
# - ask_agent_coder("Implement feature X")
```

## b00t Integration

### Agent Presets (from langchain.ai.toml)

**Researcher:**
```toml
[langchain.agents.researcher]
model = "anthropic/claude-sonnet-4"
tools = ["crawl4ai-mcp", "github-mcp", "grok"]
middleware = ["summarization"]
system_prompt = "You are a technical researcher..."
```

**Coder:**
```toml
[langchain.agents.coder]
model = "anthropic/claude-sonnet-4"
tools = ["github-mcp", "sequential-thinking-mcp"]
middleware = ["human-in-loop"]
system_prompt = "You are a Rust/TypeScript expert..."
```

**Coordinator:**
```toml
[langchain.agents.coordinator]
model = "anthropic/claude-sonnet-4"
tools = ["taskmaster-mcp", "sequential-thinking-mcp"]
peer_agents = ["researcher", "coder"]
system_prompt = "You coordinate multiple agents..."
```

### Slash Commands (via k0mmand3r IPC)

```bash
# Run agent
/agent run --name=researcher --input="Research LangChain v1.0"

# Execute chain
/chain run --name=research-and-digest --url=https://docs.langchain.com

# Cross-agent broadcast
/agent broadcast --message="Status update?" --to=all

# Invoke specific agent
/agent call --agent=researcher --tool=crawl --url=https://example.com
```

### Chain Workflows

```toml
[langchain.chains.research-and-digest]
steps = [
  { agent = "researcher", task = "crawl", params = { url = "${url}" } },
  { tool = "grok", action = "digest", content = "${crawl_result}" },
  { tool = "grok", action = "ask", query = "${question}" }
]
```

## LangChain v1.0 vs v0.x

| Feature | v0.x | v1.0 |
|---------|------|------|
| **create_agent** | ❌ Manual | ✅ Built-in |
| **Middleware** | ❌ None | ✅ System |
| **Stability** | ⚠️  Breaking changes | ✅ No breaking until 2.0 |
| **Package scope** | 🐘 Large | 🎯 Essential abstractions |
| **LangGraph** | Separate | Integrated runtime |

**Migration:**
- Legacy functionality → `langchain-classic`
- `langgraph.prebuilt` → `langchain.agents`

## Best Practices

### 1. Use create_agent for Simple Agents

```python
# ✅ Good - simple, readable
agent = create_agent(model, tools, system_prompt)

# ❌ Avoid - overly complex for simple cases
agent = (
    RunnablePassthrough.assign(history=...)
    | ChatPromptTemplate.from_messages(...)
    | model.bind_tools(tools)
    | ...  # Many more steps
)
```

### 2. Leverage Middleware for Cross-Cutting Concerns

```python
# ✅ Good - middleware handles approval
agent = create_agent(
    model, tools,
    middleware=[HumanInLoopMiddleware()]
)

# ❌ Avoid - manual approval logic in every tool
class MyTool(BaseTool):
    def _run(self, ...):
        if requires_approval:
            # Manual approval logic...
```

### 3. Use Type-Safe MCP Tool Discovery

```python
# ✅ Good - JSON-Schema → Pydantic → BaseTool
tools = await MCPToolLoader(mcp_client).load_tools()

# ❌ Avoid - manual tool wrapping
class MyCrawlTool(BaseTool):
    def _run(self, url: str):
        # Manual HTTP request to MCP server...
```

### 4. Organize Agents by Role

```python
# ✅ Good - specialized agents
researcher = create_agent(model, research_tools, researcher_prompt)
coder = create_agent(model, coding_tools, coder_prompt)
coordinator = create_agent(model, [], coordinator_prompt, peer_agents=[...])

# ❌ Avoid - single "do everything" agent
super_agent = create_agent(model, all_tools, generic_prompt)
```

### 5. Enable LangSmith Tracing

```bash
export LANGCHAIN_TRACING_V2="true"
export LANGCHAIN_API_KEY="lsv2_pt_..."
export LANGCHAIN_PROJECT="b00t"
```

Provides observability for debugging agent behavior.

## Common Patterns

### ReAct Loop (Think → Act → Observe)

```python
agent = create_agent(
    model=model,
    tools=tools,
    system_prompt="""You are a helpful assistant.

Use tools to gather information, then provide a final answer.

Think step-by-step:
1. What information do I need?
2. Which tool should I use?
3. What did I learn?
4. Do I have enough to answer?
"""
)
```

### Plan-and-Execute

```python
planner = create_agent(
    model=model,
    tools=[sequential_thinking],
    system_prompt="Break down the task into steps"
)

executor = create_agent(
    model=model,
    tools=all_tools,
    system_prompt="Execute the plan steps"
)

# Coordinator runs planner → executor
coordinator = create_agent(
    model=model,
    peer_agents=[planner, executor]
)
```

### Human-in-Loop Approval

```python
agent = create_agent(
    model=model,
    tools=tools,
    middleware=[
        HumanInLoopMiddleware(
            approval_required_for=["file_write", "git_commit"]
        )
    ]
)

# Agent will pause and wait for approval before writing files
result = await agent.run("Update the README")
```

## Troubleshooting

### "No module named 'langchain'"

```bash
uv add langchain langchain-anthropic langgraph
```

### "Tool not found"

Ensure MCP server is running and discoverable:
```bash
# Check MCP server
curl http://localhost:8001/mcp/list_tools

# Or via stdio
b00t-mcp grok --method list_tools
```

### "Context too long"

Enable summarization middleware:
```python
middleware=[SummarizationMiddleware(threshold=4000)]
```

### "Agent stuck in loop"

Set max iterations:
```python
agent = create_agent(model, tools, max_iterations=10)
```

## Comparison: LangChain vs Alternatives

| Feature | LangChain | LlamaIndex | CrewAI |
|---------|-----------|------------|--------|
| **Focus** | General agents | RAG/Search | Multi-agent crews |
| **MCP Support** | ✅ Via deepmcpagent | ❌ None | ❌ None |
| **Middleware** | ✅ Built-in | ❌ Manual | ✅ Role-based |
| **State Management** | ✅ LangGraph | ❌ Manual | ✅ Built-in |
| **Tool Discovery** | ✅ Dynamic (MCP) | ❌ Static | ❌ Static |

**Choose LangChain when:**
- Need dynamic tool discovery (MCP)
- Want middleware (human-in-loop, summarization)
- Building multi-step reasoning agents
- Need production stability (v1.0)

## References

- LangChain v1.0: https://docs.langchain.com/oss/python/releases/langchain-v1
- LangGraph: https://langchain-ai.github.io/langgraph/
- DeepMCPAgent: https://github.com/cryxnet/deepmcpagent
- LangSmith: https://smith.langchain.com/
- b00t Integration: `langchain-agent/README.md`
- Architecture: `LANGCHAIN_ARCHITECTURE.md`
