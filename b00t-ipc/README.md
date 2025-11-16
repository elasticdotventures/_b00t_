# b00t-ipc

Inter-Process Communication library for b00t multi-agent systems.

## Overview

`b00t-ipc` provides the foundational primitives for autonomous agent coordination:

- **Message Bus**: Async pub/sub via tokio channels
- **Agent Identity**: Skills, personality, role-based configuration
- **Voting Protocol**: Quorum-based democratic decision making
- **Typed Messages**: Handshake, Vote, Crew, Delegate, Negotiate, Status

## Quick Start

```rust
use b00t_ipc::{Agent, MessageBus, Message, VoteChoice};

#[tokio::main]
async fn main() {
    // Create agents
    let alpha = Agent::new("alpha", vec!["rust", "testing"]);
    let beta = Agent::new("beta", vec!["docker", "deploy"]);

    // Initialize message bus
    let bus = MessageBus::new().await.unwrap();
    bus.register(alpha).await.unwrap();
    bus.register(beta).await.unwrap();

    // Handshake
    bus.handshake("alpha", "beta").await.unwrap();

    // Vote on proposal
    let proposal_id = bus
        .create_proposal("Use async IPC", "alpha")
        .await.unwrap();

    bus.vote(&proposal_id, "alpha", VoteChoice::Yes).await.unwrap();
    bus.vote(&proposal_id, "beta", VoteChoice::Yes).await.unwrap();

    assert!(bus.is_proposal_passed(&proposal_id).await.unwrap());
}
```

## Message Types

### Handshake
Initiate connection between agents:
```rust
Message::Handshake {
    from: "alpha",
    to: "beta",
    proposal: "Join crew"
}
```

### Vote
Cast vote on proposal:
```rust
Message::Vote {
    from: "alpha",
    proposal_id: "abc123",
    vote: VoteChoice::Yes,
    reason: Some("Good architecture")
}
```

### CrewForm
Form coordinated team:
```rust
Message::CrewForm {
    initiator: "alpha",
    members: vec!["beta", "gamma"],
    purpose: "Build multi-agent POC"
}
```

### Delegate
Transfer authority:
```rust
Message::Delegate {
    from: "alpha",
    to: "beta",
    crown: "👑",
    budget: 100  // 🍰 cake tokens
}
```

## Agent Roles

- **Specialist**: Default role, executes tasks with expertise
- **Captain**: Holds crown 👑, manages budget, coordinates crew
- **Observer**: Monitors and learns without voting rights

## Voting Protocol

Proposals require quorum (default: 2 votes):

```rust
let mut proposal = Proposal::new("Use Unix sockets", "alpha");
proposal.cast_vote("alpha".to_string(), VoteChoice::Yes);
proposal.cast_vote("beta".to_string(), VoteChoice::Yes);

assert!(proposal.is_passed());
```

## Architecture

```
MessageBus
├── agents: HashMap<String, Agent>
├── proposals: HashMap<String, Proposal>
├── tx: UnboundedSender<Message>
└── rx: UnboundedReceiver<Message>
```

All async operations use tokio runtime with RwLock for concurrent access.

## Testing

```bash
cargo test --package b00t-ipc
```

All 4 tests passing:
- `test_agent_creation`
- `test_message_bus`
- `test_voting`
- `test_proposal_lifecycle`

## Dependencies

- **tokio**: Async runtime and channels
- **serde**: Message serialization
- **uuid**: Unique identifiers
- **anyhow**: Error handling

## Future Work

- Unix domain socket transport
- Persistent message log
- Leader election (Raft)
- Smart contract budgets

## License

MIT
