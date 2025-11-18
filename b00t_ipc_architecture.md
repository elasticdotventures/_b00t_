# b00t-mcp: Comprehensive IPC and Agent Communication Architecture

## Executive Summary

b00t-mcp provides a sophisticated, multi-layered agent communication infrastructure built on top of the **Agent Coordination Protocol (ACP)**. The system supports both **local inter-process communication (IPC)** via Unix domain sockets and **distributed federation** via NATS messaging, with JWT-based security and namespace enforcement for multi-tenant agent coordination.

---

## 1. Architecture Overview

### Core Components

```
┌─────────────────────────────────────────────────────────────┐
│                        b00t System                          │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐ │
│  │  b00t-cli    │  │  b00t-mcp    │  │  MCP Agents      │ │
│  │ (CLI Client) │  │ (MCP Server) │  │ (LLM Powered)    │ │
│  └──────┬───────┘  └──────┬───────┘  └────────┬─────────┘ │
│         │                 │                   │           │
│         └─────────────────┼───────────────────┘           │
│                           │                               │
│         ┌─────────────────▼─────────────────────┐        │
│         │    Agent Coordination Layer (ACP)     │        │
│         │                                       │        │
│         │  - Message Types (STATUS, PROPOSE,   │        │
│         │    STEP)                             │        │
│         │  - Step Synchronization (Barriers)   │        │
│         │  - Namespace Enforcement (JWT)       │        │
│         └─────────────────┬─────────────────────┘        │
│                           │                               │
│  ┌────────────────────────┼────────────────────────┐    │
│  │        Transport Layer                          │    │
│  │                                                  │    │
│  │ ┌───────────────────────────────────────────┐  │    │
│  │ │ Local Socket Transport                    │  │    │
│  │ │ (UnixStream @ ~/.b00t/chat.channel.socket)│  │    │
│  │ └───────────────────────────────────────────┘  │    │
│  │                                                  │    │
│  │ ┌───────────────────────────────────────────┐  │    │
│  │ │ NATS Transport (async-nats)               │  │    │
│  │ │ (Server: c010.promptexecution.com:4222)   │  │    │
│  │ └───────────────────────────────────────────┘  │    │
│  └──────────────────────────────────────────────────┘    │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Key Layers

1. **CLI Layer** (`b00t-cli`): Command-line interface for agent coordination
2. **MCP Server Layer** (`b00t-mcp`): Model Context Protocol server exposing agent tools
3. **ACP Protocol Layer** (`b00t-lib-chat`): Core Agent Coordination Protocol implementation
4. **Transport Layer**: Local socket (IPC) and NATS (distributed messaging)

---

## 2. Agent Coordination Protocol (ACP)

### 2.1 Core Message Types

The ACP defines three fundamental message types:

#### STATUS
- **Purpose**: Convey current state or logs of an agent
- **Used for**: Progress updates, heartbeats, state reporting
- **Payload**: Arbitrary JSON with description field
- **Example**:
```json
{
  "step": 1,
  "agent_id": "claude.124435",
  "type": "STATUS",
  "payload": {
    "description": "Processing initial analysis",
    "progress": 25,
    "timestamp": "2025-03-04T05:30:00Z"
  },
  "timestamp": "2025-03-04T05:30:00Z",
  "message_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### PROPOSE
- **Purpose**: Suggest an action, plan, or mutation
- **Used for**: Proposing deployments, feature flags, resource changes
- **Payload**: Arbitrary JSON with action field
- **Example**:
```json
{
  "step": 3,
  "agent_id": "deployment-orchestrator",
  "type": "PROPOSE",
  "payload": {
    "action": "deploy_to_staging",
    "service": "api-backend",
    "version": "v2.1.0",
    "timestamp": "2025-03-04T05:32:00Z"
  },
  "timestamp": "2025-03-04T05:32:00Z",
  "message_id": "6e7f8400-e29b-41d4-a716-446655440001"
}
```

#### STEP
- **Purpose**: Mark the completion of a synchronization step
- **Used for**: Barrier synchronization, coordination points
- **Payload**: Minimal (step number)
- **Example**:
```json
{
  "step": 5,
  "agent_id": "claude.124435",
  "type": "STEP",
  "payload": {"step": 5},
  "timestamp": "2025-03-04T05:35:00Z",
  "message_id": "7f8f8400-e29b-41d4-a716-446655440002"
}
```

### 2.2 Message Structure

All ACP messages MUST include:
- `step` (u64): Monotonically increasing step number
- `agent_id` (String): Unique agent identifier (e.g., "claude.124435")
- `type` (MessageType): One of STATUS, PROPOSE, STEP
- `payload` (JSON): Arbitrary structured data
- `timestamp` (DateTime<UTC>): ISO 8601 timestamp
- `message_id` (Optional UUID): Unique message ID for tracking
- `correlation_id` (Optional UUID): For request/response patterns

### 2.3 Step Synchronization Barrier

The `StepBarrier` mechanism ensures all agents complete a step before advancing:

```rust
pub struct StepBarrier {
    current_step: u64,                          // Current coordination step
    known_agents: Vec<String>,                  // List of all agents in group
    step_completions: HashMap<u64, Vec<String>>, // Tracks which agents completed
    timeout_ms: u64,                            // Synchronization timeout
}
```

**Synchronization Flow**:
1. All agents start at step 0
2. When an agent completes its work, it sends a STEP message
3. StepBarrier records completion: `barrier.record_step_completion(step, agent_id)`
4. When all agents complete: `barrier.is_step_complete(step)` returns true
5. Coordinator advances: `barrier.try_advance_step()` → new step
6. If timeout occurs: `barrier.force_advance_step()` (forced progression)

---

## 3. MCP Tools for Agent Communication

The b00t-mcp server exposes the following MCP tools for agent-to-agent communication:

### 3.1 Agent Discovery & Management

#### `agent_discover`
- **Purpose**: Discover available agents in the system
- **Parameters**:
  - `capabilities` (optional): Filter by required capabilities
  - `crew` (optional): Filter by crew membership
  - `role` (optional): Filter by agent role (ai-assistant, ci-cd, monitoring)
  - `json` (optional): Output in JSON format

**Example Usage** (via MCP):
```json
{
  "name": "agent_discover",
  "arguments": {
    "capabilities": "code_execution,testing",
    "role": "ci-cd"
  }
}
```

### 3.2 Direct Messaging

#### `agent_message`
- **Purpose**: Send direct message to a specific agent
- **Parameters**:
  - `to_agent` (required): Target agent ID
  - `subject` (required): Message subject/topic
  - `content` (required): Message body
  - `ack` (optional): Require acknowledgment

**Example Usage**:
```json
{
  "name": "agent_message",
  "arguments": {
    "to_agent": "ci-pipeline-runner",
    "subject": "run_tests",
    "content": "Please run test suite for branch feature/new-api",
    "ack": true
  }
}
```

### 3.3 Task Delegation (Captain Only)

#### `agent_delegate`
- **Purpose**: Delegate a task to a worker agent (captain role only)
- **Parameters**:
  - `worker` (required): Worker agent ID
  - `task_id` (required): Unique task identifier
  - `description` (required): Task description
  - `capabilities` (optional): Required capabilities filter
  - `deadline` (optional): Deadline in minutes
  - `priority` (optional): Priority level
  - `blocking` (optional): Block until completion

**Example Usage**:
```json
{
  "name": "agent_delegate",
  "arguments": {
    "worker": "deployment-agent",
    "task_id": "task-deploy-v2.1.0",
    "description": "Deploy v2.1.0 to staging environment with smoke tests",
    "deadline": "30",
    "priority": "high",
    "blocking": true
  }
}
```

### 3.4 Task Completion (Worker Response)

#### `agent_complete`
- **Purpose**: Report task completion (worker responds to captain)
- **Parameters**:
  - `captain` (required): Captain agent ID
  - `task_id` (required): Task ID being completed
  - `status` (required): Completion status (success, failure, timeout)
  - `result` (optional): Result description
  - `artifacts` (optional): Output artifact paths

**Example Usage**:
```json
{
  "name": "agent_complete",
  "arguments": {
    "captain": "orchestrator-agent",
    "task_id": "task-deploy-v2.1.0",
    "status": "success",
    "result": "Deployment completed successfully with all smoke tests passing",
    "artifacts": "/tmp/deployment-logs.txt,/tmp/test-results.json"
  }
}
```

### 3.5 Progress Reporting

#### `agent_progress`
- **Purpose**: Report progress on an ongoing task
- **Parameters**:
  - `task_id` (required): Task ID
  - `progress` (required): Progress percentage (0-100)
  - `message` (required): Status message
  - `eta` (optional): Estimated completion in minutes

**Example Usage**:
```json
{
  "name": "agent_progress",
  "arguments": {
    "task_id": "task-deploy-v2.1.0",
    "progress": "45",
    "message": "Deploying to staging environment",
    "eta": "15"
  }
}
```

### 3.6 Hive Mission Coordination

#### `acp_hive_join`
- **Purpose**: Join an existing hive mission
- **Parameters**:
  - `mission_id`: Mission identifier
  - `role`: Agent role in mission
  - `namespace` (optional): Agent namespace
  - `nats_url` (optional): NATS server URL

#### `acp_hive_create`
- **Purpose**: Create a new hive mission
- **Parameters**:
  - `mission_id`: New mission identifier
  - `expected_agents`: Expected number of participating agents
  - `description`: Mission description
  - `role`: Agent role in mission

#### `acp_hive_status`, `acp_hive_propose`, `acp_hive_step_sync`
- **Purpose**: Send status/proposals, synchronize steps within hive missions

### 3.7 Voting & Consensus

#### `agent_vote_create`
- **Purpose**: Create a voting proposal (captain only)
- **Parameters**:
  - `subject`: Proposal subject
  - `description`: Full description
  - `options`: JSON array of voting options
  - `vote_type`: Voting mechanism (plurality, consensus, etc.)
  - `deadline`: Voting deadline in minutes
  - `voters`: Eligible voters (comma-separated agent IDs)

#### `agent_vote_submit`
- **Purpose**: Submit a vote on a proposal
- **Parameters**:
  - `proposal_id`: Proposal ID to vote on
  - `vote`: Vote choice (JSON)
  - `reasoning` (optional): Justification

### 3.8 Event Notification

#### `agent_notify`
- **Purpose**: Send event notifications to agents
- **Parameters**:
  - `event_type`: Type of event (file_created, pr_opened, deployment_complete)
  - `source`: Event source system
  - `details`: Event details (JSON)
  - `agents` (optional): Target specific agents

### 3.9 Capability Requests

#### `agent_capability`
- **Purpose**: Request agents with specific capabilities
- **Parameters**:
  - `capabilities`: Required capabilities (comma-separated)
  - `description`: What you need to accomplish
  - `urgency` (optional): Request urgency

### 3.10 Waiting & Blocking

#### `agent_wait`
- **Purpose**: Block until receiving a message (blocking call)
- **Parameters**:
  - `from_agent` (optional): Filter by sender agent
  - `message_type` (optional): Filter by message type
  - `subject` (optional): Filter by subject
  - `task_id` (optional): Filter by task ID
  - `timeout` (default: 300s): Timeout in seconds

---

## 4. b00t-cli Agent Coordination Commands

### 4.1 Chat/Message Interface

The `b00t-cli chat` command provides the Agent Coordination Protocol (ACP) interface:

```bash
# Send message to coordination socket
b00t-cli chat send \
  --channel mission.delta \
  --message "Deployment complete" \
  --metadata '{"env":"prod"}'

# Show chat transport information
b00t-cli chat info
# Output:
#   🥾 Local chat socket: /home/brianh/.b00t/chat.channel.socket
#   📡 Available transports: local, nats (stub)
```

**Transport Options**:
- `--transport local` (default): Unix domain socket at `~/.b00t/chat.channel.socket`
- `--transport nats`: NATS message bus (authenticated)

### 4.2 Session Management

```bash
b00t-cli session init          # Initialize new session
b00t-cli session status        # Show session status
b00t-cli session end           # End session
b00t-cli session get <key>     # Get session value
b00t-cli session set <key> <value>
```

### 4.3 Identity & Context

```bash
b00t-cli whoami                # Show agent identity and context
# Output:
#   🤖 Claude Code PID:25542
#   🥾 Agent at PromptExecution
#   🧠 Operator: they/them (github:@elasticdotventures)
```

---

## 5. Local IPC Transport

### 5.1 Unix Domain Socket

**Location**: `~/.b00t/chat.channel.socket`

**Protocol**: JSON Lines (newline-delimited JSON)

**Lifecycle**:
1. b00t-mcp initializes and calls `spawn_local_server(inbox)`
2. Server binds Unix domain socket
3. Clients connect and send newline-delimited JSON messages
4. Messages queued in `ChatInbox`
5. MCP server drains inbox before responding
6. Response includes indicator: `<🥾>{ "chat": { "msgs": N }}</🥾>`

### 5.2 Message Format

```json
{
  "channel": "mission.delta",
  "sender": "frontend.agent",
  "body": "handoff complete",
  "metadata": {"ticket": "OPS-123"},
  "timestamp": "2025-03-04T05:30:00Z"
}
```

**Fields**:
- `channel`: Logical channel (team, mission, crew)
- `sender`: Free-form sender descriptor
- `body`: Plain text message
- `metadata`: Optional structured data (JSON)
- `timestamp`: RFC 3339 UTC timestamp

### 5.3 Client Example (Rust)

```rust
use b00t_chat::{ChatClient, ChatMessage};

let client = ChatClient::local_default()?;
let msg = ChatMessage::new("mission.delta", "agent-x", "Status update")
    .with_metadata(serde_json::json!({"step": 5}));
client.send(&msg).await?;
```

---

## 6. NATS Transport (Distributed)

### 6.1 Configuration

**Default Server**: `nats://c010.promptexecution.com:4222`

**Subject Pattern**: `chat.{channel}`

**Authentication**: JWT-based with environment variables:
- `NATS_URL`: Override NATS server address
- `B00T_HIVE_JWT`: JWT token for namespace authentication

### 6.2 NATS Features

- Pub/Sub messaging for distributed agent coordination
- Subject-based routing with wildcards
- Queue groups for load balancing
- Request/Response patterns
- Message ordering guarantees

### 6.3 Federation

NATS enables federation of multiple b00t clusters:
- **Leaf nodes**: Remote clusters connect to primary
- **Jetstream**: Durable message persistence
- **Clustering**: High availability setup

---

## 7. Security & Namespace Enforcement

### 7.1 JWT-Based Authentication

**Components**:
- `AcpJwtValidator`: Validates and parses JWT tokens
- `AcpSecurityContext`: Holds validated claims
- `NamespaceEnforcer`: Enforces namespace boundaries

### 7.2 Namespace Structure

```
account.{hive}.{role}

Examples:
- account.engineering.ai-assistant
- account.devops.ci-cd
- account.monitoring.bot
```

**Enforcement**:
- Agents can only receive messages within their namespace
- NATS subjects scoped to namespace prefix
- JWT claims include namespace and role

### 7.3 Agent Identity

Format: `{type}.{identifier}`

Examples:
- `claude.124435` (LLM agent)
- `mcp_agent_abc12345` (MCP-based agent)
- `ci-pipeline-runner` (Dedicated service)
- `deployment-orchestrator` (Leader agent)

---

## 8. Step Synchronization Deep Dive

### 8.1 Barrier Pattern

The step barrier ensures strict sequential coordination:

```
Agent A: Step 0 ───┐
Agent B: Step 0 ───┼──► All at Step 0? YES ──► Advance
Agent C: Step 0 ───┘

Agent A: Step 1 ───┐
Agent B: (waiting) ├──► All at Step 1? NO ──► WAIT
Agent C: (waiting) ┘      (timeout: 30s)

Agent A: Step 1 ───┐
Agent B: Step 1 ───┼──► All at Step 1? YES ──► Advance
Agent C: Step 1 ───┘
```

### 8.2 Usage Example

```rust
// Initialize barrier with known agents
let mut barrier = StepBarrier::new(
    vec!["agent1".to_string(), "agent2".to_string()],
    30000 // 30 second timeout
);

// Agent completes work
barrier.record_step_completion(0, "agent1".to_string());
barrier.record_step_completion(0, "agent2".to_string());

// Check and advance
if barrier.is_step_complete(0) {
    barrier.try_advance_step(); // Now at step 1
}

// Get pending agents
let pending = barrier.pending_agents(1);
// Returns agents that haven't reported step 1 completion
```

### 8.3 Timeout Behavior

- **Soft timeout**: Log warning, list pending agents
- **Hard timeout**: Force advance to next step, continue
- **Cleanup**: Retain only last N steps to prevent memory growth

---

## 9. Hive Mission Coordination

### 9.1 Mission Model

```rust
pub struct HiveMission {
    pub mission_id: String,           // Unique identifier
    pub namespace: String,            // Account.hive.role
    pub expected_agents: usize,       // Participant count
    pub current_step: u64,            // Current coordination step
    pub timeout_seconds: u64,         // Operation timeout
    pub description: String,          // Mission purpose
}
```

### 9.2 Agent Status Tracking

```rust
pub struct AgentStatus {
    pub agent_id: String,             // Agent identifier
    pub step: u64,                    // Current step
    pub status: String,               // Status description
    pub last_seen: DateTime<Utc>,     // Last activity
    pub role: String,                 // Role in mission
}
```

### 9.3 Hive Operations

**Create Mission**:
```
Captain Agent creates HiveMission with expected_agents=3
→ Registers as leader
→ Broadcasts to namespace
```

**Join Mission**:
```
Worker Agent 1 → JOIN mission_id → Subscribe to namespace subjects
Worker Agent 2 → JOIN mission_id → Subscribe to namespace subjects
Worker Agent 3 → JOIN mission_id → Subscribe to namespace subjects
```

**Coordinate**:
```
Agent 1 → STATUS "processing"
Agent 2 → STATUS "ready"
Agent 3 → STATUS "ready"
Captain → All agents ready? → Advance step
```

---

## 10. MCP Server Integration

### 10.1 Tool Registration

b00t-mcp exposes agent communication tools via MCP:

```rust
// From mcp_tools.rs
pub async fn derive_agent_tools() -> Vec<MCPTool> {
    vec![
        MCPTool::new("agent_discover", "Discover available agents"),
        MCPTool::new("agent_message", "Send message to agent"),
        MCPTool::new("agent_delegate", "Delegate task to worker"),
        MCPTool::new("agent_complete", "Report task completion"),
        MCPTool::new("agent_progress", "Report progress"),
        MCPTool::new("agent_vote_create", "Create voting proposal"),
        MCPTool::new("agent_vote_submit", "Submit vote"),
        // ... more tools
    ]
}
```

### 10.2 Inbox Architecture

```rust
#[derive(Clone)]
pub struct ChatRuntime {
    inbox: ChatInbox,
}

// Global singleton
pub fn global() -> Self {
    // Spawn local socket server
    // Return runtime with inbox access
}

// Drain messages before response
pub async fn drain_indicator(&self) -> String {
    let messages = self.inbox.drain().await;
    // Append indicator to response
}
```

### 10.3 ACL (Access Control List)

**File**: `~/.dotfiles/b00t-mcp-acl.toml`

Controls which commands MCP agents can invoke:
- Whitelist allowed commands
- Namespace-based restrictions
- Role-based access control
- JWT validation

---

## 11. Example: Multi-Agent Workflow

### 11.1 Scenario: CI/CD Pipeline Orchestration

```
┌──────────────────────────────────────────────────────────┐
│              Multi-Agent CI/CD Pipeline                 │
└──────────────────────────────────────────────────────────┘

Phase 1: Code Analysis (Step 0)
  🔍 Code-Review Agent
  └─→ STATUS "Analyzing code quality"
  🧪 Test Agent  
  └─→ STATUS "Preparing test environment"

[All agents report STEP 0 completion]
[Barrier advances to Step 1]

Phase 2: Execution (Step 1)
  🧪 Test Agent
  └─→ STATUS "Running unit tests"
  └─→ PROPOSE "Run integration tests"
  🔄 Deployment Agent
  └─→ STATUS "Building artifacts"

[All agents report STEP 1 completion]
[Barrier advances to Step 2]

Phase 3: Decision (Step 2)
  👨‍⚖️ Approval Agent
  └─→ STATUS "Reviewing deployment proposal"
  └─→ PROPOSE "Deploy to staging"
  🚀 Release Agent
  └─→ STATUS "Waiting for approval"

[Vote: Deploy to staging? YES]
[Barrier advances to Step 3]

Phase 4: Deployment (Step 3)
  🚀 Release Agent
  └─→ STATUS "Deploying to staging"
  └─→ PROPOSE "Run smoke tests"
  📊 Monitoring Agent
  └─→ STATUS "Monitoring health metrics"

[All agents report STEP 3 completion]
[Mission complete]
```

### 11.2 Message Flow

```rust
// Agent 1: Code Review
let review_agent = Agent::new(config).await?;
review_agent.send_status("Analyzing code quality", json!({"files": 42})).await?;
review_agent.complete_step().await?;

// Agent 2: Testing  
let test_agent = Agent::new(config).await?;
test_agent.send_status("Tests passing", json!({"coverage": 95})).await?;
test_agent.complete_step().await?;

// Barrier checks completion
barrier.is_step_complete(0)? // true when all agents done
barrier.try_advance_step()   // Move to step 1

// Agent 1: Propose action
review_agent.send_propose("merge_approved", json!({
  "reason": "Code quality acceptable"
})).await?;

// Agent 2: Acknowledgment
test_agent.send_status("All tests green", json!({
  "tests_passed": 156
})).await?;
```

---

## 12. Debugging & Monitoring

### 12.1 Chat Socket Inspection

```bash
# Check socket exists and is listening
ls -l ~/.b00t/chat.channel.socket

# Monitor messages in real-time
nc -U ~/.b00t/chat.channel.socket | jq .

# Send test message
b00t-cli chat send --channel test --message "hello"
```

### 12.2 Logging

Enable tracing logs:
```bash
export RUST_LOG=debug
b00t-mcp --stdio
```

**Key log messages**:
- `🐝 Agent {id} initialized` - Agent startup
- `Sending hive status: {desc}` - Status updates
- `Advanced to step {n}` - Barrier progression
- `Step {n} timed out, forcing advancement` - Timeout handling

### 12.3 Health Checks

```bash
# Check transport
b00t-cli chat info

# Check NATS connectivity
nats context ls
nats pub test.hello "Hello" --server=nats://c010.promptexecution.com:4222

# Verify agent identity
b00t-cli whoami
```

---

## 13. Future Enhancements

### Planned Improvements

1. **Distributed Tracing**: OpenTelemetry integration for end-to-end visibility
2. **Message Durability**: Jetstream persistence for critical operations
3. **Load Balancing**: NATS queue groups for horizontal scaling
4. **Circuit Breakers**: Graceful degradation on agent failures
5. **Rate Limiting**: Per-agent and per-namespace rate controls
6. **Audit Logging**: Complete message audit trail
7. **WebSocket Support**: Real-time UI integration
8. **gRPC Transport**: High-performance binary protocol option

---

## 14. Summary: Key Takeaways

### Agent Communication Layers

1. **Protocol Layer (ACP)**
   - 3 message types: STATUS, PROPOSE, STEP
   - Step barrier for synchronization
   - JWT-based security and namespaces

2. **Transport Layer**
   - Local: Unix domain socket (IPC)
   - Distributed: NATS message bus
   - Pluggable architecture

3. **MCP Integration**
   - 20+ tools for agent coordination
   - Task delegation (captain→worker)
   - Progress tracking and voting

4. **Orchestration**
   - Hive missions for multi-agent workflows
   - Step synchronization barriers
   - Namespace-enforced isolation

### When to Use Each Component

- **Chat/Socket**: Simple agent-to-agent messages, local coordination
- **ACP Messages**: Structured coordination with explicit step progression
- **Hive Missions**: Multi-agent workflows with barrier synchronization
- **Task Delegation**: Captain-worker patterns with acknowledged completion
- **Voting**: Consensus-based decisions across agent groups

