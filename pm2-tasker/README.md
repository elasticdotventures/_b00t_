# b00t PM2 Tasker

PM2-based task executor with k0mmand3r IPC integration for the b00t ecosystem.

## Overview

The PM2 Tasker listens to Redis pub/sub channels for slash commands parsed by k0mmand3r and executes task datums using PM2 process management.

## Architecture

```
┌─────────────────────────────────────────────────┐
│             b00t Container                       │
│                                                  │
│  ┌────────────────────────────────────────┐    │
│  │         PM2 Process Manager            │    │
│  │                                         │    │
│  │  ┌──────────────────────────────────┐  │    │
│  │  │  Tasker Runner (TypeScript)      │  │    │
│  │  │  - Reads datum configurations    │  │    │
│  │  │  - Executes tasks via PM2        │  │    │
│  │  │  - Listens to k0mmand3r IPC      │  │    │
│  │  └──────────────────────────────────┘  │    │
│  │                                         │    │
│  │  ┌─────────────────┐  ┌──────────────┐ │    │
│  │  │  /start task    │  │  /stop task  │ │    │
│  │  │  IPC Channel    │  │  IPC Channel │ │    │
│  │  └─────────────────┘  └──────────────┘ │    │
│  └────────────────────────────────────────┘    │
│                                                  │
│  ┌────────────────────────────────────────┐    │
│  │       k0mmand3r Parser                  │    │
│  │  - Parses /start, /stop commands        │    │
│  │  - Routes to Redis IPC channel          │    │
│  └────────────────────────────────────────┘    │
│                                                  │
│  ┌────────────────────────────────────────┐    │
│  │       Redis Pub/Sub                     │    │
│  │  - Channel: b00t:k0mmand3r              │    │
│  └────────────────────────────────────────┘    │
└─────────────────────────────────────────────────┘
```

## Features

- **Slash Command Integration**: Responds to k0mmand3r slash commands via Redis pub/sub
- **Process Management**: Leverages PM2 for robust process lifecycle management
- **Datum-based Execution**: Executes tasks defined in b00t datum TOML files
- **Graceful Shutdown**: Handles SIGINT/SIGTERM for clean shutdown
- **Status Broadcasting**: Publishes task status changes to Redis channels

## Supported Commands

### `/start` - Start a task

```bash
# Basic start
/start --task=mytask --command="node server.js"

# With environment variables
/start --task=api --command="python api.py" --env_PORT=8080 --env_HOST=0.0.0.0

# From datum
/start --task=worker --datum=myworker.cli

# With multiple instances (clustering)
/start --task=api --command="node api.js" --instances=4

# With memory limit
/start --task=heavy --command="node process.js" --max_memory_restart=1G
```

### `/stop` - Stop a running task

```bash
/stop --task=mytask
```

### `/restart` - Restart a task

```bash
/restart --task=mytask
```

### `/delete` - Remove a task from PM2

```bash
/delete --task=mytask
```

### `/status` - List all running tasks

```bash
/status
```

## Installation

### Prerequisites

- Node.js >= 18.0.0
- PM2 installed globally (`npm install -g pm2`)
- Redis server running

### Setup

```bash
# Install dependencies
npm install

# Build TypeScript
npm run build

# Start with PM2
npm run pm2:start

# Or run in development mode
npm run dev
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `REDIS_URL` | `redis://localhost:6379` | Redis connection URL |
| `PM2_TASKER_CHANNEL` | `b00t:k0mmand3r` | Redis channel for k0mmand3r commands |
| `LOG_LEVEL` | `info` | Logging level (debug, info, warn, error) |
| `NODE_ENV` | `development` | Environment (development, production, test) |

## Usage with k0mmand3r

The PM2 Tasker integrates with k0mmand3r by listening to the `b00t:k0mmand3r` Redis channel. Commands are parsed by k0mmand3r and published as JSON messages:

```typescript
{
  "verb": "start",
  "params": {
    "task": "myapp",
    "command": "node app.js",
    "env_PORT": "3000"
  },
  "content": "optional content",
  "timestamp": "2025-11-17T15:00:00Z",
  "agent_id": "agent-123"
}
```

## Testing

```bash
# Run tests (TBD)
npm test

# Test with Redis CLI
redis-cli PUBLISH b00t:k0mmand3r '{"verb":"start","params":{"task":"test","command":"echo hello"}}'
```

## Docker Integration

A Dockerfile is provided for containerized deployment:

```bash
# Build image
docker build -t b00t-pm2-tasker .

# Run container
docker run -d \
  --name pm2-tasker \
  -e REDIS_URL=redis://redis:6379 \
  --network b00t-network \
  b00t-pm2-tasker
```

## Logs

PM2 logs are stored in `~/.pm2/logs/`:

```bash
# View logs
npm run pm2:logs

# Or directly with PM2
pm2 logs b00t-tasker

# View all PM2 processes
pm2 list
```

## Troubleshooting

### PM2 not connecting

Ensure PM2 daemon is running:
```bash
pm2 ping
pm2 status
```

### Redis connection errors

Verify Redis is running and accessible:
```bash
redis-cli ping
```

Check `REDIS_URL` environment variable.

### Task not starting

Check PM2 logs for errors:
```bash
pm2 logs b00t-tasker --err
```

Verify the command/script path is correct.

## Development

```bash
# Watch mode with hot reload
npm run dev

# Lint code
npm run lint

# Stop PM2 process
npm run pm2:stop

# Delete PM2 process
npm run pm2:delete
```

## Architecture Notes

### DRY Philosophy

Following b00t's DRY (Don't Repeat Yourself) principle, the PM2 Tasker:
- Leverages existing PM2 ecosystem (no reinventing process management)
- Reuses k0mmand3r for command parsing
- Integrates with b00t-ipc for agent coordination
- Uses Redis pub/sub (already part of b00t stack)

### Future Enhancements

- [ ] WASM interface for Go integration (if needed)
- [ ] Datum loader from b00t-cli
- [ ] Integration with b00t session management
- [ ] Web UI for task monitoring
- [ ] Metrics collection and export
- [ ] Task scheduling (cron-like)
- [ ] Task dependency management (DAG execution)

## License

MIT

## References

- [PM2 Documentation](https://pm2.keymetrics.io/docs/usage/quick-start/)
- [k0mmand3r](../../k0mmand3r/README.md)
- [b00t-ipc](../../b00t-ipc/README.md)
- [Redis Pub/Sub](https://redis.io/docs/manual/pubsub/)
