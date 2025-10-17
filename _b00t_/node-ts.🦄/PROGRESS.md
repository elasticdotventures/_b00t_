# b00t Development Progress

## Session Summary: 2025-10-05

### Completed

1. **✅ Claude CLI Docker Environment (`claude.🐳/`)**
   - Docker-backed Claude CLI with persistent credentials
   - Mounted `~/.b00t` for b00t repository access
   - Mounted `~/.claude-tmp` for persistent workspace
   - 100+ autonomous subagents from awesome-claude-code-subagents
   - Pattern: Similar to npm.🐳

2. **✅ mosquitto MQTT Broker Setup**
   - Created `docker-compose.yml` with mosquitto service
   - Configured for port 1883 (MQTT) and 9001 (WebSocket)
   - Created `mosquitto/config/mosquitto.conf` with ACP-ready topics
   - Ready for agent coordination protocol

3. **✅ Docker Environment Structure (`docker.🐳/`)**
   - Created modular docker container pattern
   - Separated concerns: rust, b00t, (future: node, python, etc.)

4. **✅ Rust Build Environment (`docker.🐳/rust/`)**
   - Reusable Rust 1.75 builder image
   - Includes all build dependencies (libssl-dev, pkg-config, etc.)
   - Shared cargo cache for faster builds
   - env.sh wrapper for cargo, rustc commands

5. **✅ b00t Runtime Environment (`docker.🐳/b00t/`)**
   - Multi-stage Dockerfile using rust-builder base
   - Builds both b00t-cli and b00t-mcp
   - Slim debian:bookworm runtime with mosquitto-clients
   - env.sh wrapper following established pattern
   - Gospel convention: `~/.b00t` (hidden) + `~/_b00t_` symlink

6. **✅ Architecture Design (`B00T-ARCHITECTURE.md`)**
   - Group-based skills system (not per-agent UIDs)
   - Gospel convention (~/.b00t hidden, ~/_b00t_ visible symlink)
   - MQTT namespace: `b00t.{agent-name}.{skills}/{message-type}`
   - Docker-based skills (npm, rust, python)
   - Project .b00t/ standard (RFC 2119 MUST)
   - Agent memoization patterns

### Directory Structure

```
/home/brianh/homeassistant/_b00t_/node-ts.🦄/
├── docker-compose.yml           # mosquitto MQTT broker
├── mosquitto/
│   ├── config/mosquitto.conf
│   ├── data/
│   └── log/
├── npm.🐳/                      # Node.js Docker pattern
│   └── env.sh
├── claude.🐳/                    # Claude CLI Docker pattern
│   ├── env.sh
│   ├── README.md
│   └── awesome-claude-code-subagents/
├── docker.🐳/                    # Docker environments
│   ├── rust/
│   │   ├── Dockerfile           # Reusable rust builder
│   │   └── env.sh
│   └── b00t/
│       ├── Dockerfile           # b00t runtime (uses rust builder)
│       ├── env.sh
│       └── README.md
├── B00T-ARCHITECTURE.md         # Complete system design
└── PROGRESS.md                   # This file
```

### Gospel Convention

**~/.b00t** (Hidden Gospel):
- Canonical source code repository
- Contains: b00t-cli, b00t-mcp, b00t-acp, etc.
- Hidden from `ls` (chmod 700)
- Only accessed when task-relevant

**~/_b00t_** → **~/.b00t/_b00t_/** (Symlink):
- Visible agent workspace
- Convention across all b00t deployments
- Created automatically by docker.🐳/b00t/env.sh

### Key Architectural Decisions

1. **Group-Based Skills vs Per-Agent UIDs**
   - Agents blessed with skills via Unix groups
   - Single agent can have multiple skills (poly-disciplinary)
   - Example: `alice` in groups `b00t`, `b00t-rust`, `b00t-node`

2. **MQTT Namespace Change**
   - Old: `account.{username}/acp/...`
   - New: `b00t.{agent-name}.{skill-list}/{message-type}`
   - Example: `b00t.alice.rust-node/status`

3. **Docker Skill Execution**
   - Skills run via Docker (npm, rust, python, etc.)
   - Agents pin specific versions in config
   - System has one copy, agents specify version
   - Example: `b00t-cli exec rust cargo build`

4. **Modular Docker Structure**
   - `docker.🐳/rust/` - Reusable Rust builder
   - `docker.🐳/b00t/` - b00t runtime (uses rust builder)
   - Future: `docker.🐳/node/`, `docker.🐳/python/`, etc.

### Next Steps (Priority Order)

#### Phase 0: Build & Test (IMMEDIATE)
- [ ] Clone dotfiles to ~/.b00t
- [ ] Build rust-builder image
- [ ] Build b00t:aarch64 image
- [ ] Test b00t-cli compilation
- [ ] Test b00t-mcp compilation

#### Phase 1: MQTT Integration
- [ ] Replace async-nats with rumqttc in b00t-acp
- [ ] Update Cargo.toml dependencies
- [ ] Implement rumqttc transport layer
- [ ] Test with mosquitto
- [ ] Test step synchronization

#### Phase 2: Group-Based Skills
- [ ] Create b00t group system
- [ ] Create skill groups (b00t-rust, b00t-node, etc.)
- [ ] Implement `b00t agent create` command
- [ ] Implement `b00t agent whoami` command
- [ ] Test multi-skill agents

#### Phase 3: b00t ai Command
- [ ] Create b00t ai wrapper script
- [ ] Create b00t subagent definition
- [ ] Integrate with Claude Code
- [ ] Test agent coordination via MQTT
- [ ] Implement message checking loop

#### Phase 4: Agent Memoization
- [ ] Implement project .b00t/ directory standard
- [ ] Create agent state management
- [ ] Implement git workspace pattern
- [ ] Test multi-agent coordination
- [ ] Document coordination patterns

### References

- [B00T-ARCHITECTURE.md](./B00T-ARCHITECTURE.md) - Complete system design
- [docker.🐳/b00t/README.md](./docker.🐳/b00t/README.md) - b00t Docker setup
- [claude.🐳/README.md](./claude.🐳/README.md) - Claude CLI setup
- [README-hive-acp.md](/tmp/dotfiles-explore/README-hive-acp.md) - ACP protocol spec

### External Dependencies

- **elasticdotventures/dotfiles**: Source repository for b00t-cli, b00t-mcp
- **mosquitto**: MQTT broker for agent coordination
- **rumqttc**: Rust MQTT client (to replace async-nats)
- **Claude Code**: AI agent framework with subagents
- **awesome-claude-code-subagents**: 100+ production subagents

---

**Status**: Architecture designed, Docker environment ready
**Blocker**: Need to clone ~/.b00t and test compilation
**Next Action**: Clone dotfiles to ~/.b00t and build images
