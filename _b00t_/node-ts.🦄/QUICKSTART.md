# b00t Quick Start Guide

Get b00t up and running on aarch64 with unified container (CLI + MCP).

## Prerequisites

- ✅ Gospel at `~/.b00t` (from elasticdotventures/dotfiles)
- ✅ Docker or Podman installed
- ✅ mosquitto MQTT broker running (port 1883)
- ✅ At least 4GB RAM for Docker builds

## Step 1: Get b00t Container

You have 3 options:

### Option A: Pull from GHCR (Fastest - 30 seconds)

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄

# Pull pre-built b00t-cli from GitHub Actions
./docker.🐳/b00t/pull-from-ghcr.sh
```

**Note:** GHCR image contains **b00t-cli only** (from CI/CD). For b00t-mcp, use Option B.

### Option B: Build Unified Container (Recommended - 10-15 min)

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄

# Build both b00t-cli and b00t-mcp in one container
./docker.🐳/b00t/build-unified.sh
```

**Build time:** ~10-15 minutes (first build), ~2-3 minutes (subsequent)

### Option C: Build CLI Only (Legacy)

```bash
./docker.🐳/b00t/quick-build-aarch64.sh
```

**Recommendation:** Use Option B for complete setup (CLI + MCP)

**Output:**
```
Images tagged:
  - b00t:latest (unified)
  - b00t:v0.7.0
  - b00t:aarch64
  - b00t-cli:latest (alias)
  - b00t-mcp:latest (alias)
```

## Step 2: Verify Build

```bash
# Test CLI
docker run --rm b00t:latest b00t-cli --version

# Test MCP
docker run --rm b00t:latest b00t-mcp --version

# Test alias
docker run --rm b00t:latest b00t --version

# Check architecture
docker run --rm b00t:latest uname -m
# Should output: aarch64
```

## Step 3: Configure Claude MCP

Add b00t-mcp server to Claude's MCP configuration:

```bash
# Copy sample config
mkdir -p ~/.claude
cp .claude/mcp-config.sample.json ~/.claude/config.json

# Or manually add to ~/.claude/config.json:
```

```json
{
  "mcpServers": {
    "b00t": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "--network", "host",
        "-v", "$HOME/.b00t:/home/node/.b00t:ro",
        "-e", "B00T_MQTT_URL=mqtt://localhost:1883",
        "b00t:latest",
        "b00t-mcp"
      ]
    }
  }
}
```

## Step 4: Start mosquitto MQTT Broker

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄

# Start mosquitto
docker-compose up -d mosquitto

# Verify running
docker-compose ps mosquitto
```

## Step 5: Build Claude Gospel Agent (Optional)

Build Claude as a canonical b00t agent:

```bash
cd ~/.b00t/_b00t_/agents.🤓/claude
./build.sh
```

This creates `claude-b00t:latest` image with Docker CLI for building containers.

## Step 6: Test Claude → b00t-mcp Bridge

### Option A: From Gospel

```bash
cd ~/.b00t/_b00t_/agents.🤓/claude
source env.sh
claude
```

### Option B: From Project (Symlinked)

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄
source claude.🐳/env.sh  # Symlink to gospel
claude
```

Inside Claude, check for b00t MCP tools:
```
/mcp
```

You should see b00t server listed.

## Step 7: Test b00t-cli Commands

```bash
# Via container
docker run --rm b00t:latest b00t-cli status

# Via alias
docker run --rm b00t:latest b00t --help

# With workspace
docker run --rm -v $PWD:$PWD -w $PWD b00t:latest b00t-cli agent whoami
```

## Step 8: Test MQTT Coordination

### Start b00t-mcp server

```bash
docker run --rm -it \
  --network host \
  -v ~/.b00t:/home/node/.b00t:ro \
  -e B00T_MQTT_URL="mqtt://localhost:1883" \
  b00t:latest \
  b00t-mcp
```

### Monitor MQTT messages

```bash
# Subscribe to b00t topics
docker exec -it mosquitto mosquitto_sub -t "b00t/#" -v
```

### Send test message via Claude

Inside Claude with b00t-mcp:
```
Use b00t-mcp to send a test STATUS message to MQTT
```

## Troubleshooting

### Build fails with OOM

Increase Docker memory:
```bash
docker info | grep -i memory
# Should show at least 4GB
```

### mosquitto not running

```bash
docker-compose up -d mosquitto
docker-compose logs mosquitto
```

### MCP server not found

Verify config:
```bash
cat ~/.claude/config.json
```

Restart Claude after config changes.

### Docker socket permission denied

```bash
# Add user to docker group
sudo usermod -aG docker $USER
newgrp docker
```

### Gospel not found

```bash
ls -la ~/.b00t
# Should show dotfiles from elasticdotventures/dotfiles
```

## Directory Structure

```
~/.b00t/                                    # Gospel (source of truth)
├── _b00t_/
│   ├── agents.🤓/claude/                   # Claude canonical location
│   └── docker.🐳/Dockerfile.claude         # Claude agent build
├── Dockerfile.b00t-cli                     # b00t-cli build (gospel)
├── justfile                                # Build orchestration
└── .github/workflows/                      # CI/CD source of truth

~/homeassistant/_b00t_/node-ts.🦄/
├── claude.🐳 → ~/.b00t/_b00t_/agents.🤓/claude/  # Symlink
├── docker.🐳/b00t/
│   ├── Dockerfile.b00t                     # Unified container
│   ├── build-unified.sh                    # Build script
│   └── README.md
├── .claude/
│   └── mcp-config.sample.json              # Sample MCP config
└── docker-compose.yml                      # mosquitto broker
```

## What You Get

✅ **b00t:latest** - Unified container with:
  - b00t-cli - Command-line interface
  - b00t-mcp - MCP server
  - b00t - Alias to b00t-cli

✅ **Claude as gospel agent**:
  - Canonical location: `~/.b00t/_b00t_/agents.🤓/claude/`
  - Symlinked to projects (no duplication)
  - Gospel access (read-only mount)
  - Docker CLI for building containers

✅ **MQTT coordination**:
  - mosquitto broker on localhost:1883
  - b00t.{agent}.{skills}/{type} topic pattern
  - StepSync protocol (STATUS, PROPOSE, STEP, ACK)

✅ **Claude ↔ b00t-mcp bridge**:
  - MCP tools exposed to Claude
  - Agent coordination via MQTT
  - Extensible to subagents

## Next Steps

1. **Create agents**: `docker run --rm b00t:latest b00t-cli agent create alice --skills rust,node`
2. **Test coordination**: Multi-agent workflows via MQTT
3. **Replace NATS with MQTT**: Update b00t-lib-agent-coordination-protocol-rs
4. **Build subagents**: Specialized agents for different tasks

## References

- Architecture: `B00T-ARCHITECTURE.md`
- b00t Build Guide: `docker.🐳/b00t/README.md`
- aarch64 Guide: `docker.🐳/b00t/BUILD-AARCH64.md`
- Claude Agent: `~/.b00t/_b00t_/agents.🤓/claude/README.md`
- Gospel: `~/.b00t/AGENTS.md`

---

**Status**: Ready to build and test
**Time to first working setup**: ~15 minutes
