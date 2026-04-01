# b00t Virtual Filesystem (virtfs) Architecture

## Vision

A FUSE-based virtual filesystem that presents b00t datums as dynamically-generated Claude Code skills and sub-agent proxies, harmonized with TOGAF enterprise architecture using Nasdanika's declarative assembly patterns.

**Mount Point**: `~/.claude/b00t/` (virtual filesystem)

## TOGAF Alignment

### Architecture Development Method (ADM)

**Phase A - Architecture Vision**
- Datums define capabilities (business layer)
- Virtfs provides technical realization (application layer)
- FUSE implements infrastructure (technology layer)

**Phase B - Business Architecture**
- Datum types (CLI, MCP, AI, K8s) = business capabilities
- Entanglement graph = capability dependencies
- Usage examples = business processes

**Phase C - Information Systems Architecture**
- Application: b00t-cli + virtfs + MCP servers
- Data: Datum TOML files (source of truth)
- Integration: Claude Code slash commands

**Phase D - Technology Architecture**
- Infrastructure: FUSE filesystem, Rust runtime
- Platform: Linux/macOS/WSL
- Network: MCP protocol, local sockets

### Nasdanika Integration Patterns

**Declarative Assembly** (from Nasdanika MCP server):
- Individual capabilities (datums) assembled into filesystem
- Generators create skills from datum metadata
- Observability via filesystem stats

**Resource Provider Pattern**:
```rust
trait DatumResourceProvider {
    fn provide(&self, path: &str) -> Result<Resource>;
    fn list(&self) -> Vec<ResourceMetadata>;
    fn observe(&self) -> Observable<DatumChange>;
}
```

**Mount Point**: `~/.claude/b00t/` (virtual filesystem)

## Core Concept

```
~/.claude/b00t/               # FUSE mount point
├── skills/                    # Auto-generated .md files from datums
│   ├── rust.md               # Generated from rust.cli.toml
│   ├── kubernetes.md         # Generated from k8s.cli.toml
│   ├── embed-anything.md     # Generated from embed-anything.cli.toml
│   └── ...
├── agents/                    # Sub-agent proxies
│   ├── kieran-rails/         # Directory = agent namespace
│   │   ├── review.sh         # Executable that proxies to Task tool
│   │   └── config.json       # Agent configuration
│   ├── security-sentinel/
│   └── ...
└── datums/                    # Read-only datum browser
    ├── cli/                   # Grouped by type
    │   ├── rust.toml -> /home/user/.b00t/_b00t_/rust.cli.toml
    │   └── ...
    └── mcp/
        └── ...
```

## Implementation Strategy

### Phase 1: Basic FUSE Mount (fuser crate)

**Cargo.toml additions:**
```toml
[dependencies]
fuser = "0.15"
libc = "0.2"
```

**Module Structure:**
- `b00t-cli/src/virtfs/mod.rs` - Main FUSE filesystem
- `b00t-cli/src/virtfs/skill_generator.rs` - Datum → .md conversion
- `b00t-cli/src/virtfs/agent_proxy.rs` - Agent executable generation
- `b00t-cli/src/commands/mount.rs` - CLI interface

### Phase 2: Datum-to-Skill Dynamic Generation

**Algorithm:**
1. Watch datum TOML files for changes (via inotify/fsevents)
2. On change: regenerate corresponding skill .md file
3. Template system uses datum metadata:
   - `learn.topic` → Skill description
   - `usage` → Example commands
   - `install` → Setup instructions
   - `env` → Environment configuration

**Skill Template:**
```markdown
# {{ datum.name }}

{{ datum.hint }}

## Installation
\```bash
{{ datum.install }}
\```

## Usage
{% for example in datum.usage %}
### {{ example.description }}
\```bash
{{ example.command }}
\```
{% endfor %}

## Environment
{% for (key, value) in datum.env %}
- `{{ key }}`: {{ value }}
{% endfor %}

## Learn More
{{ datum.learn.content }}
```

### Phase 3: Agent Proxy Pattern

**Agent Directory Structure:**
```
~/.claude/b00t/agents/security-sentinel/
├── review.sh         # Executable proxy
├── audit.sh          # Another tool
└── config.json       # Agent metadata
```

**Proxy Script Generation:**
```bash
#!/bin/bash
# Auto-generated proxy for security-sentinel agent
# DO NOT EDIT - regenerated from b00t datums

AGENT_TYPE="security-sentinel"
TASK_DESCRIPTION="$*"

# Invoke via Claude Code Task tool (through MCP or local socket)
b00t-cli agent invoke \
    --type "$AGENT_TYPE" \
    --description "$TASK_DESCRIPTION" \
    --output json
```

### Phase 4: Claude Code Integration

**b00t init hook** (created during project initialization):

```bash
# .claude/hooks/post-init.sh
#!/bin/bash
# Mount b00t virtual filesystem

b00t-cli mount ~/.claude/b00t --daemon

echo "✅ b00t skills mounted at ~/.claude/b00t/"
echo "   Available skills: $(ls ~/.claude/b00t/skills/ | wc -l)"
echo "   Available agents: $(ls ~/.claude/b00t/agents/ | wc -l)"
```

**Auto-discovery:**
Claude Code scans `~/.claude/b00t/skills/*.md` and presents them as slash commands.

## DRY Benefits

1. **Single Source of Truth**: Datums define everything
2. **Auto-sync**: Skill files regenerate when datums change
3. **No Manual Updates**: Claude Code sees latest datum state
4. **Version Control**: Datums in git, skills are ephemeral

## FUSE Operations

| Operation | Implementation |
|-----------|----------------|
| `readdir` | List datums → generate file list |
| `read` | Generate .md content on-the-fly from datum |
| `getattr` | Return virtual file metadata (size, timestamps) |
| `lookup` | Map filename to datum |
| `statfs` | Report filesystem stats |

## Advanced Features (Future)

### Semantic Search Integration
```
~/.claude/b00t/search/
├── query               # Named pipe: echo "how to install k8s" > query
└── results.json        # Auto-updated search results
```

### Entanglement Graph
```
~/.claude/b00t/graph/
├── rust.dot            # GraphViz of rust dependencies
└── full.svg            # Complete datum graph
```

### Live Reload
Use inotify (Linux) / FSEvents (macOS) to detect datum changes and invalidate caches.

## CLI Interface

```bash
# Mount virtual filesystem
b00t mount [--mount-point ~/.claude/b00t] [--daemon]

# Unmount
b00t unmount [--mount-point ~/.claude/b00t]

# Status
b00t mount status

# Force regenerate all skills
b00t mount refresh

# Test without mounting (generate to disk)
b00t mount generate --output /tmp/b00t-skills
```

## Integration with MCP Servers

**b00t-mcp integration:**
- Add MCP tool: `read_skill(skill_name: str)` → reads from virtfs
- Add MCP tool: `list_skills()` → readdir on virtfs
- Add MCP tool: `invoke_agent(agent_type, prompt)` → proxy to Task tool

**rust-cargo-docs-rag-mcp integration:**
- Rust docs searchable via virtfs:
  ```
  ~/.claude/b00t/docs/rust/
  ├── std/               # Stdlib docs as .md
  ├── tokio/             # Crate docs
  └── search/            # Named pipe for queries
  ```

## Security Model

- **Read-only**: Skills are generated, not writable
- **Sandboxed**: FUSE runs in userspace, no kernel privileges needed
- **Validated**: Datum parsing validates TOML before generation

## Performance

- **Lazy Generation**: Skills generated on first read
- **Caching**: LRU cache of generated .md files
- **Parallel**: Multiple reads can occur concurrently (fuser is thread-safe)

## Error Handling

- Invalid datum TOML → Generate error.md skill with parse details
- Missing dependencies → Skill shows installation requirements
- Mount failures → Fallback to static file generation

## Testing Strategy

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_skill_generation() {
        let datum = load_datum("rust.cli.toml");
        let skill = generate_skill(&datum);
        assert!(skill.contains("# rust"));
    }

    #[test]
    fn test_fuse_readdir() {
        let fs = B00tFS::new(test_datums());
        let entries = fs.readdir("/skills");
        assert_eq!(entries.len(), 83); // Current datum count
    }
}
```

## Migration Path

1. Implement basic FUSE mount with static files
2. Add datum → skill generation
3. Integrate with Claude Code via hooks
4. Add agent proxies
5. Implement live reload
6. Add semantic search integration

## References

- [fuser crate](https://github.com/cberner/fuser)
- [FUSE protocol](https://www.kernel.org/doc/html/latest/filesystems/fuse.html)
- [Claude Code custom commands](https://code.claude.com/docs)
