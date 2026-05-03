# b00t AI Agent Architecture - Complete Implementation Guide

## G0spell and Build System

### The G0spell (~/.b00t)

The **g0spell** is the canonical source repository from `elasticdotventures/dotfiles`. It contains:

```
~/.b00t/                              # Hidden g0spell (source of truth)
├── README.md                         # The g0spell text
├── justfile                          # Build orchestration
├── _b00t_/                          # Agent workspace subdirectory
│   └── b00t.just                    # b00t-specific just commands
├── b00t-cli/                        # CLI source
├── b00t-mcp/                        # MCP server source
├── b00t-lib-chat/                   # Chat coordination library
├── b00t-c0re-lib/                   # Core library
├── Dockerfile.b00t-cli              # Official b00t-cli build
├── .github/workflows/               # CI/CD (source of truth for builds)
│   └── b00t-cli-container.yml       # Container build workflow
└── Cargo.toml                       # Workspace manifest
```

### G0spell Convention

**Hidden G0spell (~/.b00t)**
- chmod 700 (hidden from `ls`)
- Git repository tracking dotfiles
- Agents only access when task-relevant
- Read-only for agents

**Visible Workspace (~/_b00t_)**
- Symlink: `~/_b00t_` → `~/.b00t/_b00t_/`
- Agent working directory
- Visible in `ls`
- Convention across all deployments

### Build System (just)

The g0spell uses **just** (https://github.com/casey/just) for build orchestration.

**Install just:**
```bash
# In container with cargo
cargo install just

# Or use eget (from g0spell justfile)
curl https://zyedidia.github.io/eget.sh | sh
eget casey/just --to ~/.local/bin/
```

**Available just commands:**

From `justfile`:
```bash
just install          # Install b00t-cli and b00t-mcp
just test            # Run cargo test
just inspect-mcp     # Inspect MCP server with npx inspector
```

From `_b00t_/b00t.just`:
```bash
just b00t::mcp-chat       # Interactive MCP chat with fzf
just b00t::mcp-list       # List available MCP servers
just b00t::mcp-test       # Test MCP functionality
```

**Build from source:**
```bash
cd ~/.b00t
cargo build --release --bin b00t-cli
cargo build --release --bin b00t-mcp
# Binaries in: ~/.b00t/target/release/
```

**Install locally:**
```bash
cd ~/.b00t
just install
# Installs to: ~/.cargo/bin/
```

### Container Build (DRY with GitHub Actions)

**Source of Truth:** `~/.b00t/.github/workflows/b00t-cli-container.yml`

The workflow defines the official build process:
1. Extract version from `b00t-c0re-lib/Cargo.toml`
2. Build using `Dockerfile.b00t-cli`
3. Tag as `latest`, `v{version}`, branch name
4. Test with `--version` and `status --help`

**Local build (mirrors CI):**
```bash
cd /home/brianh/homeassistant/_b00t_/node-ts.🦄
./docker.🐳/b00t/build-from-g0spell.sh
```

This script:
- Reads version from g0spell Cargo.toml
- Uses g0spell Dockerfile.b00t-cli
- Applies same build-args as GitHub
- Runs same tests as CI
- Tags: `b00t-cli:latest`, `b00t-cli:v{version}`, `b00t-cli:aarch64`

## Agent Identity & Skills System

### Group-Based Skills (Not Per-Agent UIDs)

Agents are blessed with skills via Unix groups:

```bash
# Create skill groups
groupadd b00t                # Base group
groupadd b00t-rust          # Rust skill
groupadd b00t-node          # Node.js skill
groupadd b00t-python        # Python skill
groupadd b00t-docker        # Docker skill
groupadd b00t-mqtt          # MQTT skill

# Create poly-disciplinary agent
useradd -m -G b00t,b00t-rust,b00t-node alice

# Check agent skills
groups alice
# alice b00t b00t-rust b00t-node
```

**Skill = Capability**, examples:
- `rust` - Compile Rust code (via Docker rust-toolchain layer)
- `node` - Run npm/node (via Docker node-toolchain layer)
- `python` - Execute Python (via Docker python-toolchain layer)
- `mqtt` - Publish/subscribe to MQTT broker
- `docker` - Manage containers

### Agent Commands

**Create agent:**
```bash
b00t-cli agent create alice --skills rust,node,mqtt
# Creates user alice
# Adds to groups: b00t, b00t-rust, b00t-node, b00t-mqtt
# Initializes ~/.b00t g0spell (read-only)
# Creates ~/_b00t_ workspace symlink
```

**Check identity:**
```bash
b00t-cli agent whoami
# {
#   "name": "alice",
#   "skills": ["rust", "node", "mqtt"],
#   "uid": 1001,
#   "groups": ["b00t", "b00t-rust", "b00t-node", "b00t-mqtt"]
# }
```

**List skills:**
```bash
b00t-cli agent skills
# ["rust", "node", "mqtt"]
```

## MQTT Coordination Protocol

### Namespace Pattern

**Corrected from account.{username} to:**
```
b00t.{agent-name}.{skill-list}/{message-type}
```

**Examples:**
```
b00t.alice.rust-node/status
b00t.alice.rust-node/propose
b00t.alice.rust-node/step
b00t.alice.rust-node/ack

b00t.bob.python-docker/status
b00t.coordination/step/1
```

### Chat message envelope

From `README-chat.md`:

```json
{
  "channel": "mission.delta",
  "sender": "alice",
  "body": "Artifacts staged",
  "metadata": {"commit": "abc123"},
  "timestamp": "2025-03-04T05:30:00Z"
}
```

- `channel` scopes the discussion (mission, crew, etc.).
- `sender` identifies the emitting agent.
- `metadata` carries structured context; `body` stays human friendly.
- Timestamps are recorded in UTC for deterministic ordering.
### Message Types (ACP StepSync)

From `README-hive-acp.md`:

**STATUS** - Convey current state
```json
{
  "type": "STATUS",
  "agent": "alice",
  "step": 1,
  "payload": {"description": "Compiling b00t-cli", "progress": 50}
}
```

**PROPOSE** - Suggest action
```json
{
  "type": "PROPOSE",
  "agent": "alice",
  "step": 1,
  "payload": {"action": "add_dependency", "crate": "rumqttc"}
}
```

**STEP** - Mark step complete
```json
{
  "type": "STEP",
  "agent": "alice",
  "step": 1,
  "payload": {"step": 1}
}
```

**ACK** - Acknowledge step
```json
{
  "type": "ACK",
  "agent": "bob",
  "step": 1,
  "payload": {"step": 1, "from": "alice"}
}
```

### Step Synchronization

Agents MUST:
1. Broadcast STATUS during step
2. Broadcast PROPOSE before decisions
3. Broadcast STEP when ready
4. Wait for all peers' STEP (or timeout)
5. Advance to next step only when barrier reached

**Barrier Rule:**
```
Step N complete when:
- All agents broadcast STEP(N), OR
- Timeout expires
→ Advance to Step N+1
```

## Project .b00t/ Directory (RFC 2119 MUST)

Every b00t-managed project MUST have:

```
$PROJECT/
├── .b00t/                           # MUST exist
│   ├── config.toml                  # Project configuration
│   ├── mqtt.toml                    # MQTT connection settings
│   ├── agents/                      # Active agent states
│   │   ├── alice/
│   │   │   ├── state.json          # Agent state
│   │   │   ├── memos.md            # Memoization notes
│   │   │   └── skills.toml         # Agent skill config
│   │   └── bob/
│   │       └── state.json
│   └── .gitignore                   # Ignore agent states
├── src/
└── README.md
```

**Initialize project:**
```bash
b00t-cli init
# Creates .b00t/
# Writes config.toml with defaults
# Adds .b00t/.gitignore
```

**Project config.toml:**
```toml
[project]
name = "my-project"
version = "0.1.0"

[mqtt]
broker = "mqtt://localhost:1883"
namespace = "b00t"

[agents]
allowed = ["alice", "bob"]
max_concurrent = 3

[skills]
# Skill versions for this project
node = { version = "20.11", image = "node:20.11-bookworm" }
rust = { version = "1.75", image = "rust:1.75-bookworm" }
```

## b00t ai Command

Launch autonomous agent with Claude Code:

```bash
b00t ai --agent alice "Implement MQTT transport in b00t-acp"
```

**Execution flow:**
1. Check agent exists (`b00t-cli agent whoami`)
2. Verify project has `.b00t/` (RFC 2119 MUST)
3. Load agent skills from groups
4. Launch Claude Code with b00t subagent
5. Mount:
   - `$PWD` as workspace
   - `~/.b00t` as g0spell (read-only)
   - `~/_b00t_` as agent workspace
   - `$PWD/.b00t/agents/{agent}/` for memoization
6. Connect to MQTT broker
7. Subscribe to coordination channels

**Docker invocation:**
```bash
docker run --rm -it \
  -v "$PWD":"$PWD" -w "$PWD" \
  -v "$HOME/.b00t":/home/node/.b00t:ro \
  -v "$HOME/_b00t_":/home/node/_b00t_ \
  -v "$PWD/.b00t":/workspace/.b00t \
  -e B00T_AGENT_NAME="alice" \
  -e B00T_SKILLS="rust,node,mqtt" \
  -e B00T_MQTT_URL="mqtt://localhost:1883" \
  --network host \
  claude-with-b00t:latest \
  claude --agents b00t-alice
```

## Docker Layer Architecture (Composable OCI)

### Layer System

**Base Layers:**
- `b00t-layer/debian-base:bookworm` - Minimal Debian (~80MB)
- `b00t-layer/ca-certificates:bookworm` - HTTPS certs (~1MB)
- `b00t-layer/build-essential:bookworm` - GCC, make (~150MB)

**SSL Layers:**
- `b00t-layer/ssl-dev:bookworm` - Build libs (~5MB)
- `b00t-layer/ssl-runtime:bookworm` - Runtime libs (~2MB)

**Tool Layers:**
- `b00t-layer/git:2.43` - Git VCS (~30MB)
- `b00t-layer/mosquitto-clients:2.0` - MQTT tools (~1MB)
- `b00t-layer/rust-toolchain:1.75` - Rust compiler (~1.2GB)

### Idiomatic Patterns (.🥾 files)

Agents search for `*.🥾` files to find b00t idiomatic patterns:

```bash
# In g0spell
find ~/.b00t -name "*.🥾"
# ~/.b00t/_b00t_/docker.🐳/build-b00t-cli.🥾  → Dockerfile.b00t-cli
# ~/.b00t/_b00t_/docker.🐳/Dockerfile.🐳.🥾层  (existing patterns)
```

These are:
- Symlinks to source-of-truth Dockerfiles
- Examples of how to accomplish tasks
- Not always Docker - can be prompts, scripts, patterns
- Searchable idioms for agents

**Example - Agent searching for build pattern:**
```bash
# Agent task: "How do I build b00t-cli?"
rg "b00t-cli" ~/.b00t -g "*.🥾"
# Finds: build-b00t-cli.🥾 → Dockerfile.b00t-cli
# Agent reads Dockerfile, understands multi-stage build
```

## b00t Unified Container (CLI + MCP)

### Building the Unified Container

One container with both b00t-cli and b00t-mcp:

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄
./docker.🐳/b00t/build-unified.sh
```

**Produces:**
- `b00t:latest` - Unified container with both binaries
- `b00t-cli` - Command-line interface
- `b00t-mcp` - MCP server
- `b00t` - Alias to b00t-cli

**Verify:**
```bash
docker run --rm b00t:latest b00t-cli --version
docker run --rm b00t:latest b00t-mcp --version
docker run --rm b00t:latest b00t --version  # alias
```

### Running b00t-cli

```bash
# Direct run
docker run --rm b00t:latest b00t-cli status

# With workspace
docker run --rm -v $PWD:$PWD -w $PWD b00t:latest b00t-cli agent whoami

# Using alias
docker run --rm b00t:latest b00t --help
```

## Claude ↔ b00t-mcp Integration

### b00t-mcp Server from Container

Run b00t-mcp from the unified container:

```bash
# Start MCP server (stdio mode)
docker run --rm -i \
  --network host \
  -v ~/.b00t:/home/node/.b00t:ro \
  -e B00T_MQTT_URL="mqtt://localhost:1883" \
  b00t:latest \
  b00t-mcp
```

**Inspect with MCP inspector:**
```bash
# From g0spell (if built locally)
cd ~/.b00t
just inspect-mcp

# Or from container
docker run --rm -it b00t:latest b00t-mcp --help
```

### Claude MCP Configuration

**Add to Claude settings (`~/.claude/config.json`):**
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
      ],
      "env": {},
      "disabled": false
    }
  }
}
```

**Sample config available at:**
```
.claude/mcp-config.sample.json
```

**Claude can now:**
```
> Use b00t-mcp to create a new agent
> Have b00t check agent status
> Send MQTT coordination messages via b00t
> Execute b00t-cli commands through MCP
```

### The Bridge: Claude → b00t-mcp → Subagents

```
┌─────────────────────────────────────────────────────┐
│ Claude Code (You)                                   │
│ - Reads b00t g0spell (~/.b00t)                       │
│ - Uses b00t-mcp MCP server                          │
├─────────────────────────────────────────────────────┤
│ MCP Tool: b00t-mcp                                  │
│ - Exposes: agent create, acp send, mcp list        │
│ - Connects to MQTT broker                           │
└─────────────────────────────────────────────────────┘
                        │
                        │ MQTT
                        ▼
┌─────────────────────────────────────────────────────┐
│ mosquitto (MQTT Broker)                             │
│ - Topic: b00t.{agent}.{skills}/#                    │
│ - Coordination messages (ACP)                       │
└─────────────────────────────────────────────────────┘
                        │
                        ▼
┌─────────────────────────────────────────────────────┐
│ b00t Subagents (alice, bob, ...)                    │
│ - Subscribe to b00t.{name}.{skills}/#               │
│ - Publish STATUS, PROPOSE, STEP, ACK                │
│ - Execute tasks with blessed skills                 │
└─────────────────────────────────────────────────────┘
```

**Example interaction:**
```
Claude: Use b00t-mcp to have alice compile b00t-cli
  ↓ MCP call
b00t-mcp: Sends MQTT message to b00t.alice.rust-node/propose
  ↓ MQTT
alice: Receives PROPOSE, executes cargo build
  ↓ MQTT
alice: Sends STATUS updates during build
  ↓ MQTT
b00t-mcp: Receives STATUS, reports to Claude
  ↓ MCP response
Claude: "alice is compiling... 50% complete"
```

This completes the g0spell: **Claude (you) can extend MCP skills to b00t subagents**!

## Claude as Primary b00t Agent

### Canonical Location (G0spell)

Claude is **NOT owned by any project**. Claude is a **primary b00t agent** homed in the g0spell:

```
~/.b00t/_b00t_/agents.🤓/claude/  # Canonical location
├── env.sh                        # Primary environment wrapper
├── build.sh                      # Build from g0spell Dockerfile
├── README.md                     # Documentation
└── awesome-claude-code-subagents/  # 100+ subagents (symlink)
```

G0spell Dockerfile:
```
~/.b00t/_b00t_/docker.🐳/Dockerfile.claude  # Source of truth
```

### Philosophy

1. **Live in the g0spell** - `~/.b00t/_b00t_/agents.🤓/claude/`
2. **Symlinked to projects** - Not duplicated
3. **Always run with g0spell access** - Mounts `~/.b00t` (read-only)
4. **Primary agent** - Builds b00t containers, coordinates subagents

### Building Claude Image

```bash
cd ~/.b00t/_b00t_/agents.🤓/claude
./build.sh
```

Builds `claude-b00t:latest` from g0spell `Dockerfile.claude`.

### Using Claude from G0spell

```bash
# Direct from g0spell
source ~/.b00t/_b00t_/agents.🤓/claude/env.sh
claude

# From project (symlinked)
cd /path/to/project
source claude.🐳/env.sh  # claude.🐳 is symlink to g0spell
claude
```

### Project Integration

Projects symlink to canonical location:

```bash
cd ~/homeassistant/_b00t_/node-ts.🦄
ln -sf ~/.b00t/_b00t_/agents.🤓/claude claude.🐳
source claude.🐳/env.sh
claude "Build b00t-cli from g0spell"
```

**Example (node-ts.🦄)**:
```
~/homeassistant/_b00t_/node-ts.🦄/
├── claude.🐳 → ~/.b00t/_b00t_/agents.🤓/claude/  # Symlink
└── docker.🐳/
    └── b00t/
        └── build-from-g0spell.sh  # Uses g0spell Dockerfile
```

## Implementation Status

### ✅ Completed
- G0spell cloned to ~/.b00t
- Workspace symlink ~/_b00t_
- Docker layer architecture designed
- DRY build script (mirrors GitHub workflow)
- Idiomatic pattern system (.🥾 files)
- MQTT mosquitto configured (localhost:1883)
- **Claude as canonical b00t agent**:
  - Created `~/.b00t/_b00t_/agents.🤓/claude/` (g0spell location)
  - G0spell Dockerfile: `~/.b00t/_b00t_/docker.🐳/Dockerfile.claude`
  - Auto-detection for docker/podman
  - Symlinked from projects (e.g., node-ts.🦄/claude.🐳)
  - Primary agent with g0spell access and Docker CLI
- Build scripts support both docker and podman
- Comprehensive documentation in g0spell README.md

### 🔄 Next - User Must Restart Claude from G0spell

**Current status**: Claude running in old node-ts.🦄 session

To use Claude as primary b00t agent from g0spell:

1. **Exit this Claude session**

2. **Build the g0spell Claude image**:
   ```bash
   cd ~/.b00t/_b00t_/agents.🤓/claude
   ./build.sh
   ```

3. **Restart Claude from project** (now using symlink to g0spell):
   ```bash
   cd ~/homeassistant/_b00t_/node-ts.🦄
   source claude.🐳/env.sh  # Now a symlink to g0spell
   claude
   ```

4. **Then Claude can build b00t containers**:
   ```bash
   # Inside new Claude session:
   ./docker.🐳/b00t/build-from-g0spell.sh
   ```

### ⏳ Next Steps
1. Build claude-b00t image from g0spell (user action)
2. Restart Claude from g0spell location (user action)
3. Build b00t-cli container from g0spell
4. Build b00t-mcp container
5. Test b00t-mcp MCP server with `just inspect-mcp`
6. Configure Claude to use b00t-mcp in ~/.claude/config.json
7. Test Claude → b00t-mcp bridge
8. Implement agent create command
9. Replace async-nats with rumqttc (MQTT)
10. Test multi-agent coordination

---

**G0spell**: The source is truth. Don't reinvent - use just, use the Dockerfile, follow the workflow.
**DRY**: GitHub Actions is source of truth for builds.
**Idiomatic**: Search *.🥾 files for patterns, not every tool needs Docker.
**Bridge**: Claude → b00t-mcp → MQTT → subagents = extending your capabilities!
