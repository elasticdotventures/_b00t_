# PM2 Tasker Integration Guide

## Overview

The PM2 Tasker integrates PM2 process management with b00t's k0mmand3r IPC system, enabling slash command-driven task execution within the b00t ecosystem.

## Architecture

### Components

1. **k0mmand3r Parser** (`/home/user/_b00t_/k0mmand3r/`)
   - Rust-based slash command parser
   - WASM bindings for TypeScript/Python
   - Parses commands like `/start --task=mytask`

2. **Redis Pub/Sub** (`b00t-c0re-lib/src/redis.rs`)
   - IPC message bus for agent coordination
   - Channel: `b00t:k0mmand3r` (default)
   - Status channel: `b00t:task:status`

3. **PM2 Tasker Service** (this directory)
   - TypeScript service listening to Redis
   - Manages task lifecycle via PM2
   - Publishes status updates

4. **PM2 Process Manager** (`_b00t_/pm2.cli.toml`)
   - Node.js-based process manager
   - Handles clustering, monitoring, restarts
   - Integrates with b00t datum system

### Data Flow

```
User/Agent
    |
    | (1) Sends slash command
    v
k0mmand3r Parser
    |
    | (2) Parses & validates
    v
Redis Pub/Sub (b00t:k0mmand3r)
    |
    | (3) Broadcasts message
    v
PM2 Tasker Service
    |
    | (4) Routes command
    v
PM2 Process Manager
    |
    | (5) Executes task
    v
Task Process (running)
    |
    | (6) Status updates
    v
Redis Pub/Sub (b00t:task:status)
    |
    v
Subscribers (monitoring, logging, etc.)
```

## Message Format

### k0mmand3r Message Schema

```typescript
{
  "verb": "start" | "stop" | "restart" | "delete" | "status",
  "params": {
    "task": "task-name",           // Required for most commands
    "command": "node server.js",   // Command to execute
    "datum": "mydatum.cli",        // Optional: datum reference
    "args": "arg1 arg2",           // Space-separated args
    "env_VAR": "value",            // Environment variables (prefix: env_)
    "cwd": "/path/to/dir",         // Working directory
    "instances": "4",              // PM2 cluster instances
    "max_memory_restart": "1G"     // Memory restart threshold
  },
  "content": "optional content",
  "timestamp": "2025-11-17T15:00:00Z",
  "agent_id": "agent-123"
}
```

### Task Status Message

```typescript
{
  "type": "task_status",
  "task_name": "myapp",
  "status": "started" | "stopped" | "restarted" | "deleted",
  "timestamp": "2025-11-17T15:00:00Z",
  "pm_id": 0,  // PM2 process ID (optional)
  // Additional metadata
}
```

## Integration Points

### 1. With k0mmand3r (Rust)

The PM2 Tasker consumes messages parsed by k0mmand3r. To integrate:

```rust
// Example: Publishing from Rust
use redis::Commands;

let client = redis::Client::open("redis://localhost:6379")?;
let mut con = client.get_connection()?;

let message = serde_json::json!({
    "verb": "start",
    "params": {
        "task": "myapp",
        "command": "node app.js"
    },
    "timestamp": chrono::Utc::now().to_rfc3339()
});

con.publish("b00t:k0mmand3r", message.to_string())?;
```

### 2. With b00t-cli

Add PM2 management commands to b00t-cli:

```bash
# Via justfile
just pm2-start mytask "node server.js"
just pm2-stop mytask
just pm2-status

# Or directly via b00t-cli (future enhancement)
b00t-cli pm2 start --task=mytask --command="node server.js"
```

### 3. With b00t-ipc (Agent Coordination)

Agents can send task management messages:

```rust
use b00t_ipc::{Agent, Message, MessageBus};

let agent = Agent::new("alpha", vec!["rust", "devops"]);
let bus = MessageBus::new().await?;

// Send task start command
bus.publish_kommand(K0mmandMessage {
    verb: "start".to_string(),
    params: HashMap::from([
        ("task".to_string(), "worker".to_string()),
        ("command".to_string(), "python worker.py".to_string()),
    ]),
    agent_id: Some(agent.id.clone()),
    timestamp: Utc::now(),
}).await?;
```

### 4. With Docker Container

Run PM2 Tasker in b00t container:

```yaml
# docker-compose.yml
version: '3.8'
services:
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    networks:
      - b00t-network

  pm2-tasker:
    build: ./pm2-tasker
    depends_on:
      - redis
    environment:
      REDIS_URL: redis://redis:6379
      PM2_TASKER_CHANNEL: b00t:k0mmand3r
      LOG_LEVEL: info
    networks:
      - b00t-network
    volumes:
      - pm2-data:/root/.pm2

networks:
  b00t-network:
    driver: bridge

volumes:
  pm2-data:
```

## Command Examples

### Start a Task

```bash
# Basic command
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"api",
    "command":"node api.js"
  }
}'

# With environment variables
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"api",
    "command":"node api.js",
    "env_PORT":"8080",
    "env_NODE_ENV":"production"
  }
}'

# Cluster mode (4 instances)
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"api",
    "command":"node api.js",
    "instances":"4",
    "max_memory_restart":"500M"
  }
}'
```

### Stop a Task

```bash
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"stop",
  "params":{"task":"api"}
}'
```

### Check Status

```bash
# Request status
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"status",
  "params":{}
}'

# Subscribe to status updates
redis-cli SUBSCRIBE b00t:task:status
```

## WASM Interface (Future)

The user mentioned needing a WASM interface to Go. This is NOT currently required since:

1. k0mmand3r already has WASM bindings for TypeScript
2. Redis pub/sub provides language-agnostic IPC
3. Go can publish/subscribe to Redis directly

If needed, we can add Go bindings:

```go
// Example: Go publishing to Redis
package main

import (
    "context"
    "encoding/json"
    "github.com/redis/go-redis/v9"
)

func main() {
    ctx := context.Background()
    rdb := redis.NewClient(&redis.Options{
        Addr: "localhost:6379",
    })

    msg := map[string]interface{}{
        "verb": "start",
        "params": map[string]string{
            "task": "myapp",
            "command": "go run main.go",
        },
    }

    data, _ := json.Marshal(msg)
    rdb.Publish(ctx, "b00t:k0mmand3r", data)
}
```

## Testing

### 1. Unit Tests (TBD)

```bash
cd pm2-tasker
npm test
```

### 2. Integration Tests

```bash
# Terminal 1: Start Redis
docker run -p 6379:6379 redis:7-alpine

# Terminal 2: Start PM2 Tasker
cd pm2-tasker
npm run dev

# Terminal 3: Send test commands
just test-start-cmd
just test-status-cmd
just test-stop-cmd

# Terminal 4: Monitor status
just watch-status
```

### 3. Docker Integration Test

```bash
# Build and run full stack
docker-compose up -d

# Send test command
docker-compose exec redis redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{"task":"test","command":"echo hello"}
}'

# Check logs
docker-compose logs -f pm2-tasker
```

## Troubleshooting

### PM2 Tasker not receiving messages

1. Check Redis connection:
   ```bash
   redis-cli ping
   ```

2. Verify channel subscription:
   ```bash
   redis-cli PUBSUB CHANNELS
   ```

3. Check PM2 Tasker logs:
   ```bash
   docker-compose logs pm2-tasker
   # or
   npm run pm2:logs
   ```

### Tasks not starting

1. Verify PM2 daemon is running:
   ```bash
   pm2 status
   ```

2. Check command syntax in message

3. Verify permissions for command execution

### Status updates not received

1. Subscribe to status channel:
   ```bash
   redis-cli SUBSCRIBE b00t:task:status
   ```

2. Check Redis publish permissions

## Future Enhancements

- [ ] Integration with b00t datum loader (read task config from TOML)
- [ ] Agent role-based access control (captains can manage all tasks)
- [ ] Task dependency management (DAG execution)
- [ ] Scheduled task execution (cron-like)
- [ ] Web UI for task monitoring
- [ ] Metrics export (Prometheus, OpenTelemetry)
- [ ] WASM-Go bindings (if required)
- [ ] Task result caching in Redis
- [ ] Distributed task execution across multiple PM2 Tasker instances

## References

- [k0mmand3r Parser](../k0mmand3r/README.md)
- [b00t-ipc](../b00t-ipc/README.md)
- [Redis Pub/Sub](https://redis.io/docs/manual/pubsub/)
- [PM2 Documentation](https://pm2.keymetrics.io/docs/)
- [b00t Datum System](../skills/datum-system/SKILL.md)
