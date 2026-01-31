# b00t virtfs Implementation Roadmap

## Phase 1: Foundation (Sprint 1-2)

### Milestone 1.1: FUSE Filesystem Skeleton
**Goal**: Basic FUSE mount with read-only directory listing

**Tasks:**
- [ ] Add `fuser = "0.15"` to Cargo.toml
- [ ] Create `b00t-cli/src/virtfs/mod.rs` module
- [ ] Implement basic FUSE operations:
  - [ ] `init()` - Initialize filesystem
  - [ ] `readdir()` - List directory contents
  - [ ] `getattr()` - Get file attributes
  - [ ] `lookup()` - Find file by name
- [ ] Create `b00t mount` CLI command
- [ ] Test: Mount at `/tmp/b00t-test` and `ls`

**Deliverable**: Can mount virtfs and see empty directories

### Milestone 1.2: Static Datum Browser
**Goal**: Read-only view of existing datums

**Tasks:**
- [ ] Implement `datums/` directory structure
- [ ] Group datums by type (cli/, mcp/, ai/, etc.)
- [ ] Symlink to actual TOML files
- [ ] Test: `cat ~/.claude/b00t/datums/cli/rust.toml`

**Deliverable**: Browse datums via filesystem

## Phase 2: Skill Generation (Sprint 3-4)

### Milestone 2.1: Template Engine
**Goal**: Convert datums to .md skills

**Tasks:**
- [ ] Add `tera = "1.19"` or `handlebars = "5.1"` for templates
- [ ] Create skill template: `templates/skill.md.tera`
- [ ] Implement `SkillGenerator` struct
- [ ] Parse datum TOML and extract:
  - [ ] `learn.topic` → description
  - [ ] `usage` → examples
  - [ ] `install` → setup
  - [ ] `env` → configuration
- [ ] Test: Generate rust.md from rust.cli.toml

**Deliverable**: Single skill generation working

### Milestone 2.2: Dynamic Read Operations
**Goal**: Generate skills on `read()` calls

**Tasks:**
- [ ] Implement `read()` FUSE operation
- [ ] Cache generated .md content (LRU cache)
- [ ] Return generated content as file data
- [ ] Handle errors → generate error.md
- [ ] Test: `cat ~/.claude/b00t/skills/kubernetes.md`

**Deliverable**: All 83 datums readable as skills

## Phase 3: Live Reload (Sprint 5)

### Milestone 3.1: File Watching
**Goal**: Regenerate skills when datums change

**Tasks:**
- [ ] Add `notify = "6.1"` for file watching
- [ ] Watch `~/.b00t/_b00t_/*.toml` for changes
- [ ] Invalidate cache on datum modification
- [ ] Regenerate affected skill
- [ ] Test: Edit datum, read skill, verify change

**Deliverable**: Skills auto-update on datum changes

## Phase 4: Agent Proxies (Sprint 6-7)

### Milestone 4.1: Agent Directory Structure
**Goal**: Executable proxies for sub-agents

**Tasks:**
- [ ] Create `agents/` directory in virtfs
- [ ] Map Task tool sub-agents to directories
- [ ] Generate proxy scripts:
  ```bash
  #!/bin/bash
  # Proxy to security-sentinel agent
  b00t-cli agent invoke --type security-sentinel "$@"
  ```
- [ ] Make proxies executable (mode 755)
- [ ] Test: `~/.claude/b00t/agents/security-sentinel/review.sh file.rs`

**Deliverable**: Can invoke agents via filesystem

### Milestone 4.2: MCP Integration
**Goal**: Connect to Claude Code Task tool

**Tasks:**
- [ ] Implement `b00t-cli agent invoke` command
- [ ] Communicate with Claude Code via:
  - [ ] MCP stdio (preferred)
  - [ ] Unix socket (fallback)
  - [ ] HTTP REST API (last resort)
- [ ] Parse agent response
- [ ] Return formatted output
- [ ] Test: Full roundtrip agent invocation

**Deliverable**: Agents callable from shell

## Phase 5: Claude Code Integration (Sprint 8)

### Milestone 5.1: Slash Command Generation
**Goal**: Auto-generate `.claude/commands/*.md` from skills

**Tasks:**
- [ ] Implement `b00t init --claude-code` command
- [ ] Run `claude-code-init-hook.sh` template
- [ ] Create symlinks: `.claude/commands/rust.md` → `~/.claude/b00t/skills/rust.md`
- [ ] Verify Claude Code discovers commands
- [ ] Test: Type `/rust` in Claude Code

**Deliverable**: Slash commands working

### Milestone 5.2: MCP Server Orchestration
**Goal**: Auto-install b00t-mcp and rust-cargo-docs-rag-mcp

**Tasks:**
- [ ] Detect Claude Code installation
- [ ] Call `claude mcp add-json` for b00t-mcp
- [ ] Pull Docker image: `ghcr.io/promptexecution/rust-cargo-docs-rag-mcp:latest`
- [ ] Add rust-cargo-docs MCP server
- [ ] Test: Verify both MCP servers loaded

**Deliverable**: Complete Claude Code integration

## Phase 6: Advanced Features (Sprint 9-10)

### Milestone 6.1: Semantic Search Interface
**Goal**: Named pipe for semantic queries

**Tasks:**
- [ ] Create `search/` directory
- [ ] Implement named pipe: `search/query`
- [ ] On write: trigger embed_anything search
- [ ] Write results to `search/results.json`
- [ ] Test: `echo "how to install k8s" > ~/.claude/b00t/search/query`

**Deliverable**: Semantic search via filesystem

### Milestone 6.2: Entanglement Graph
**Goal**: Visualize datum dependencies

**Tasks:**
- [ ] Parse datum `depends_on` and `entangled` fields
- [ ] Generate GraphViz DOT files
- [ ] Create `graph/` directory
- [ ] Generate per-datum graphs: `graph/rust.dot`
- [ ] Generate full graph: `graph/full.svg`
- [ ] Test: View graph in browser

**Deliverable**: Dependency visualization

## Phase 7: Production Hardening (Sprint 11)

### Milestone 7.1: Error Handling & Logging
**Tasks:**
- [ ] Add `tracing` crate for structured logging
- [ ] Handle all FUSE operation errors gracefully
- [ ] Log to `~/.b00t/virtfs.log`
- [ ] Add metrics: cache hit rate, skill generation time
- [ ] Test: Kill virtfs ungracefully, verify clean restart

### Milestone 7.2: Performance Optimization
**Tasks:**
- [ ] Profile with `cargo flamegraph`
- [ ] Optimize skill generation (parallel where possible)
- [ ] Tune LRU cache size based on memory
- [ ] Add concurrent read support
- [ ] Benchmark: 1000 skill reads/sec target

### Milestone 7.3: Cross-Platform Support
**Tasks:**
- [ ] Test on Linux (Ubuntu 22.04+)
- [ ] Test on macOS (14+ Sonoma)
- [ ] Test on WSL2
- [ ] Handle platform-specific FUSE quirks
- [ ] Document platform requirements

## Phase 8: TOGAF Compliance & Documentation (Sprint 12)

### Milestone 8.1: Architecture Documentation
**Tasks:**
- [ ] Complete TOGAF ADM alignment docs
- [ ] Document Nasdanika pattern implementations
- [ ] Create architecture diagrams (C4 model)
- [ ] Write governance playbook
- [ ] Review with architecture board

### Milestone 8.2: User Documentation
**Tasks:**
- [ ] Write installation guide
- [ ] Create usage examples
- [ ] Document troubleshooting
- [ ] Record demo video
- [ ] Publish to b00t docs site

## Success Criteria

### Phase 1-2 (Foundation + Skills)
- ✅ Mount virtfs successfully
- ✅ 83 datums browsable as skills
- ✅ Skills readable with correct content

### Phase 3-4 (Live + Agents)
- ✅ Skills auto-regenerate on datum change
- ✅ 15 sub-agents accessible via filesystem
- ✅ Agent proxies executable and functional

### Phase 5-6 (Integration + Advanced)
- ✅ Claude Code slash commands working
- ✅ MCP servers auto-installed
- ✅ Semantic search via filesystem

### Phase 7-8 (Production + Docs)
- ✅ No memory leaks or crashes
- ✅ <10ms skill read latency (p95)
- ✅ Cross-platform compatibility
- ✅ Complete architecture docs

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|-----------|
| FUSE performance issues | High | Aggressive caching, async I/O |
| Cross-platform FUSE differences | Medium | Abstract FUSE layer, platform tests |
| Claude Code API changes | Medium | Version pinning, fallback modes |
| Datum parsing errors | Low | Robust error handling, validation |
| Mount point conflicts | Low | Configurable paths, cleanup |

## Dependencies

- **Rust Crates**: fuser, tera/handlebars, notify, tracing, tokio
- **External**: Claude Code, Docker (for rust-cargo-docs-rag-mcp)
- **Internal**: b00t-cli, b00t-mcp, embed_anything integration

## Timeline

- **Sprint 1-2** (2 weeks): Foundation
- **Sprint 3-4** (2 weeks): Skill Generation
- **Sprint 5** (1 week): Live Reload
- **Sprint 6-7** (2 weeks): Agent Proxies
- **Sprint 8** (1 week): Claude Code Integration
- **Sprint 9-10** (2 weeks): Advanced Features
- **Sprint 11** (1 week): Production Hardening
- **Sprint 12** (1 week): Documentation

**Total**: ~12 weeks (3 months)

## Next Steps

1. Create feature branch: `git checkout -b feature/virtfs-implementation`
2. Start Phase 1, Milestone 1.1
3. Set up CI/CD for virtfs tests
4. Schedule weekly reviews
