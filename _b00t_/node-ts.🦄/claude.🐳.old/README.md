# Claude CLI Docker Wrapper

Docker-backed Claude CLI with persistent credentials, shared npm cache, and autonomous subagents.

## Features

- **Containerized Claude CLI**: Runs in node:20-bookworm, no host npm pollution
- **Persistent Credentials**: `~/.claude` credentials survive container restarts
- **Shared NPM Cache**: Reuses npm cache from `npm.🐳` setup
- **Persistent Workspace**: `~/.claude-tmp` for read/write temporary files
- **G0spell Access**: `~/.b00t` mounted (read-only) for b00t source of truth
- **Auto-Detection**: Supports both Docker and Podman runtimes
- **Docker-in-Docker**: Socket mounting for building b00t containers from within Claude
- **Autonomous Subagents**: 100+ production-ready subagents from [awesome-claude-code-subagents](https://github.com/VoltAgent/awesome-claude-code-subagents)
- **Working Directory Chroot**: Current directory mounted and used as workspace

## Quick Start

### Basic Claude (Node-only)

```bash
# Source the environment
source claude.🐳/env.sh

# Run Claude CLI
claude

# Check version
claudev
# or
claude --version
```

### Claude with Docker CLI (for building b00t containers)

To build b00t-cli and b00t-mcp from within Claude:

```bash
# Build the enhanced Claude image with Docker CLI
./claude.🐳/build-claude-with-docker.sh

# Use the enhanced image
export CLAUDE_DOCKER_IMAGE=claude-with-docker:latest
source claude.🐳/env.sh

# Now Claude can build containers
claude
# Inside Claude: I can run ./docker.🐳/b00t/build-from-g0spell.sh
```

## Auto-Detection: Docker vs Podman

The `env.sh` wrapper automatically detects your container runtime:

1. **Docker detection**: Checks for `docker` command and `/var/run/docker.sock`
2. **Podman fallback**: Checks for `podman` and socket locations:
   - `/run/podman/podman.sock`
   - `$XDG_RUNTIME_DIR/podman/podman.sock`
3. **Socket mounting**: Mounts detected socket to `/var/run/docker.sock` in container
4. **Runtime variable**: Sets `CONTAINER_RUNTIME` environment variable (docker/podman)

This allows Claude to build Docker containers using the same runtime as the host.

## Available Subagents

The setup includes 100+ specialized subagents across 10 categories:

### Core Development
- `api-designer` - REST and GraphQL API architect
- `backend-developer` - Server-side expert
- `frontend-developer` - UI/UX specialist
- `fullstack-developer` - End-to-end features
- `mobile-developer` - Cross-platform mobile
- And more...

### Language Specialists
- `typescript-pro`, `python-pro`, `golang-pro`
- `react-specialist`, `vue-expert`, `nextjs-developer`
- `rust-engineer`, `java-architect`
- And more...

### Infrastructure
- `devops-engineer`, `kubernetes-specialist`
- `cloud-architect`, `terraform-engineer`
- `sre-engineer`, `security-engineer`
- And more...

### Quality & Security
- `code-reviewer`, `security-auditor`
- `qa-expert`, `penetration-tester`
- `performance-engineer`, `debugger`
- And more...

### Data & AI
- `data-scientist`, `ml-engineer`
- `llm-architect`, `prompt-engineer`
- `data-engineer`, `mlops-engineer`
- And more...

See [awesome-claude-code-subagents](https://github.com/VoltAgent/awesome-claude-code-subagents) for the complete list.

## Using Subagents

### Automatic Delegation
Claude Code automatically invokes appropriate subagents:
```bash
claude
> Review this code for security issues
# Automatically invokes security-auditor or code-reviewer
```

### Explicit Invocation
```bash
claude
> Use the code-reviewer subagent to analyze my latest changes
> Ask the typescript-pro agent to refactor this function
> Have the devops-engineer set up a CI/CD pipeline
```

### Command Line Subagent Selection
```bash
# Use specific agent via CLI
claude --agents '{"reviewer": {"description": "Custom reviewer"}}'
```

## Volume Mounts

| Host Path | Container Path | Purpose |
|-----------|---------------|---------|
| `$PWD` | `$PWD` | Working directory (chroot) |
| `~/.claude` | `/home/node/.claude` | Credentials & settings |
| `~/.claude-tmp` | `/home/node/.tmp` | Persistent temp workspace |
| `~/.b00t` | `/home/node/.b00t` | G0spell (read-only) - b00t source of truth |
| `~/.npm` | `/home/node/.npm` | NPM cache (shared) |
| `~/.cache/node` | `/home/node/.cache` | Node cache |
| `claude.🐳/awesome-claude-code-subagents/categories` | `/home/node/.claude/agents` | Subagent definitions (read-only) |
| Socket (auto-detected) | `/var/run/docker.sock` | Container runtime (for building b00t containers) |

## Building b00t Containers from Claude

Once you've built and are running the `claude-with-docker` image, Claude can build b00t-cli and b00t-mcp containers:

```bash
# Inside Claude session:
./docker.🐳/b00t/build-from-g0spell.sh
```

This script:
- Auto-detects docker or podman runtime
- Reads version from g0spell `b00t-c0re-lib/Cargo.toml`
- Uses g0spell's `Dockerfile.b00t-cli` (source of truth)
- Mirrors GitHub Actions workflow (DRY principle)
- Builds multi-stage Rust container
- Tests the built container
- Tags as `b00t-cli:latest`, `b00t-cli:v{version}`, `b00t-cli:aarch64`

**G0spell Integration**: The build script uses the g0spell (`~/.b00t`) as the source of truth, ensuring builds match the CI/CD workflow defined in `.github/workflows/b00t-cli-container.yml`.

## Configuration

### Custom Docker Image
```bash
export CLAUDE_DOCKER_IMAGE="node:22-bookworm"
source claude.🐳/env.sh
```

### Platform Override
```bash
export CLAUDE_DOCKER_PLATFORM="linux/amd64"  # Force x86 on ARM
source claude.🐳/env.sh
```

## Advanced Usage

### Non-Interactive Mode (Pipes)
```bash
echo "Explain this function" | claude --print < script.js
```

### Resume Session
```bash
claude --resume
claude --continue  # Continue most recent
```

### Custom Model
```bash
claude --model opus
claude --model sonnet
```

### Permission Modes
```bash
claude --permission-mode acceptEdits
claude --permission-mode bypassPermissions  # Sandbox only
```

### Add Additional Directories
```bash
claude --add-dir /path/to/other/project
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│ Host: /home/brianh/homeassistant/_b00t_/node-ts │
├─────────────────────────────────────────────────┤
│ source claude.🐳/env.sh                          │
│   ├─ Creates ~/.claude, ~/.claude-tmp           │
│   ├─ Sets SUBAGENTS_DIR path                    │
│   └─ Defines _claude_docker() function          │
└─────────────────────────────────────────────────┘
                     │
                     ▼
┌─────────────────────────────────────────────────┐
│ Docker: node:20-bookworm                        │
├─────────────────────────────────────────────────┤
│ /home/node/.claude/     ← Credentials           │
│ /home/node/.tmp/        ← Workspace (rw)        │
│ /home/node/.claude/agents/ ← Subagents (ro)     │
│ $PWD                    ← Working dir           │
│                                                 │
│ npx @anthropic-ai/claude                        │
└─────────────────────────────────────────────────┘
```

## Sharing with npm.🐳

Both `npm.🐳` and `claude.🐳` share:
- Same Node image (`node:20-bookworm`)
- Same npm cache (`~/.npm`)
- Same node cache (`~/.cache/node`)
- Same UID/GID mapping (no root-owned files)

## Troubleshooting

### Socket permission denied

If you get permission errors accessing the container socket:

**For Docker:**
```bash
# Add your user to docker group
sudo usermod -aG docker $USER
newgrp docker

# Or check socket permissions
ls -la /var/run/docker.sock
```

**For Podman:**
```bash
# Start user podman socket
systemctl --user start podman.socket
systemctl --user enable podman.socket

# Check socket location
ls -la $XDG_RUNTIME_DIR/podman/podman.sock
ls -la /run/podman/podman.sock
```

### Container runtime not detected

Check that docker or podman is installed and accessible:
```bash
command -v docker || command -v podman
docker --version  # or podman --version
```

### Subagents not appearing
```bash
# Check if mounted correctly
docker run --rm -it \
  -v "$(pwd)/claude.🐳/awesome-claude-code-subagents/categories":/agents:ro \
  node:20-bookworm ls -la /agents
```

### Permission issues
Ensure `--user "$(id -u):$(id -g)"` is preserved in env.sh

### Credentials not persisting
Check that `~/.claude` exists and is writable:
```bash
ls -la ~/.claude
```

### Docker CLI not found inside Claude container

If you see "docker: command not found" inside Claude, you're using the basic node:20-bookworm image. Build and use the claude-with-docker image:

```bash
./claude.🐳/build-claude-with-docker.sh
export CLAUDE_DOCKER_IMAGE=claude-with-docker:latest
source claude.🐳/env.sh
```

## References

- [Claude Code Documentation](https://docs.claude.com/en/docs/claude-code/sub-agents)
- [awesome-claude-code-subagents](https://github.com/VoltAgent/awesome-claude-code-subagents)
- [VoltAgent Framework](https://github.com/voltagent/voltagent)
