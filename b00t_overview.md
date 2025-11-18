# b00t-mcp Agent Communication: Complete Overview

## Document Summary

This analysis comprehensively reviews b00t-mcp's IPC and agent communication architecture through examination of:

1. **b00t-lib-chat** - Core ACP protocol library
   - Source: `/home/brianh/.b00t/b00t-lib-chat/src/`
   - 14 Rust modules implementing ACP messaging, transport, security, and coordination

2. **b00t-mcp** - MCP server exposing agent tools
   - Source: `/home/brianh/.b00t/b00t-mcp/src/`
   - 18 Rust modules providing MCP integration and agent communication tools

3. **b00t-cli** - Command-line interface for agent coordination
   - Command: `b00t-cli chat` - Agent Coordination Protocol interface
   - Command: `b00t-cli whoami` - Agent identity and context
   - Command: `b00t-cli session` - Session management

4. **Infrastructure**
   - Local IPC: Unix domain socket at `~/.b00t/chat.channel.socket`
   - Distributed: NATS at `nats://c010.promptexecution.com:4222`

---

## Key Findings

### 1. Three-Layer Communication Model

**Protocol Layer (ACP)**
- Defines 3 message types: STATUS, PROPOSE, STEP
- All messages include step number (synchronization point)
- Monotonically increasing steps enable strict ordering
- JSON payloads allow flexible extension

**Transport Layer**
- Dual transport backend (local socket + NATS)
- Abstracted via `ChatTransport` enum
- Local socket: JSON lines over Unix domain socket
- NATS: Subject-based pub/sub with JWT auth

**Tool Layer (MCP)**
- 10+ agent communication tools
- Integrated with `derive_mcp` reflection
- ACL-controlled via TOML configuration
- Supports captain-worker delegation patterns

### 2. Synchronization Mechanism

**Step Barrier Pattern**
- Tracks which agents completed each step
- Blocks progression until all agents signal completion
- Timeouts force advancement (30 second default)
- Enables multi-agent coordination without distributed locks

**Usage**: CI/CD pipelines, multi-stage workflows, consensus decisions

### 3. Security & Isolation

**JWT-Based Authentication**
- `AcpJwtValidator` validates tokens
- `AcpSecurityContext` holds claims
- `NamespaceEnforcer` prevents cross-namespace access

**Namespace Structure**
- Format: `account.{org}.{role}`
- Examples: `account.engineering.ai-assistant`, `account.devops.ci-cd`
- Subjects scoped to namespace: prevents information leakage

**Agent Identification**
- Format: `{type}.{id}` (e.g., `claude.124435`)
- Allows routing to specific agent instances
- Namespace + agent ID = complete routing key

### 4. Hive Mission Coordination

**Multi-Agent Workflows**
- Captain agent creates mission with expected participant count
- Worker agents join mission
- StepBarrier coordinates phases across all agents
- Status and proposal messages exchanged per step

**Monitoring**
- `HiveMission` tracks mission metadata
- `HiveStatus` aggregates agent states
- `AgentStatus` per-agent tracking with last seen timestamp

---

## Comparison: Local vs Distributed

| Aspect | Local (Socket) | NATS (Distributed) |
|--------|---------------|-------------------|
| **Protocol** | JSON Lines | NATS Pub/Sub |
| **Location** | `~/.b00t/chat.channel.socket` | `c010.promptexecution.com:4222` |
| **Latency** | Sub-millisecond | ~10-50ms |
| **Reliability** | Process-local only | Survives process restart |
| **Security** | Filesystem permissions | JWT + NATS ACLs |
| **Scalability** | Single machine | Multi-cluster federation |
| **Use Case** | Within-system IPC | Inter-cluster coordination |

---

## Tool Taxonomy

### By Function

**Discovery & Introspection**
- `agent_discover` - Find agents
- `agent_capability` - Request by capability

**Direct Communication**
- `agent_message` - Send message to agent
- `agent_wait` - Block until message received

**Task Orchestration (Captain→Worker)**
- `agent_delegate` - Assign task
- `agent_progress` - Track progress
- `agent_complete` - Report completion

**Consensus & Voting**
- `agent_vote_create` - Create proposal (captain)
- `agent_vote_submit` - Cast vote (any agent)

**Notifications & Events**
- `agent_notify` - Broadcast event
- `acp_hive_*` - Mission-specific operations

### By Role

**Captain (Orchestrator)**
- Can: delegate tasks, create votes/missions
- Example: deployment coordinator

**Worker (Executor)**
- Can: report progress/completion, send status
- Example: test runner, deployment agent

**Observer (Monitor)**
- Can: discover agents, wait for messages, send notifications
- Example: monitoring agent, logger

---

## Information Flow: Example Workflow

```
Step 0: Initialization
┌────────────────────────────────────────────────────┐
│ Agent A initiates: "Starting analysis"             │
│ Agent B responds: "Ready for analysis"             │
│ Agent C confirms: "Queues prepared"                │
│ ↓                                                  │
│ StepBarrier checks: A=ready, B=ready, C=ready     │
│ ✓ All agents sent STEP message for step 0         │
│ → Advance to Step 1                               │
└────────────────────────────────────────────────────┘

Step 1: Processing
┌────────────────────────────────────────────────────┐
│ Agent A: "Processing 500 records" (STATUS)        │
│ Agent B: "Running validation tests" (STATUS)      │
│ Agent C: "Storing results" (STATUS)               │
│ ↓                                                  │
│ Agent A: "Ready for review" (STEP)                │
│ Agent B: "All tests passed" (STEP)                │
│ Agent C: "Storage confirmed" (STEP)               │
│ ↓                                                  │
│ StepBarrier checks: A=ready, B=ready, C=ready     │
│ → Advance to Step 2                               │
└────────────────────────────────────────────────────┘

Step 2: Decision
┌────────────────────────────────────────────────────┐
│ Agent A: "Propose: deploy to staging" (PROPOSE)   │
│ Agent B: "Approve: tests passing" (PROPOSE)       │
│ ↓                                                  │
│ Captain Agent: Consensus reached? YES             │
│ Captain: PROPOSE "Deploy to staging" (PROPOSE)    │
│ ↓                                                  │
│ All send STEP for step 2                          │
│ → Advance to Step 3                               │
└────────────────────────────────────────────────────┘

Step 3: Deployment
┌────────────────────────────────────────────────────┐
│ Agent C: "Deploying service v2.1.0" (STATUS)     │
│ Agent B: "Monitoring health metrics" (STATUS)     │
│ ↓                                                  │
│ All send STEP for step 3                          │
│ → Mission Complete                                │
└────────────────────────────────────────────────────┘
```

---

## Code Artifacts

### Source Code Locations

**Core ACP Library**
- `/home/brianh/.b00t/b00t-lib-chat/src/agent.rs` - Agent struct, coordination
- `/home/brianh/.b00t/b00t-lib-chat/src/protocol.rs` - Message types, StepBarrier
- `/home/brianh/.b00t/b00t-lib-chat/src/transport.rs` - Socket and NATS backends
- `/home/brianh/.b00t/b00t-lib-chat/src/security.rs` - JWT, namespace enforcement
- `/home/brianh/.b00t/b00t-lib-chat/src/message.rs` - Chat message structure

**MCP Integration**
- `/home/brianh/.b00t/b00t-mcp/src/acp_tools.rs` - MCP tool implementations
- `/home/brianh/.b00t/b00t-mcp/src/acp_hive.rs` - Hive mission coordination
- `/home/brianh/.b00t/b00t-mcp/src/chat.rs` - Chat runtime and inbox

**CLI Interface**
- Command: `b00t-cli chat send` - Send messages
- Command: `b00t-cli chat info` - Show transport info
- Command: `b00t-cli whoami` - Agent identity
- Command: `b00t-cli session` - Session management

### Configuration

- **ACL**: `~/.dotfiles/b00t-mcp-acl.toml` - Access control
- **Socket**: `~/.b00t/chat.channel.socket` - Local IPC endpoint
- **Session**: `~/.b00t/sessions/` - Session state
- **Logs**: `~/.b00t/logs/` - Activity logs

---

## Security Model

### Threat Model

1. **Unauthorized Access**
   - Mitigated: Unix socket permissions + JWT validation
   - NATS subjects enforce namespace boundaries

2. **Man-in-the-Middle**
   - Local: Socket in user home directory (file permissions)
   - NATS: TLS can be enabled (not shown in code)

3. **Message Spoofing**
   - Agent ID in NATS subject prevents spoofing
   - JWT claims validate agent identity

4. **Denial of Service**
   - StepBarrier timeout prevents hang
   - Per-agent rate limiting possible (not implemented)

### Trust Assumptions

- NATS server is trusted (no authentication shown in stubs)
- File permissions on `~/.b00t/` trusted to user
- JWT issuer is single source of truth for authentication
- Operator (BMI) is trusted for session init

---

## Future Evolution

### Immediate (Planned)

1. **Distributed Tracing**: OpenTelemetry spans for message flow
2. **Message Persistence**: Jetstream durability for critical ops
3. **Load Balancing**: NATS queue groups for horizontal scaling

### Medium-term

1. **Circuit Breakers**: Graceful degradation on agent failures
2. **Rate Limiting**: Per-agent and per-namespace throttling
3. **Audit Logging**: Complete message audit trail with signatures

### Long-term

1. **WebSocket Transport**: Real-time UI integration
2. **gRPC Option**: High-performance binary protocol
3. **Multi-cluster**: Across-datacenter federation
4. **Consensus Algorithms**: BFT voting for critical decisions

---

## Lessons Learned (b00t Gospel Alignment)

### Design Principles Applied

1. **Simplicity**
   - 3 message types covers most patterns
   - Step barrier avoids distributed locks

2. **Composability**
   - Tools layer built on protocol layer
   - Transport layer swappable

3. **Observability**
   - Every message has timestamp, message_id, step
   - Tracing logs at each stage

4. **Security by Default**
   - JWT required for NATS
   - Namespace isolation enforced

5. **No Reinvention**
   - Uses proven patterns (barriers, pub/sub, JWT)
   - Leverages existing libraries (async-nats, tokio)

---

## Usage Recommendations

### When to Use Local Socket
- Single machine coordination
- High frequency updates (sub-ms latency)
- Development/testing
- Example: Local container orchestration

### When to Use NATS
- Multi-container/machine coordination
- Durability required (persistent messages)
- Complex routing patterns
- Example: Distributed CI/CD pipeline

### When to Use Hive Missions
- Coordinated multi-agent workflows
- Explicit step synchronization needed
- Mission state tracking required
- Example: Complex deployment orchestration

### When to Use Task Delegation
- Captain-worker relationships
- Progress tracking important
- Async task completion
- Example: Job queue system

### When to Use Voting
- Consensus required
- Multiple stakeholders
- Audit trail needed
- Example: Feature flag decisions

---

## Conclusion

b00t-mcp provides a production-grade agent communication framework with:

1. **Clear Protocol**: 3 message types, step-based synchronization
2. **Flexible Transport**: Local IPC + distributed NATS
3. **Security Built-in**: JWT + namespace isolation
4. **Rich Coordination**: Barriers, voting, task delegation
5. **MCP Integration**: 20+ tools accessible to LLM agents

The architecture enables complex multi-agent workflows while maintaining simplicity, observability, and security. The step barrier pattern is particularly elegant for ensuring ordering without distributed consensus algorithms.

Recommended starting point: Use local socket for development, NATS for production. Start with STATUS messages, add PROPOSE/voting as needed. Use Hive missions for complex multi-agent workflows.

