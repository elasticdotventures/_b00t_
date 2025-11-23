# b00t-cli Container Build

Build b00t-cli and b00t-mcp containers from gospel source using Docker/Podman.

## Structure

```
docker.🐳/b00t/
├── build-from-gospel.sh    # DRY build script (mirrors GitHub Actions)
├── quick-build-aarch64.sh  # Fast native ARM64 build
├── BUILD-AARCH64.md        # aarch64-specific guide
├── env.sh                  # Docker wrapper (deprecated - use b00t-cli container)
└── README.md              # This file
```

Gospel Dockerfile (source of truth):
```
~/.b00t/Dockerfile.b00t-cli
```

## Quick Start - Unified Build (Recommended)

**One container with both b00t-cli and b00t-mcp:**

From the **host** (not from within Claude container):

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄

# Build unified container (CLI + MCP in one image)
./docker.🐳/b00t/build-unified.sh
```

This builds a single container with:
- `b00t-cli` - Command-line interface
- `b00t-mcp` - MCP server
- `b00t` - Alias to b00t-cli

### Verify Unified Build

```bash
# Test CLI
docker run --rm b00t:latest b00t-cli --version
docker run --rm b00t:latest b00t --version  # alias

# Test MCP
docker run --rm b00t:latest b00t-mcp --version

# Check architecture
docker run --rm b00t:latest uname -m  # Should show: aarch64
```

### Run from Unified Container

```bash
# CLI help
docker run --rm b00t:latest b00t-cli --help
docker run --rm b00t:latest b00t --help  # same

# MCP server
docker run --rm -it b00t:latest b00t-mcp

# With workspace
docker run --rm -v $PWD:$PWD -w $PWD b00t:latest b00t-cli status

# Interactive shell (both binaries available)
docker run --rm -it b00t:latest bash
```

## Alternative: Build CLI Only

If you only need b00t-cli (not recommended):

```bash
./docker.🐳/b00t/quick-build-aarch64.sh
```

## Gospel Convention

b00t follows a specific directory convention:

- **`~/.b00t`**: Hidden gospel (canonical source code)
  - Contains actual b00t source: b00t-cli, b00t-mcp, b00t-acp
  - Hidden from `ls` (chmod 700)
  - Only accessed if task-relevant

- **`~/_b00t_`**: Symlink to `~/.b00t/_b00t_/`
  - Visible agent workspace
  - Convention across all b00t deployments
  - Created automatically by env.sh

## Volume Mounts

| Host Path | Container Path | Purpose |
|-----------|---------------|---------|
| `$PWD` | `$PWD` | Working directory |
| `~/.b00t` | `/home/b00t/.b00t` | Gospel (hidden source) |
| `~/_b00t_` | `/home/b00t/_b00t_` | Agent workspace (visible) |

## Group-Based Skills

Agents are blessed with skills via Unix groups:

```bash
# Example: Agent with rust, node, docker skills
# Belongs to groups: b00t, b00t-rust, b00t-node, b00t-docker

groups alice
# alice b00t b00t-rust b00t-node b00t-docker
```

Skills are Docker-based capabilities:
- `rust` - Rust development
- `node` - Node.js/npm
- `python` - Python development
- `docker` - Container orchestration
- `mqtt` - Message broker access

## MQTT Coordination

Agents communicate via MQTT topics:

```
b00t.{agent-name}.{skill-list}/{message-type}

Examples:
b00t.alice.rust-node/status
b00t.alice.rust-node/propose
b00t.bob.python-docker/step
```

## Building from Gospel (All Platforms)

### DRY-Compliant Build

The comprehensive build script that mirrors GitHub Actions:

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄
./docker.🐳/b00t/build-from-gospel.sh
```

This script:
- Mirrors `~/.b00t/.github/workflows/b00t-cli-container.yml`
- Auto-detects docker or podman
- Uses gospel's `Dockerfile.b00t-cli` as source of truth
- Reads version from gospel `b00t-c0re-lib/Cargo.toml`
- Tests the built container
- Tags: `b00t-cli:latest`, `b00t-cli:v{version}`, `b00t-cli:{arch}`

### Architecture Support

The gospel Dockerfile is multi-arch compatible:

- **aarch64/arm64**: Native build (recommended for ARM64 hosts)
- **x86_64/amd64**: Native build (recommended for x86 hosts)
- **Cross-compile**: Use Docker buildx with `--platform` flag

No cross-compilation needed - builds natively on your architecture.

### Build Scripts

| Script | Purpose | When to Use |
|--------|---------|-------------|
| `build-unified.sh` | **Unified b00t container** (CLI + MCP) | **Recommended** - One container, both binaries |
| `quick-build-aarch64.sh` | Fast native ARM64 build (CLI only) | Legacy - builds only b00t-cli |
| `build-from-gospel.sh` | Full DRY-compliant build (CLI only) | Legacy - production b00t-cli only |

**Recommendation**: Use `build-unified.sh` - it builds both b00t-cli and b00t-mcp in a single container with convenient aliases.

## Environment Variables

```bash
# Custom image
export B00T_DOCKER_IMAGE="b00t:custom"

# Force platform
export B00T_DOCKER_PLATFORM="linux/amd64"

# Agent identity
export B00T_AGENT_NAME="alice"

# MQTT broker
export B00T_MQTT_URL="mqtt://localhost:1883"
```

## Integration with Other Services

### mosquitto (MQTT Broker)

```bash
# Start mosquitto
cd /home/brianh/homeassistant/_b00t_/node-ts.🦄
docker-compose up mosquitto

# b00t agents auto-connect to mqtt://localhost:1883
```

### Claude Code

```bash
# Claude can run b00t commands via mounted ~/.b00t
source claude.🐳/env.sh
claude
> Run b00t-cli to check agent status
```

## Architecture Reference

See [B00T-ARCHITECTURE.md](../../B00T-ARCHITECTURE.md) for complete system design including:
- Group-based skills system
- Gospel convention (~/.b00t vs ~/_b00t_)
- MQTT coordination protocol
- Agent memoization patterns
- Multi-agent workflows

## Build Performance

**First build** (~10-15 minutes on aarch64):
- Downloads Rust toolchain (~1.2GB)
- Compiles dependencies
- Builds b00t-cli binary

**Subsequent builds** (~2-3 minutes):
- Uses Docker layer cache
- Only recompiles changed code

## Troubleshooting

### Slow build

First build downloads full Rust toolchain. Check cache:
```bash
docker images | grep rust
```

### Out of memory

Increase Docker memory to 4GB minimum:
```bash
# Check current limit
docker info | grep -i memory

# For Docker Desktop: Settings → Resources → Memory
```

### Gospel not found

```bash
ls -la ~/.b00t
# If missing: Already cloned at ~/.b00t from dotfiles
```

### Build context too large

The gospel contains all workspace members (b00t-cli, b00t-mcp, b00t-c0re-lib, etc.). This is intentional and required by the Dockerfile.

## Next Steps

1. **Build unified container** (from host):
   ```bash
   cd ~/homeassistant/_b00t_/node-ts.🦄
   ./docker.🐳/b00t/build-unified.sh
   ```

2. **Verify build** (both CLI and MCP):
   ```bash
   docker run --rm b00t:latest b00t-cli --version
   docker run --rm b00t:latest b00t-mcp --version
   docker run --rm b00t:latest uname -m  # Should show: aarch64
   ```

3. **Test MCP server**:
   ```bash
   # Start MCP server
   docker run --rm -it b00t:latest b00t-mcp

   # Or use gospel justfile
   cd ~/.b00t
   just inspect-mcp
   ```

4. **Configure Claude** to use b00t-mcp:
   ```bash
   # Add to ~/.claude/config.json
   {
     "mcpServers": {
       "b00t": {
         "command": "docker",
         "args": ["run", "--rm", "-i", "b00t:latest", "b00t-mcp"],
         "env": {
           "B00T_MQTT_URL": "mqtt://localhost:1883"
         }
       }
     }
   }
   ```

5. **Test Claude → b00t-mcp bridge**:
   - Restart Claude
   - Verify b00t MCP tools are available
   - Test agent coordination via MQTT

## References

- Gospel Dockerfile: `~/.b00t/Dockerfile.b00t-cli`
- GitHub Workflow: `~/.b00t/.github/workflows/b00t-cli-container.yml`
- aarch64 Guide: `./BUILD-AARCH64.md`
- Architecture: `../../B00T-ARCHITECTURE.md`
- b00t Gospel: `~/.b00t/AGENTS.md`

---

**Status**: Ready to build unified container on aarch64
**Action**: Run `./docker.🐳/b00t/build-unified.sh` from host
**Recommendation**: Use unified build - one container, both binaries (CLI + MCP)
