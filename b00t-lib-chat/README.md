# 🥾 B00T Agent Coordination Protocol (ACP) Library

[![Crates.io](https://img.shields.io/crates/v/b00t-acp.svg)](https://crates.io/crates/b00t-acp)
[![Documentation](https://docs.rs/b00t-acp/badge.svg)](https://docs.rs/b00t-acp)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

A lightweight peer-to-peer coordination protocol for turn-based agent collaboration, implemented in Rust with Python and TypeScript bindings.

## 🎯 Features

- **Step-based coordination**: Discrete rounds with barrier synchronization
- **NATS transport**: Built on async-nats for reliable messaging  
- **Role-based permissions**: Isolated namespaces per GitHub user
- **Language bindings**: Python and TypeScript/JavaScript support
- **Production ready**: Used in b00t agent orchestration system

## 📐 Protocol Overview

The Agent Coordination Protocol (ACP) defines a minimal mechanism for multiple autonomous agents to coordinate their actions in discrete steps using Lamport timing.

### Message Types

- **STATUS**: Convey current state or logs of an agent
- **PROPOSE**: Suggest an action, plan, or mutation  
- **STEP**: Mark completion of a step for barrier synchronization

### Subject Pattern

Messages are published to NATS subjects following the pattern:
```
{namespace}.acp.{step}.{agent_id}.{message_type}
```

Example: `account.elasticdotventures.acp.5.claude.124435.propose`

## 🚀 Quick Start

### Rust

Add to your `Cargo.toml`:
```toml
[dependencies]
b00t-acp = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
serde_json = "1.0"
```

```rust
use b00t_acp::{Agent, AgentConfig};
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = AgentConfig::new(
        "claude.124435".to_string(),
        "nats://c010.promptexecution.com:4222".to_string(),
        "account.elasticdotventures".to_string(),
    )
    .with_jwt("eyJ0eXAi...".to_string())
    .with_role("ai-assistant".to_string());

    let agent = Agent::new(config).await?;
    agent.start().await?;
    
    // Send status message
    agent.send_status("Agent ready", json!({"ready": true})).await?;
    
    // Propose an action
    agent.send_propose("execute_command", json!({
        "command": "git status",
        "working_dir": "/tmp"
    })).await?;
    
    // Complete step
    agent.complete_step().await?;
    
    Ok(())
}
```

### Python

Install via pip (when published):
```bash
pip install b00t-acp
```

```python
import asyncio
from b00t_acp import Agent, AgentConfig

async def main():
    config = AgentConfig("claude.124435", 
                        "nats://c010.promptexecution.com:4222",
                        "account.elasticdotventures")
    config.with_jwt("eyJ0eXAi...")
    config.with_role("ai-assistant")
    
    agent = Agent()
    await agent.connect(config)
    await agent.start()
    
    # Send messages
    await agent.send_status("Agent ready", {"ready": True})
    await agent.send_propose("execute_command", {
        "command": "git status",
        "working_dir": "/tmp"
    })
    
    # Complete step
    await agent.complete_step()

asyncio.run(main())
```

### TypeScript/JavaScript

Install via npm (when published):
```bash
npm install b00t-acp
```

```typescript
import { Agent, AgentConfig } from 'b00t-acp';

async function main() {
    const config = new AgentConfig(
        "claude.124435",
        "nats://c010.promptexecution.com:4222", 
        "account.elasticdotventures"
    );
    config.withJwt("eyJ0eXAi...");
    config.withRole("ai-assistant");
    
    const agent = new Agent();
    await agent.connect(config);
    await agent.start();
    
    // Send messages
    await agent.sendStatus("Agent ready", { ready: true });
    await agent.sendPropose("execute_command", {
        command: "git status",
        working_dir: "/tmp"
    });
    
    // Complete step
    await agent.completeStep();
}

main();
```

## 🏗️ Architecture

### Step Synchronization

Agents coordinate through a step barrier mechanism:

1. Agents broadcast STATUS and PROPOSE messages during a step
2. When ready, agents broadcast STEP message
3. Step completes when all known agents send STEP (or timeout)
4. All agents advance to next step simultaneously

### Transport Layer

Built on NATS for reliable, scalable messaging:

- **Authentication**: JWT-based with GitHub user isolation
- **Subjects**: Hierarchical namespace per user/role/step
- **Delivery**: At-least-once with optional persistence
- **Clustering**: Native NATS server clustering support

### Example Flow

```mermaid
sequenceDiagram
    participant A1 as Agent A1
    participant A2 as Agent A2 
    participant A3 as Agent A3
    
    Note over A1,A2,A3: Step 1
    
    A1->>A2: STATUS {step:1, payload:{...}}
    A1->>A3: STATUS {step:1, payload:{...}}
    A2->>A1: PROPOSE {step:1, action:"deploy"}
    A2->>A3: PROPOSE {step:1, action:"deploy"}
    
    A1->>A2: STEP {step:1}
    A1->>A3: STEP {step:1}
    A2->>A1: STEP {step:1}
    A2->>A3: STEP {step:1}
    A3->>A1: STEP {step:1}
    A3->>A2: STEP {step:1}
    
    Note over A1,A2,A3: Step 1 Complete → Advance to Step 2
```

## 🔧 Development

### Prerequisites

- Rust 1.75+
- NATS server (for integration tests)
- Python 3.8+ (for Python bindings)
- Node.js 18+ (for TypeScript bindings)

### Build

```bash
# Rust library
cargo build --release

# Python bindings  
cargo build --release --features python
maturin develop

# WebAssembly/TypeScript bindings
cargo build --release --features wasm
wasm-pack build --target web --features wasm
```

### Testing

```bash
# Unit tests
cargo test

# Integration tests (requires NATS server)
cargo test --features integration-tests

# Example
cargo run --example basic
```

### Documentation

```bash
cargo doc --open
```

## 🌐 NATS Server Setup

For production use with c010.promptexecution.com:

1. **JWT Authentication**: Configured via b00t-backend-nats
2. **Namespace Isolation**: `account.{github-username}.*` 
3. **Role Permissions**: Based on agent role (ai-assistant, ci-cd, etc.)

See [b00t-backend-nats](../b00t-backend-nats/) for server configuration.

## 🔒 Security Configuration

### JWT Validation

When using JWT authentication, the `B00T_OPERATOR_JWT` environment variable **MUST** be set for secure operation:

```bash
export B00T_OPERATOR_JWT="your-operator-jwt-here"
```

This operator JWT is used to derive the signing secret for validating agent JWTs. Without it:
- **With JWT token in config**: Agent initialization will **fail hard** with authentication error
- **Without JWT token in config**: Agent runs without authentication (development mode only)

⚠️ **Security Warning**: Never use placeholder or hardcoded secrets in production. The library enforces this by requiring the operator JWT environment variable when JWT authentication is used.

### Development Mode

For local development without NATS authentication:
1. Do not set `jwt_token` in `AgentConfig`
2. Agent will run without namespace enforcement
3. A warning will be logged: "No JWT provided - running without authentication"

### Production Mode

For production deployments:
1. Set `B00T_OPERATOR_JWT` environment variable with your operator JWT
2. Provide agent JWT via `AgentConfig.with_jwt()` or `B00T_HIVE_JWT` environment variable
3. Agent will validate JWT and enforce namespace isolation
4. Any validation failures will terminate agent initialization

## 📚 Use Cases

### AI Agent Coordination

```rust
// Claude agent proposing code changes
agent.send_propose("code_change", json!({
    "file": "src/main.rs",
    "changes": [
        {"line": 42, "content": "println!(\"Hello, b00t!\");"}
    ],
    "reason": "Add greeting message"
})).await?;
```

### CI/CD Pipeline Coordination

```rust
// Deploy agent coordinating with monitoring
agent.send_status("deployment_start", json!({
    "environment": "production",
    "version": "v1.2.3",
    "services": ["api", "frontend"]
})).await?;
```

### Multi-Agent Workflows

```rust
// Research agent coordinating with analysis agent
agent.send_propose("data_analysis", json!({
    "dataset": "/data/metrics.json", 
    "analysis_type": "trend_analysis",
    "timeframe": "30_days"
})).await?;
```

## 🤝 Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Add tests for new functionality
5. Ensure all tests pass
6. Submit a pull request

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🔗 Related Projects

- [b00t-website](../b00t-website/): Web dashboard and GitHub OAuth
- [b00t-backend-nats](../b00t-backend-nats/): NATS server configuration
- [b00t-cli](../b00t-cli/): Command-line tools
- [Claude Desktop](https://claude.ai/): AI assistant integration

---

**🥾 Built with b00t - The extreme programming agent toolkit**