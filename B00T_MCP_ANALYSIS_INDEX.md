# b00t-mcp: IPC and Agent Communication - Complete Analysis

**Analysis Date**: November 17, 2025  
**Thoroughness Level**: Very Thorough  
**Source Code Reviewed**: 32 Rust files, 4 source directories, 2 CLI commands  

---

## Document Set Overview

This analysis provides a comprehensive review of b00t-mcp's Inter-Process Communication (IPC) and agent communication capabilities. The documentation is organized into three complementary documents:

### 1. [b00t_ipc_architecture.md](/tmp/b00t_ipc_architecture.md) - Complete Architecture Reference
**Length**: 26 KB | **Sections**: 14  
**Purpose**: Deep technical dive into every aspect of the system

**Contains**:
- Architecture diagrams and component relationships
- Agent Coordination Protocol (ACP) message types with examples
- Complete MCP tools reference (10+ tools documented)
- b00t-cli command reference
- Local IPC (Unix socket) and NATS (distributed) transport details
- Security model: JWT, namespace enforcement, agent identity
- Step synchronization barrier mechanism (deep dive)
- Hive mission coordination patterns
- Multi-agent workflow example: CI/CD pipeline
- Debugging and monitoring procedures
- Future enhancements roadmap

**Best For**: Implementation, integration, deep understanding

---

### 2. [b00t_quick_reference.md](/tmp/b00t_quick_reference.md) - Quick Lookup Guide
**Length**: 9.6 KB | **Sections**: 15  
**Purpose**: Fast reference for common tasks

**Contains**:
- Tool summary table (agent communication + hive missions)
- CLI command examples (copy-paste ready)
- Architecture quick map (ASCII diagram)
- Message format templates (JSON examples)
- Namespace structure reference
- Configuration file reference
- Environment variable list
- Common code patterns (4 examples)
- Debugging checklist
- File location reference
- Integration examples (Kubernetes, CI/CD, web frameworks)
- Limits and constraints table

**Best For**: Quick lookup, integration, troubleshooting

---

### 3. [b00t_overview.md](/tmp/b00t_overview.md) - Executive Summary
**Length**: 13 KB | **Sections**: 11  
**Purpose**: High-level understanding and decision-making

**Contains**:
- Document summary listing all reviewed components
- Key findings (4 major areas)
- Local vs distributed comparison table
- Tool taxonomy (by function and by role)
- Multi-step information flow example
- Code artifacts and file locations
- Security model and threat analysis
- Future evolution roadmap
- Design principles and alignment
- Usage recommendations for each component
- Conclusion with recommended starting points

**Best For**: Architecture decisions, project planning, understanding principles

---

## Key Discoveries

### The b00t-mcp System

b00t-mcp is a sophisticated agent communication framework with:

1. **Three Communication Layers**
   - Protocol Layer: Agent Coordination Protocol (ACP) with 3 message types
   - Transport Layer: Dual backends (local socket + NATS)
   - Tool Layer: MCP integration exposing 20+ agent coordination tools

2. **Core Protocol (ACP)**
   - Message Types: STATUS (state updates), PROPOSE (actions), STEP (synchronization)
   - Step Barrier: Synchronizes multi-agent workflows without distributed locks
   - JWT Security: Namespace-enforced authentication and isolation

3. **Transport Duality**
   - Local: Unix domain socket at `~/.b00t/chat.channel.socket` (IPC)
   - Distributed: NATS at `nats://c010.promptexecution.com:4222` (federation)

4. **Coordination Patterns**
   - Task Delegation: Captain→Worker with progress tracking
   - Consensus Voting: Multi-agent decision making
   - Hive Missions: Complex multi-phase workflows
   - Step Barriers: Strict synchronization points

### Source Code Components Analyzed

```
b00t-lib-chat (14 Rust modules)
├── agent.rs          - Agent struct and coordination
├── protocol.rs       - ACP message types, StepBarrier
├── transport.rs      - Socket and NATS backends
├── security.rs       - JWT validation, namespace enforcement
├── message.rs        - Chat message structure
├── client.rs         - High-level client API
├── server.rs         - Message server
├── error.rs          - Error types
└── 6 more support modules

b00t-mcp (18 Rust modules)
├── acp_tools.rs      - MCP tool implementations
├── acp_hive.rs       - Hive mission coordination
├── chat.rs           - Chat runtime and inbox
├── mcp_tools.rs      - MCP tool registry
└── 14 more integration modules

b00t-cli
├── chat send         - Send messages
├── chat info         - Show transports
├── whoami            - Agent identity
└── session *         - Session management
```

---

## Tool Inventory

### Agent Communication Tools (10+)

| Tool | Purpose | Type |
|------|---------|------|
| `agent_discover` | Find agents by capabilities | Discovery |
| `agent_message` | Direct agent-to-agent messaging | Communication |
| `agent_delegate` | Captain assigns task to worker | Orchestration |
| `agent_complete` | Worker reports completion | Orchestration |
| `agent_progress` | Report task progress | Tracking |
| `agent_wait` | Block until message received | Synchronization |
| `agent_vote_create` | Create voting proposal (captain) | Consensus |
| `agent_vote_submit` | Cast vote (any agent) | Consensus |
| `agent_notify` | Broadcast event notification | Notification |
| `agent_capability` | Request agents by capability | Discovery |

### Hive Mission Tools (6+)

| Tool | Purpose |
|------|---------|
| `acp_hive_create` | Create new hive mission |
| `acp_hive_join` | Join existing mission |
| `acp_hive_status` | Send status to hive |
| `acp_hive_propose` | Propose action to hive |
| `acp_hive_step_sync` | Wait for step synchronization |
| `acp_hive_show` | Display mission status |

---

## Protocol Summary

### Message Types

**STATUS**: Convey current state or logs
```json
{
  "step": 1,
  "agent_id": "claude.124435",
  "type": "STATUS",
  "payload": {"description": "...", "data": {...}},
  "timestamp": "...",
  "message_id": "uuid"
}
```

**PROPOSE**: Suggest an action, plan, or mutation
```json
{
  "step": 2,
  "agent_id": "orchestrator",
  "type": "PROPOSE",
  "payload": {"action": "deploy", "version": "v2.1.0"},
  "timestamp": "..."
}
```

**STEP**: Mark completion of synchronization step
```json
{
  "step": 5,
  "agent_id": "agent-id",
  "type": "STEP",
  "payload": {"step": 5},
  "timestamp": "..."
}
```

### Step Barrier Algorithm

1. Agent completes work → sends STEP message
2. StepBarrier records: `barrier.record_step_completion(step, agent_id)`
3. Check completion: `barrier.is_step_complete(step)` returns true when all agents done
4. Advance: `barrier.try_advance_step()` → move to next step
5. Timeout: After 30s, `barrier.force_advance_step()` forces progression

---

## Transport Architecture

### Local Socket (IPC)

```
Location:  ~/.b00t/chat.channel.socket
Protocol:  JSON Lines (newline-delimited JSON)
Latency:   Sub-millisecond
Scope:     Process-local, single machine
Security:  Unix file permissions + socket namespace
```

**Message Format**:
```json
{
  "channel": "mission.delta",
  "sender": "agent-x",
  "body": "Status message",
  "metadata": {...},
  "timestamp": "2025-03-04T12:00:00Z"
}
```

### NATS (Distributed)

```
Server:    nats://c010.promptexecution.com:4222
Protocol:  NATS Pub/Sub with subjects
Latency:   ~10-50ms
Scope:     Multi-machine, federated clusters
Security:  JWT authentication + NATS ACLs
```

**Subject Pattern**: `{namespace}.acp.{step}.{agent_id}.{type}`

Example: `account.engineering.acp.5.claude.124435.status`

---

## Security Model

### Namespace Structure
```
account.{organization}.{role}

Examples:
- account.engineering.ai-assistant
- account.devops.ci-cd
- account.monitoring.bot
```

### Agent Identity
```
{type}.{identifier}

Examples:
- claude.124435          (LLM agent)
- deployment-orchestrator (Service agent)
- ci-pipeline-runner     (Automation agent)
```

### Authentication
- **JWT Validation**: `AcpJwtValidator` validates tokens
- **Security Context**: `AcpSecurityContext` holds claims
- **Namespace Enforcement**: `NamespaceEnforcer` prevents cross-namespace access

---

## File Locations Reference

### Code Repositories
```
/home/brianh/.b00t/
├── b00t-cli/              # CLI binary (Rust)
├── b00t-mcp/              # MCP server (Rust)
├── b00t-lib-chat/         # ACP library (Rust)
└── k0mmand3r/             # Command framework
```

### Runtime Directories
```
~/.b00t/
├── chat.channel.socket    # Local IPC endpoint
├── .b00t.g0spell.md       # b00t Gospel (alignment)
├── .claude/               # Claude Code config
├── logs/                  # Activity logs
├── sessions/              # Session state
└── secrets/               # Credentials
```

### Configuration
```
~/.dotfiles/
├── b00t-mcp-acl.toml      # MCP access control
├── _b00t_.toml            # Configuration
└── learn/                 # Learning materials
```

---

## Quick Start Guide

### 1. Send a Message
```bash
b00t-cli chat send --channel myteam --message "Starting deployment"
```

### 2. Check Agent Identity
```bash
b00t-cli whoami
```

### 3. Monitor Transport
```bash
b00t-cli chat info
```

### 4. Enable Debug Logging
```bash
export RUST_LOG=debug
b00t-mcp --stdio
```

### 5. Simple Rust Usage
```rust
use b00t_chat::{Agent, AgentConfig};

let config = AgentConfig::new(
    "my-agent".to_string(),
    "nats://c010.promptexecution.com:4222".to_string(),
    "account.myorg.ai-assistant".to_string()
);

let agent = Agent::new(config).await?;
agent.send_status("Starting work", json!({"task": "test"})).await?;
agent.complete_step().await?;
```

---

## Strengths of the Design

1. **Simplicity**: 3 message types covers most patterns
2. **Ordering**: Step barrier avoids distributed consensus
3. **Security**: JWT + namespace isolation by default
4. **Flexibility**: Swappable transport backends
5. **Observability**: Every message timestamped with ID
6. **Composability**: Protocol → Transport → Tools layers
7. **Production-Ready**: Tested patterns, proven libraries

---

## Use Case Matrix

| Use Case | Local Socket | NATS | Hive Mission | Task Delegation | Voting |
|----------|--------------|------|--------------|-----------------|--------|
| Local coordination | ✓ | | | | |
| Multi-machine | | ✓ | | | |
| Multi-phase workflows | | | ✓ | | |
| Async task execution | | | | ✓ | |
| Consensus decisions | | | | | ✓ |
| Development/testing | ✓ | ✓ | | | |
| Production deployment | | ✓ | ✓ | ✓ | |

---

## Related Documentation

- **b00t Gospel**: `~/.b00t/.b00t.g0spell.md` - Alignment and operating protocols
- **Learning Materials**: `~/.dotfiles/_b00t_/learn/` - Topic-specific guides
- **MCP Registry**: `~/.b00t/mcp_registry.json` - Available MCP servers
- **CLI Manual**: `b00t-cli --help` - Command documentation

---

## Integration Examples

### With Docker Compose
Services can coordinate via shared socket volume or NATS cluster

### With Kubernetes
Agents run in pods, communicate via NATS cluster service

### With GitHub Actions
CI workflows send status messages via `b00t-cli chat send`

### With Web Applications
Actix-web, Rocket handlers can emit events via ChatClient

---

## Next Steps

1. **Understand**: Read `b00t_overview.md` for architecture
2. **Reference**: Use `b00t_quick_reference.md` while coding
3. **Deep Dive**: Study `b00t_ipc_architecture.md` for implementation
4. **Experiment**: Start with local socket for development
5. **Scale**: Migrate to NATS for multi-machine coordination
6. **Orchestrate**: Use Hive missions for complex workflows

---

## Conclusion

b00t-mcp provides a comprehensive, production-grade agent communication framework that elegantly solves the challenges of multi-agent coordination. The step barrier pattern is particularly noteworthy for enabling strict ordering without distributed consensus algorithms.

The architecture is proven, secure by default, and scales from local development to distributed production deployments.

---

## Document Statistics

| Document | Size | Sections | Purpose |
|----------|------|----------|---------|
| b00t_ipc_architecture.md | 26 KB | 14 | Complete technical reference |
| b00t_quick_reference.md | 9.6 KB | 15 | Quick lookup and integration |
| b00t_overview.md | 13 KB | 11 | Executive summary and planning |
| **Total** | **49 KB** | **40** | **Comprehensive analysis** |

**Code Reviewed**: 32 Rust files across 4 directories  
**Tools Documented**: 20+ MCP tools  
**Examples Provided**: 15+ code samples  
**Diagrams**: 5 ASCII architecture diagrams  

