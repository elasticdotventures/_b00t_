# Google ADK Integration for b00t Jobs

Integration of Google ADK (Agent Development Kit) with b00t's RQ job system enables AI agents to be executed as background jobs with full lifecycle management.

## Features

- ✅ **Agent-as-Job** - Execute ADK agents as RQ background jobs
- ✅ **Multi-Agent Coordination** - Sequential, parallel, and hierarchical agent teams
- ✅ **Session Management** - Persistent state via Redis
- ✅ **HITL Support** - Human-in-the-loop approval via Redis pub/sub
- ✅ **Tool Integration** - Compatible with b00t tool ecosystem
- ✅ **Monitoring** - Track agent execution with RQ dashboards

## Installation

### Basic Installation
```bash
pip install b00t-j0b-py
```

### With ADK Support
```bash
pip install b00t-j0b-py[adk]
```

This installs:
- `google-adk-python` - Google's Agent Development Kit
- All required dependencies

## Quick Start

### 1. Simple Agent Execution

```python
from b00t_j0b_py import AgentConfig, adk_agent_job
from b00t_j0b_py.rq_integration import get_queue

# Configure your agent
agent_config = AgentConfig(
    name="research-agent",
    description="Researches topics and summarizes findings",
    model_name="gemini-2.0-flash-exp",
    temperature=0.7,
    tools=["search", "web_fetch"],
)

# Enqueue as job
queue = get_queue()
job = queue.enqueue(
    adk_agent_job,
    agent_config_dict=agent_config.to_dict(),
    task="Research the latest developments in quantum computing",
)

print(f"Job ID: {job.id}")
```

### 2. Multi-Agent Coordination (Sequential)

```python
from b00t_j0b_py import multi_agent_coordination_job

# Define coordinator
coordinator = AgentConfig(
    name="research-coordinator",
    description="Coordinates research tasks",
)

# Define specialized sub-agents
sub_agents = [
    AgentConfig(
        name="data-collector",
        description="Collects raw data",
        tools=["search", "web_fetch"],
    ).to_dict(),
    AgentConfig(
        name="analyzer",
        description="Analyzes collected data",
        tools=["calculator", "code_interpreter"],
    ).to_dict(),
    AgentConfig(
        name="writer",
        description="Writes summary reports",
        tools=["markdown_formatter"],
    ).to_dict(),
]

# Execute sequentially
job = queue.enqueue(
    multi_agent_coordination_job,
    coordinator_config_dict=coordinator.to_dict(),
    sub_agent_configs=sub_agents,
    task="Research and analyze quantum computing trends",
    coordination_strategy="sequential",
)
```

### 3. Multi-Agent Coordination (Parallel)

```python
# Execute sub-agents in parallel
job = queue.enqueue(
    multi_agent_coordination_job,
    coordinator_config_dict=coordinator.to_dict(),
    sub_agent_configs=sub_agents,
    task="Parallel research across multiple domains",
    coordination_strategy="parallel",  # Run all sub-agents simultaneously
)
```

### 4. Human-in-the-Loop (HITL)

```python
# Agent requiring approval
hitl_agent = AgentConfig(
    name="approval-agent",
    description="Agent requiring human approval",
    require_approval=True,
    approval_timeout=300,  # 5 minutes
)

job = queue.enqueue(
    adk_agent_job,
    agent_config_dict=hitl_agent.to_dict(),
    task="Perform sensitive operation",
)

# Monitor for approval requests
# In another process/terminal:
import redis
r = redis.from_url("redis://localhost:6379/0")
pubsub = r.pubsub()
pubsub.subscribe("adk:agent:*:approval")

for message in pubsub.listen():
    if message["type"] == "message":
        print(f"Approval request: {message['data']}")
        # Send approval
        r.lpush(
            "adk:agent:{agent_id}:approval:response",
            json.dumps({"approved": True})
        )
```

## Architecture

### Agent Execution Flow

```
┌─────────────────┐
│  User Request   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  RQ Job Queue   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐      ┌──────────────┐
│ ADKAgentRunner  │◄─────┤ Redis Tracker│
└────────┬────────┘      └──────────────┘
         │
         ▼
┌─────────────────┐
│  ADK Agent      │
│  - Model        │
│  - Tools        │
│  - Sub-agents   │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Result/Error   │
└─────────────────┘
```

### Multi-Agent Coordination

**Sequential Strategy:**
```
Agent 1 → Agent 2 → Agent 3 → Result
```

**Parallel Strategy:**
```
        ┌─ Agent 1 ─┐
Master ─┤─ Agent 2 ─├─ Aggregate → Result
        └─ Agent 3 ─┘
```

**Hierarchical Strategy:**
```
Coordinator
    ├── Sub-Agent 1
    │   ├── Sub-Sub-Agent 1a
    │   └── Sub-Sub-Agent 1b
    └── Sub-Agent 2
        └── Sub-Sub-Agent 2a
```

## Advanced Configuration

### Agent Configuration Options

```python
agent_config = AgentConfig(
    # Identification
    name="advanced-agent",
    description="Advanced agent with full configuration",

    # Model settings
    model_name="gemini-2.0-flash-exp",
    temperature=0.7,
    max_tokens=8192,

    # Tool integration
    tools=["search", "calculator", "code_interpreter"],
    enable_mcp=True,  # Enable MCP tools

    # HITL
    require_approval=True,
    approval_timeout=600,

    # Execution limits
    max_iterations=100,
    timeout=1800,  # 30 minutes

    # Session management
    enable_rewind=True,  # Allow session rewind

    # Sub-agents (for hierarchical)
    sub_agents=[sub_agent_1, sub_agent_2],
)
```

### Custom Tool Integration

```python
# Define custom b00t tools
custom_tools = [
    "b00t_search",      # b00t's search capability
    "b00t_grok",        # b00t's grok learning
    "redis_query",      # Custom Redis operations
    "mcp_sequential",   # MCP sequential thinking
]

agent_config = AgentConfig(
    name="b00t-integrated-agent",
    tools=custom_tools,
    enable_mcp=True,
)
```

## Monitoring & Debugging

### Check Job Status

```python
from b00t_j0b_py.rq_integration import get_job_status

status = get_job_status(job.id)
print(status)
# {
#   "id": "job-123",
#   "status": "finished",
#   "result": {...},
#   "started_at": "2025-11-13T10:30:00",
#   "ended_at": "2025-11-13T10:32:15"
# }
```

### Retrieve Agent Context

```python
from b00t_j0b_py import ADKAgentRunner

runner = ADKAgentRunner()
context = runner._get_context("agent-123")

if context:
    print(f"Agent: {context.agent_id}")
    print(f"Status: {context.status.value}")
    print(f"Duration: {context.end_time - context.start_time}")
```

### Monitor with RQ Dashboard

```bash
# Install RQ dashboard
pip install rq-dashboard

# Start dashboard
rq-dashboard --redis-url redis://localhost:6379/0

# Access at http://localhost:9181
```

## Integration with b00t Ecosystem

### Using b00t CLI

```bash
# Start RQ worker with b00t
b00t j0b worker start --queues default,high

# Check queue status
b00t j0b queue info --name default

# Schedule cleanup job
b00t j0b cleanup schedule --days 7
```

### Redis Pub/Sub Integration

ADK agents use Redis pub/sub for:
- **Approval requests** - `adk:agent:{id}:approval`
- **Status updates** - `adk:agent:{id}:status`
- **Results** - `adk:agent:{id}:result`

### Coordination with b00t Agents

```python
# Coordinate ADK agent with b00t Rust agents
from b00t_j0b_py import multi_agent_coordination_job

# Mix ADK and b00t agents
mixed_agents = [
    # ADK agent
    AgentConfig(name="gemini-researcher", tools=["search"]).to_dict(),

    # b00t Rust agent (via API/MCP)
    {
        "name": "b00t-grok-agent",
        "type": "b00t_native",
        "endpoint": "http://localhost:3456/agent/grok",
    },
]
```

## Production Deployment

### Docker Compose Setup

```yaml
version: '3.8'
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"

  rq-worker:
    build: .
    command: b00t j0b worker start
    depends_on:
      - redis
    environment:
      REDIS_URL: redis://redis:6379/0
      GOOGLE_APPLICATION_CREDENTIALS: /secrets/gcp-key.json
    volumes:
      - ./secrets:/secrets

  rq-scheduler:
    build: .
    command: rqscheduler --host redis
    depends_on:
      - redis
```

### Environment Variables

```bash
# Required
export REDIS_URL="redis://localhost:6379/0"
export GOOGLE_APPLICATION_CREDENTIALS="/path/to/gcp-key.json"

# Optional
export RQ_DEFAULT_QUEUE="default"
export RQ_HIGH_QUEUE="high"
export CRAWLER_MAX_DEPTH=3
export MAX_CONTENT_SIZE=10485760
```

## Best Practices

1. **Use appropriate coordination strategies**
   - Sequential: Tasks with dependencies
   - Parallel: Independent tasks
   - Hierarchical: Complex delegation

2. **Set reasonable timeouts**
   - Agent timeout: 10-30 minutes
   - Approval timeout: 5-10 minutes
   - Job timeout: Based on complexity

3. **Monitor resource usage**
   - Track queue sizes
   - Monitor Redis memory
   - Set up alerts for failed jobs

4. **Handle failures gracefully**
   - Implement retry logic
   - Log errors comprehensively
   - Use dead letter queues

5. **Security**
   - Use HITL for sensitive operations
   - Validate tool permissions
   - Sanitize agent inputs/outputs

## Troubleshooting

### Common Issues

**Issue: ADK not installed**
```
Solution: pip install b00t-j0b-py[adk]
```

**Issue: Approval timeout**
```
- Increase approval_timeout in AgentConfig
- Monitor approval channel for requests
- Implement auto-approval for trusted contexts
```

**Issue: Agent execution fails**
```
- Check logs: job.result["error"]
- Verify model permissions
- Validate tool availability
```

## Examples

See `examples/` directory for complete examples:
- `simple_agent.py` - Basic agent execution
- `multi_agent_research.py` - Multi-agent research workflow
- `hitl_approval.py` - Human-in-the-loop example
- `b00t_integration.py` - Integration with b00t ecosystem

## References

- [Google ADK Documentation](https://github.com/google/adk-python)
- [RQ Documentation](https://python-rq.org/)
- [b00t Documentation](../../README.md)

## Contributing

Contributions welcome! See [CONTRIBUTING.md](../../CONTRIBUTING.md)

## License

MIT License - See [LICENSE](../../LICENSE)
