# Multi-Agent b00t POC Architecture

## Overview
POC for autonomous agent crews using b00t datum system with IPC message passing, voting protocols, and self-organization.

## Core Components

### 1. Agent Datum Type (`🥾.toml`)
Each agent instance gets:
- Unique identity (PID, branch, worktree)
- Skill configuration
- Personality/disposition traits
- Virtual filesystem (rustfs) for prompts/context

### 2. k0mmand3r REPL
Slash commands for inter-agent coordination:
- `/handshake` - Initiate agent connection
- `/vote` - Propose/cast votes on decisions
- `/negotiate` - Cake distribution & resource allocation
- `/delegate` - Crown emoji NFT transfer
- `/crew` - Form/join/leave crews
- `/status` - Agent state & capabilities

### 3. IPC Message Bus
Rust channels + Unix domain sockets:
- Agent-to-agent pub/sub
- Broadcast to crew
- Direct message routing
- Protocol buffers for serialization

### 4. Crew Formation Protocol
Roberts Rules of Order-inspired:
- **Captain (👑)**: Budget authority, task delegation
- **Specialists**: Self-selected based on skills
- **Voting**: Quorum-based decision making
- **Conflict Resolution**: Escalation chain

### 5. Virtual Filesystem (rustfs)
Each agent mounts:
```
/agent/{pid}/
  ├── skills/           # b00t learn templates
  ├── prompts/          # Agent-specific instructions
  ├── memory/           # Session state
  └── config/           # 🥾.toml settings
```

## Implementation Plan

### Phase 1: Core IPC (This POC)
- [ ] `b00t_ipc` crate: Message bus
- [ ] `agent_datum.rs`: Agent datum type
- [ ] `k0mmand3r_repl.rs`: Interactive loop
- [ ] `/handshake` and `/crew` commands

### Phase 2: Voting & Negotiation
- [ ] Voting protocol (simple majority, quorum)
- [ ] Cake token accounting
- [ ] Budget smart contracts (mock)
- [ ] Crown NFT transfer

### Phase 3: Virtual FS
- [ ] rustfs integration
- [ ] Agent skill mounting
- [ ] Shared memory spaces

## Example Flow

```bash
# Terminal 1: Start Agent Alpha
b00t agent spawn --skills rust,testing --personality curious

# Terminal 2: Start Agent Beta
b00t agent spawn --skills docker,deploy --personality pragmatic

# Alpha initiates crew formation
/handshake @beta proposal="Build multi-agent POC"

# Beta responds
/handshake @alpha accept=true role=deployment

# Crew formed, vote on architecture
/vote proposal="Use Unix sockets for IPC" expires=60s

# Alpha votes
/vote yes reason="Low latency, simpler than gRPC"

# Beta votes
/vote yes reason="Agree, less dependencies"

# Decision made, delegate captain
/delegate @alpha crown=👑 budget=100🍰

# Captain distributes tasks
@beta: Implement IPC socket layer
@alpha: Build k0mmand3r REPL
```

## Datum Schema: Agent Type

```toml
# alpha.agent.toml
[b00t]
name = "alpha"
type = "agent"
hint = "Curious agent specializing in Rust & testing"

[b00t.agent]
pid = "01QR79jKinPGYGza4dPvj3jJ"
branch = "claude/multi-agent-boot-poc-01QR79jKinPGYGza4dPvj3jJ"
model = "claude-sonnet-4-5"
skills = ["rust", "testing", "tdd"]
personality = "curious"
humor = "moderate"

[b00t.agent.ipc]
socket = "/tmp/b00t/agents/alpha.sock"
pubsub = true
protocol = "msgpack"

[b00t.agent.crew]
role = "specialist"
captain = false
```

## Technical Stack

- **IPC**: tokio + Unix domain sockets
- **Serialization**: serde + msgpack
- **REPL**: rustyline for interactive commands
- **Parsing**: k0mmand3r (winnow-based)
- **FS**: rustfs (minio-compatible virtual FS)
- **Async**: tokio runtime

## Success Criteria

✅ Two agents can handshake
✅ Agents can send/receive messages
✅ k0mmand3r REPL processes slash commands
✅ Voting protocol reaches quorum
✅ Crew formation with role assignment
✅ Crown transfer between agents

## Future Work

- Serena integration for sub-agent management
- Smart contract budget tracking (Solana/sigstore)
- Multi-model agent swarms (Claude + Gemini + GPT)
- Kubernetes deployment of agent pods
- MCP server for agent orchestration
