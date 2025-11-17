# PM2 Tasker Quick Start

Get the PM2 Tasker running in 5 minutes.

## Prerequisites

- Docker and Docker Compose (easiest)
- OR: Node.js 18+, PM2, and Redis (manual install)

## Option 1: Docker Compose (Recommended)

```bash
# Clone/navigate to pm2-tasker directory
cd pm2-tasker

# Start Redis + PM2 Tasker
docker-compose up -d

# Check status
docker-compose ps
docker-compose logs -f pm2-tasker

# Test with a command
docker-compose exec redis redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"hello",
    "command":"echo",
    "args":"Hello from PM2!"
  }
}'

# Check PM2 processes (inside container)
docker-compose exec pm2-tasker pm2 list

# View task logs
docker-compose exec pm2-tasker pm2 logs hello

# Stop task
docker-compose exec redis redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"stop",
  "params":{"task":"hello"}
}'

# Cleanup
docker-compose down
```

## Option 2: Local Development

```bash
# 1. Start Redis (in separate terminal)
docker run -p 6379:6379 redis:7-alpine

# 2. Install PM2 globally
npm install -g pm2

# 3. Install dependencies
cd pm2-tasker
npm install

# 4. Build TypeScript
npm run build

# 5. Start PM2 Tasker
npm run pm2:start

# 6. Check logs
npm run pm2:logs

# 7. Test command (in separate terminal)
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"test",
    "command":"echo",
    "args":"Hello PM2 Tasker!"
  }
}'

# 8. Check PM2 processes
pm2 list

# 9. Stop PM2 Tasker
npm run pm2:stop
```

## Option 3: Using Justfile

```bash
# Install and build
cd pm2-tasker
just install
just build

# Start
just start

# View logs
just logs

# Test commands
just test-start-cmd
just list
just test-stop-cmd

# Monitor
just monit

# Stop
just stop
```

## Verifying It Works

### 1. Start a simple task

```bash
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"ping-test",
    "command":"ping",
    "args":"-c 5 8.8.8.8"
  }
}'
```

### 2. Check status

```bash
redis-cli PUBLISH b00t:k0mmand3r '{"verb":"status","params":{}}'
```

### 3. Watch status updates (in separate terminal)

```bash
redis-cli SUBSCRIBE b00t:task:status
```

### 4. View PM2 process list

```bash
pm2 list
# or
docker-compose exec pm2-tasker pm2 list
```

### 5. Stop the task

```bash
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"stop",
  "params":{"task":"ping-test"}
}'
```

## Common Tasks

### Start a web server

```bash
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"webserver",
    "command":"python3",
    "args":"-m http.server 8000",
    "env_PORT":"8000"
  }
}'
```

### Cluster mode (4 instances)

```bash
redis-cli PUBLISH b00t:k0mmand3r '{
  "verb":"start",
  "params":{
    "task":"api",
    "command":"node",
    "args":"api.js",
    "instances":"4",
    "max_memory_restart":"500M"
  }
}'
```

### Stop all tasks

```bash
pm2 stop all
# or
pm2 delete all
```

## Troubleshooting

### "Connection refused" error

Check Redis is running:
```bash
redis-cli ping
```

### "PM2 not found"

Install PM2:
```bash
npm install -g pm2
```

### Tasks not appearing

Check PM2 Tasker logs:
```bash
npm run pm2:logs
# or
docker-compose logs pm2-tasker
```

### Ports already in use

Change Redis port in docker-compose.yml:
```yaml
ports:
  - "6380:6379"  # Use 6380 instead
```

## Next Steps

- Read [INTEGRATION.md](./INTEGRATION.md) for advanced integration
- Read [README.md](./README.md) for full documentation
- Explore k0mmand3r parser at `../k0mmand3r/`
- Check out b00t-ipc for agent coordination at `../b00t-ipc/`

## Quick Commands Reference

| Command | Redis Message |
|---------|---------------|
| Start task | `{"verb":"start","params":{"task":"name","command":"cmd"}}` |
| Stop task | `{"verb":"stop","params":{"task":"name"}}` |
| Restart task | `{"verb":"restart","params":{"task":"name"}}` |
| Delete task | `{"verb":"delete","params":{"task":"name"}}` |
| Status | `{"verb":"status","params":{}}` |

## Support

- Issues: https://github.com/elasticdotventures/_b00t_/issues
- Docs: See README.md and INTEGRATION.md
- b00t Discord: [TBD]
