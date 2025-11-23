# /k0mmand3r - Agent Command Interface for b00t

## Overview

`/k0mmand3r` is a command-based interface for agents to communicate, coordinate, and dispatch tasks within the b00t ecosystem. It extends the Agent Coordination Protocol (ACP) with human-friendly slash commands that map to b00t-mcp tools and IPC primitives.

## Architecture

```
┌──────────────────────────────────────────────────────┐
│        Agent (Claude, Codex, Custom Workers)         │
│                                                      │
│         Issues: /k0mmand3r dispatch codex ...       │
└──────────────────────────────────────────────────────┘
                         ▼
┌──────────────────────────────────────────────────────┐
│              /k0mmand3r Parser                       │
│  • Parses slash command syntax                       │
│  • Maps to b00t-mcp tools                            │
│  • Handles authentication/authorization              │
└──────────────────────────────────────────────────────┘
                         ▼
┌──────────────────────────────────────────────────────┐
│           b00t Agent Coordination (ACP)              │
│  • Transport: Local socket / NATS                    │
│  • Message types: STATUS, PROPOSE, STEP              │
│  • Step barriers, hive missions, voting              │
└──────────────────────────────────────────────────────┘
                         ▼
┌──────────────────────────────────────────────────────┐
│              Target Agent (e.g., Codex)              │
│  • Receives command via ACP                          │
│  • Executes task with IPC status filtering          │
│  • Returns result via ACP response                   │
└──────────────────────────────────────────────────────┘
```

## Command Syntax

```
/k0mmand3r <verb> <target> [options] [-- arguments]
```

### Core Verbs

| Verb | Purpose | Maps to b00t Tool |
|------|---------|-------------------|
| `dispatch` | Send task to agent | `agent-delegate` |
| `status` | Query agent/task status | `agent-progress` |
| `message` | Send message to agent | `agent-message` |
| `wait` | Block until response | `agent-wait` |
| `discover` | Find available agents | `agent-discover` |
| `vote` | Create/submit vote | `agent-vote-create`, `agent-vote-submit` |
| `notify` | Broadcast event | `agent-notify` |
| `capability` | Request agent with skill | `agent-capability` |
| `complete` | Mark task done | `agent-complete` |

### Target Types

- `codex` - OpenAI Codex (GPT-5) agent
- `worker` - Generic worker agent
- `crew:<name>` - Specific crew/team
- `agent:<id>` - Specific agent ID
- `channel:<name>` - Chat channel broadcast

## Examples

### 1. Dispatch Task to Codex

```bash
/k0mmand3r dispatch codex \
  --task "analyze auth system" \
  --context \
  --priority high \
  --sandbox read-only \
  -- model=gpt-5-codex reasoning=high
```

**Maps to:**
```bash
# b00t-mcp tool
mcp__b00t-mcp__b00t_agent_delegate \
  --worker "agent.codex-001" \
  --task_id "task-$(uuidgen)" \
  --description "analyze auth system" \
  --priority "high"

# Codex execution (by worker)
codex exec -m gpt-5-codex \
  --config model_reasoning_effort="high" \
  --sandbox read-only \
  "analyze auth system" 2>/dev/null
```

### 2. Query Status with Filtering

```bash
/k0mmand3r status agent:codex-001 \
  --filter "progress|complete|error" \
  --format compact
```

**Maps to:**
```bash
# b00t-mcp tool
mcp__b00t-mcp__b00t_agent_progress \
  --task_id "..." \
  --message "status query"

# Status pipe filtering
tail -f ~/.b00t/status.pipe | grep -E '(progress|complete|error)'
```

### 3. Wait for Task Completion

```bash
/k0mmand3r wait agent:codex-001 \
  --timeout 600 \
  --on-progress "echo 'Progress: {}'"
```

**Maps to:**
```bash
# b00t-mcp tool
mcp__b00t-mcp__b00t_agent_wait \
  --from_agent "agent.codex-001" \
  --timeout "600" \
  --message_type "completion"
```

### 4. Discover Capable Agents

```bash
/k0mmand3r discover \
  --capabilities "code-analysis,refactoring" \
  --crew engineering \
  --format json
```

**Maps to:**
```bash
# b00t-mcp tool
mcp__b00t-mcp__b00t_agent_discover \
  --capabilities "code-analysis,refactoring" \
  --crew "engineering"
```

### 5. Broadcast Event Notification

```bash
/k0mmand3r notify crew:engineering \
  --event deployment-complete \
  --details '{"version":"2.4.1","environment":"staging"}'
```

**Maps to:**
```bash
# b00t-mcp tool
mcp__b00t-mcp__b00t_agent_notify \
  --event_type "deployment-complete" \
  --source "agent.deployer" \
  --details '{"version":"2.4.1","environment":"staging"}'

# b00t chat broadcast
b00t-cli chat send \
  --channel engineering \
  --message '{"type":"STATUS","event":"deployment-complete","details":{...}}'
```

### 6. Create Consensus Vote

```bash
/k0mmand3r vote create \
  --subject "Adopt TypeScript for new services" \
  --options "approve,reject,defer" \
  --voters "agent.architect,agent.backend,agent.frontend" \
  --deadline 3600
```

**Maps to:**
```bash
# b00t-mcp tool
mcp__b00t-mcp__b00t_agent_vote_create \
  --subject "Adopt TypeScript..." \
  --options '["approve","reject","defer"]' \
  --vote_type "majority" \
  --deadline "3600" \
  --voters "agent.architect,agent.backend,agent.frontend"
```

## Status Message Filtering

/k0mmand3r implements intelligent status filtering using Unix pipes and `tee`:

```bash
# Status pipeline architecture
codex exec "task" 2>&1 | \
  tee >(grep -E 'ERROR|FATAL' > errors.log) | \
  tee >(grep -E 'step|progress' | /k0mmand3r format-status) | \
  grep -vE 'debug|trace' | \
  /k0mmand3r filter --user-facing
```

### Filter Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `critical` | Errors and failures only | Production monitoring |
| `compact` | Key milestones only | User-facing updates |
| `verbose` | All events except debug | Development |
| `debug` | Everything including traces | Troubleshooting |

### Status Message Format

```json
{
  "timestamp": "2025-11-17T13:15:00Z",
  "agent": "agent.codex-001",
  "task_id": "task-abc123",
  "type": "progress",
  "level": "info",
  "message": "Analyzing authentication module",
  "progress": 45,
  "eta_seconds": 120
}
```

## IPC Transport

/k0mmand3r supports multiple transport backends:

### 1. Unix Named Pipes (Local)

```bash
# Create pipes
mkfifo ~/.b00t/k0mmand3r/{request,response,status}.pipe

# Request processor
tail -f ~/.b00t/k0mmand3r/request.pipe | \
  while read cmd; do
    /k0mmand3r parse "$cmd" | \
      b00t-cli chat send --channel commands
  done
```

### 2. Unix Domain Socket (Local)

```bash
# Socket listener
socat UNIX-LISTEN:~/.b00t/k0mmand3r.sock,fork \
  EXEC:"/k0mmand3r process",pipes
```

### 3. NATS (Distributed)

```bash
# Publish command
/k0mmand3r dispatch codex --publish nats://c010.promptexecution.com:4222

# Subscribe to responses
nats sub "k0mmand3r.responses.>" | \
  /k0mmand3r filter --compact
```

### 4. MQTT (Event Bus)

```bash
# Bridge to MQTT
mosquitto_pub \
  -h localhost:1883 \
  -t "k0mmand3r/dispatch/codex" \
  -m '{"task":"analyze auth"}'

# Subscribe filtered
mosquitto_sub \
  -h localhost:1883 \
  -t "k0mmand3r/status/#" | \
  /k0mmand3r filter --user-facing
```

## Implementation

### Core Components

```
/k0mmand3r
├── parser/           # Command syntax parsing
│   ├── lexer.rs     # Token extraction
│   ├── validator.rs # Schema validation
│   └── mapper.rs    # Maps to b00t tools
├── transport/        # IPC backends
│   ├── pipe.rs      # Named pipes
│   ├── socket.rs    # Unix sockets
│   ├── nats.rs      # NATS client
│   └── mqtt.rs      # MQTT client
├── filter/          # Status filtering
│   ├── stream.rs    # Stream processing
│   ├── format.rs    # Output formatting
│   └── router.rs    # Message routing
└── agents/          # Agent dispatchers
    ├── codex.rs     # Codex integration
    ├── worker.rs    # Generic workers
    └── crew.rs      # Crew coordination
```

### b00t-mcp Integration

```rust
// pseudocode
impl K0mmand3rTool {
    async fn dispatch(&self, cmd: Command) -> Result<Response> {
        match cmd.verb {
            Verb::Dispatch => {
                // Map to agent-delegate
                let delegate_params = map_to_delegate(&cmd);
                self.mcp_client.call("agent-delegate", delegate_params).await
            }
            Verb::Status => {
                // Map to agent-progress
                let progress_params = map_to_progress(&cmd);
                self.mcp_client.call("agent-progress", progress_params).await
            }
            // ... other verbs
        }
    }
}
```

### Status Filtering Pipeline

```rust
// pseudocode
impl StatusFilter {
    fn process_stream<R: Read>(
        &self,
        input: R,
        mode: FilterMode
    ) -> impl Stream<Item = StatusMessage> {
        BufReader::new(input)
            .lines()
            .filter_map(|line| parse_status(line))
            .filter(|msg| self.should_show(msg, mode))
            .map(|msg| format_status(msg, mode))
    }
}
```

## Security

### Authentication
- JWT tokens via b00t-cli identity
- Namespace enforcement: `account.{org}.{role}`
- Agent identity validation

### Authorization
- ACL-based command filtering
- Crew membership checks
- Capability-based access

### Audit Trail
```bash
# All commands logged
~/.b00t/audit/k0mmand3r.log

# Format:
# [2025-11-17T13:15:00Z] agent.claude-001 -> /k0mmand3r dispatch codex ...
# [2025-11-17T13:15:01Z] Response: {"task_id":"...","status":"accepted"}
```

## Configuration

### ~/.b00t/k0mmand3r.toml

```toml
[transport]
default = "local-socket"

[transport.local]
socket_path = "~/.b00t/k0mmand3r.sock"
request_pipe = "~/.b00t/k0mmand3r/request.pipe"
response_pipe = "~/.b00t/k0mmand3r/response.pipe"
status_pipe = "~/.b00t/k0mmand3r/status.pipe"

[transport.nats]
url = "nats://c010.promptexecution.com:4222"
channel_prefix = "k0mmand3r"

[filter]
default_mode = "compact"
user_facing_levels = ["info", "warning", "error"]
suppress_patterns = ["^debug:", "^trace:"]

[agents.codex]
model = "gpt-5-codex"
reasoning_effort = "medium"
sandbox = "read-only"
timeout_seconds = 600
```

## Usage Patterns

### Pattern 1: Fire and Forget
```bash
/k0mmand3r dispatch codex --task "analyze code" --async
# Returns immediately with task_id
# Status available via: /k0mmand3r status task:<id>
```

### Pattern 2: Blocking Wait
```bash
/k0mmand3r dispatch codex --task "analyze code" --wait
# Blocks until completion
# Shows filtered progress updates
```

### Pattern 3: Callback on Event
```bash
/k0mmand3r dispatch codex \
  --task "analyze code" \
  --async \
  --on-complete "/k0mmand3r notify crew:dev --event analysis-done"
```

### Pattern 4: Pipeline
```bash
/k0mmand3r dispatch codex --task "find bugs" --async | \
  /k0mmand3r wait --timeout 600 | \
  /k0mmand3r dispatch worker --task "fix bugs from stdin"
```

## Integration Examples

### With Flashbacker
```markdown
<!-- In .claude/commands/fb/codex.md -->
Uses the Bash tool to execute:
```bash
/k0mmand3r dispatch codex \
  --task "$ARGUMENTS" \
  --context "$(flashback agent --context)" \
  --filter compact \
  --wait
```
```

### With PM2 Daemon
```bash
# Start k0mmand3r daemon
pm2 start /k0mmand3r \
  --name k0mmand3r-daemon \
  --interpreter none \
  -- daemon --transport nats

# Send command to daemon
/k0mmand3r dispatch codex --task "analyze" --daemon
```

### With CI/CD
```yaml
# .github/workflows/codex-review.yml
- name: Codex Code Review
  run: |
    /k0mmand3r dispatch codex \
      --task "review PR changes" \
      --sandbox read-only \
      --format github-comment \
      --wait > review.md

    gh pr comment ${{ github.event.pull_request.number }} \
      --body-file review.md
```

## Monitoring & Debugging

### Real-time Status Monitoring
```bash
# Terminal 1: Watch all status
tail -f ~/.b00t/k0mmand3r/status.pipe | /k0mmand3r filter --verbose

# Terminal 2: Dispatch tasks
/k0mmand3r dispatch codex --task "analyze"

# Terminal 3: Monitor errors
tail -f ~/.b00t/k0mmand3r/errors.log
```

### Debug Mode
```bash
export K0MMAND3R_DEBUG=1
export RUST_LOG=debug

/k0mmand3r dispatch codex --task "test" --verbose 2>&1 | tee debug.log
```

### Performance Metrics
```bash
/k0mmand3r stats
# Output:
# Total commands: 1,247
# Average latency: 34ms
# Active agents: 5
# Queue depth: 2
```

## Best Practices

1. **Use --async for long tasks** to avoid blocking
2. **Filter status messages** to prevent context bloat
3. **Set appropriate timeouts** based on task complexity
4. **Use task_id tracking** for async workflows
5. **Leverage callbacks** for event-driven coordination
6. **Monitor status pipes** during development
7. **Use compact mode** for user-facing output
8. **Audit sensitive commands** via audit log

## Future Extensions

- [ ] Web dashboard for command monitoring
- [ ] GraphQL API for command dispatch
- [ ] Workflow templates (YAML-defined multi-step)
- [ ] Rate limiting and quota management
- [ ] Multi-tenancy with namespace isolation
- [ ] Prometheus metrics export
- [ ] OpenTelemetry tracing integration

## See Also

- `/home/brianh/.dotfiles/b00t_ipc_architecture.md` - b00t IPC deep dive
- `/home/brianh/.dotfiles/codex_integration_setup.md` - Codex setup guide
- `/home/brianh/.dotfiles/b00t_quick_reference.md` - b00t tool reference
