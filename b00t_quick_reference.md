# b00t-mcp: Quick Reference Guide

## Available MCP Tools Summary

### Agent Communication Tools (In Codebase)

| Tool Name | Purpose | Captain Only | Blocking |
|-----------|---------|--------------|----------|
| `agent_discover` | Find agents by capabilities, crew, role | No | No |
| `agent_message` | Send direct message to agent | No | No |
| `agent_delegate` | Assign task to worker | Yes | Optional |
| `agent_complete` | Report task completion | No | No |
| `agent_progress` | Update progress on task | No | No |
| `agent_vote_create` | Create voting proposal | Yes | No |
| `agent_vote_submit` | Cast vote | No | No |
| `agent_wait` | Block until message received | No | Yes |
| `agent_notify` | Send event notification | No | No |
| `agent_capability` | Request agents with capabilities | No | No |

### Hive Mission Tools

| Tool Name | Purpose |
|-----------|---------|
| `acp_hive_create` | Create new hive mission |
| `acp_hive_join` | Join existing mission |
| `acp_hive_status` | Send status to hive |
| `acp_hive_propose` | Propose action to hive |
| `acp_hive_step_sync` | Wait for step synchronization |
| `acp_hive_show` | Display mission status |

### Non-Agent Tools (in b00t-mcp)

| Category | Examples |
|----------|----------|
| **CLI Reflection** | `b00t_cli_detect`, `b00t_cli_check`, `b00t_cli_install` |
| **MCP Management** | `b00t_mcp_list`, `b00t_mcp_add` |
| **AI Management** | `b00t_ai_list`, `b00t_ai_output` |
| **Stack Management** | `b00t_stack_*` commands |
| **Learning** | `b00t_learn`, `b00t_grok_ask`, `b00t_grok_digest` |
| **Lessons** | `b00t_lfmf` (lesson from failures) |
| **Advice** | `b00t_advice` for error patterns |

---

## CLI Quick Reference

### Chat Commands

```bash
# Send message to local socket
b00t-cli chat send --channel myteam --message "status update"

# Send with metadata
b00t-cli chat send --channel ops --message "ready" \
  --metadata '{"priority":"high"}'

# Use NATS transport
b00t-cli chat send --transport nats --message "distributed msg"

# Show transport info
b00t-cli chat info
```

### Agent Identity

```bash
# Show current agent info
b00t-cli whoami

# Show session info
b00t-cli session status

# Initialize new session
b00t-cli session init --agent myagent

# Set session value
b00t-cli session set mission_id "mission-alpha"
```

### Learning & Skills

```bash
# Learn about topic (loads skill)
b00t-cli learn bash
b00t-cli learn rust
b00t-cli learn git

# Record lesson learned
b00t-cli lfmf --tool nats --lesson "NATS connect timeout on cold start"

# Get advice for errors
b00t-cli advice --tool bash --query "permission denied"
```

---

## Architecture Quick Map

```
AGENT A                    AGENT B                    AGENT C
   │                          │                          │
   ├─ Send STATUS ──────┐     ├─ Send STATUS ──────┐     ├─ Send STATUS ──────┐
   ├─ Send PROPOSE ─────┼────→ NATS/Socket ←──────┼────→ NATS/Socket
   └─ Send STEP ────────┘     └─ Send STEP ────────┘     └─ Send STEP ────────┘
                                     │
                                     ↓
                            StepBarrier (local)
                            - Tracks completions
                            - Checks all agents done
                            - Advances step (or timeout)
                                     │
                                     ↓
                            MCP Server (b00t-mcp)
                            - Inbox drains messages
                            - Tools respond to requests
                            - ACL validates access
```

---

## Message Format Quick Reference

### ACP Message Template

```json
{
  "step": 1,
  "agent_id": "agent-name-or-id",
  "type": "STATUS|PROPOSE|STEP",
  "payload": {
    // Custom JSON payload
    // For STATUS: include "description"
    // For PROPOSE: include "action"
    // For STEP: minimal {"step": N}
  },
  "timestamp": "2025-03-04T12:00:00Z",
  "message_id": "uuid-string",
  "correlation_id": null  // Optional for request/response
}
```

### Chat Message Template

```json
{
  "channel": "team-or-mission-name",
  "sender": "agent-or-user-id",
  "body": "plain text message",
  "metadata": {
    // Optional structured data
  },
  "timestamp": "2025-03-04T12:00:00Z"
}
```

---

## Namespace Structure

```
account.{organization}.{role}

Examples:
├─ account.engineering.ai-assistant     (engineers' LLM agents)
├─ account.engineering.ci-cd            (CI/CD automation agents)
├─ account.devops.monitoring            (Monitoring/alerting agents)
├─ account.security.audit               (Security audit agents)
└─ account.data.analytics               (Analytics processing agents)
```

**NATS Subject Pattern**: `{namespace}.acp.{step}.{agent_id}.{type}`

Example: `account.engineering.ai-assistant.acp.5.claude.12345.status`

---

## Configuration Files

### b00t-mcp ACL

File: `~/.dotfiles/b00t-mcp-acl.toml`

```toml
[commands]
# Allow chat commands
allowed = ["chat.*", "session.*", "whoami"]

[namespaces]
# Enforce namespace for agents
default_namespace = "account.development.ai-assistant"

[roles]
# Role-based access control
captain = ["agent_delegate", "agent_vote_create"]
worker = ["agent_complete", "agent_progress"]
observer = ["agent_discover", "agent_wait"]
```

---

## Environment Variables

```bash
# NATS Configuration
export NATS_URL="nats://c010.promptexecution.com:4222"
export B00T_HIVE_JWT="eyJ0eXAiOiJKV1QiLCJhbGc..."

# Logging
export RUST_LOG="debug"     # Enable debug logs
export RUST_LOG="b00t=debug" # Just b00t logs

# Development
export _B00T_Path="~/.dotfiles/_b00t_"
```

---

## Common Patterns

### Pattern 1: Simple Status Update

```rust
let agent = Agent::new(config).await?;
agent.send_status("Processing data", json!({"records": 1500})).await?;
agent.complete_step().await?;
```

### Pattern 2: Proposal & Consensus

```rust
// Agent 1: Propose
agent1.send_propose("deploy_to_staging", json!({
    "version": "v2.1.0",
    "tests_passed": true
})).await?;

// Agent 2: Vote
agent2.send_status("Voting on deployment", json!({
    "vote": "approve"
})).await?;

// Coordinator: Advance on consensus
barrier.try_advance_step();
```

### Pattern 3: Task Delegation

```rust
// Captain creates task
captain.delegate(
    "worker-agent",
    "task-123",
    "Deploy service to prod"
).await?;

// Worker reports progress
worker.send_progress("task-123", 50, "Deploying...").await?;

// Worker completes
worker.complete_task("task-123", "success", "Deployment done").await?;
```

### Pattern 4: Hive Mission

```rust
// Create mission
let client = AcpHiveClient::new(
    "leader".to_string(),
    "orchestrator".to_string(),
    mission,
    nats_url
).await?;

// Send status to hive
client.send_status("Starting phase 1", None).await?;

// Propose action
client.propose_action("run_tests", None).await?;

// Wait for synchronization
client.wait_for_step_sync(1, 30).await?;
```

---

## Debugging Checklist

- [ ] Socket exists: `ls ~/.b00t/chat.channel.socket`
- [ ] NATS reachable: `nats -s nats://c010.promptexecution.com:4222 pub test "hi"`
- [ ] Agent identity: `b00t-cli whoami`
- [ ] Session valid: `b00t-cli session status`
- [ ] Logs enabled: `export RUST_LOG=debug`
- [ ] ACL configured: `cat ~/.dotfiles/b00t-mcp-acl.toml`
- [ ] JWT set: `echo $B00T_HIVE_JWT`
- [ ] Namespace correct: Check NATS subject pattern

---

## File Locations

```
~/.b00t/
├── chat.channel.socket              # Local IPC socket
├── .b00t.g0spell.md                 # b00t Gospel (alignment rules)
├── .claude/                         # Claude Code config
├── logs/                            # Log files
├── sessions/                        # Session state
└── secrets/                         # Credentials

~/.dotfiles/_b00t_/
├── b00t.just                        # Main justfile
├── b00t-mcp-acl.toml               # MCP access control
├── _b00t_.toml                      # Configuration
└── learn/                           # Learning materials

/home/brianh/.b00t/
├── b00t-cli/                        # CLI source
├── b00t-mcp/                        # MCP server source
├── b00t-lib-chat/                   # ACP library source
├── b00t-grok/                       # Knowledge base
└── k0mmand3r/                       # Command framework
```

---

## Limits & Constraints

| Item | Limit | Notes |
|------|-------|-------|
| Step timeout | 30 seconds | Default, configurable |
| Message ID | UUID | Tracked for deduplication |
| Correlation ID | Optional | For request/response |
| Payload size | ~1MB | JSON encoded |
| Agent ID length | 255 chars | Namespace + ID |
| Channel name | 255 chars | Logical scope |
| Metadata | Free-form JSON | No schema |

---

## Integration Points

### With Kubernetes

```bash
# Run agent in k8s pod
kubectl run agent --image=b00t-cli:latest \
  -e NATS_URL=nats://nats-cluster:4222 \
  -e B00T_HIVE_JWT=$JWT

# Port forward for socket
kubectl port-forward pod/agent 9999:9999
```

### With CI/CD

```bash
# GitHub Actions
- name: Join hive mission
  run: b00t-cli chat send --channel ci-cd --message "CI started"

# GitLab CI
script:
  - b00t-cli chat send --channel ci-cd --message "Job running"
```

### With Web Frameworks

```rust
// Actix-web integration
use b00t_chat::ChatClient;

#[post("/notify")]
async fn notify(msg: web::Json<ChatMessage>) -> HttpResponse {
    let client = ChatClient::local_default().ok();
    if let Some(client) = client {
        client.send(&msg).await.ok();
    }
    HttpResponse::Ok().finish()
}
```

