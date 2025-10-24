# Datum-Based Dependency Resolver & Software Stack System

## Executive Summary

This plan outlines a system to transform b00t's datum architecture from individual package management into a dependency-aware software stack installer that can:
1. Analyze MCP server requirements (docker, node, python, etc.)
2. Auto-install missing dependencies from the datum registry
3. Group related packages into "stacks" (recipes) for one-command deployment
4. Validate installations against a whitelist for LLM-driven operations

---

## Current State Analysis

### Datum Architecture (lib.rs:40-90)

**BootDatum Structure** contains fields for multiple installation types:
- **CLI tools**: `install`, `update`, `version`, `version_regex`
- **MCP servers**: `command`, `args`, `mcp` (multi-method)
- **Docker**: `image`, `docker_args`, `oci_uri`, `resource_path`
- **Kubernetes**: `chart_path`, `namespace`, `values_file`
- **VSCode**: `vsix_id`
- **Bash**: `script`
- **Constraints**: `require` (array), `env` (hashmap)

### Existing Datum Types (lib.rs:101-113)

1. **Cli** - Traditional CLI tools (rustc, node, gh)
2. **Mcp** - MCP servers with multi-method support (stdio, httpstream)
3. **Docker** - Containers with image drift detection
4. **K8s** - Helm charts and k8s resources
5. **Apt** - Linux package manager integration
6. **Bash** - Shell scripts
7. **Vscode** - VSCode extensions
8. **Ai** - AI provider configurations
9. **Nix** - Declarative package management (planned)

### Constraint System (traits.rs:82-102)

**Already implemented constraint evaluation**:
- `NEEDS_ANY_ENV` - At least one env var present
- `NEEDS_ALL_ENV` - All env vars present
- `OS:ubuntu/debian/macos/windows/linux` - OS requirements
- `CMD:command` - Command availability check

**Example**: browser-use.mcp.toml:
```toml
requires = ["OPENAI_API_KEY"]
```

### Registry Integration (b00t-c0re-lib/src/mcp_registry.rs)

**Dependency tracking structures exist**:
```rust
pub struct Dependency {
    pub dep_type: DependencyType,
    pub min_version: Option<String>,
    pub installed: bool,
    pub install_method: Option<String>,
}

pub enum DependencyType {
    Docker, Node, Npm, Python, Pip, Rust, System(String)
}
```

**Installation methods** already call `b00t-cli cli install <package>`:
```rust
async fn install_docker(&self) -> Result<()> {
    tokio::process::Command::new("b00t-cli")
        .args(&["cli", "install", "docker"])
        .output().await?;
}
```

---

## Problem Statement

### Current Gaps

1. **No automatic dependency resolution**: Installing `browser-use.mcp.toml` doesn't auto-install Python/uvx
2. **No stack/recipe system**: Can't define "postgres-full-stack" that installs postgres + pgadmin + monitoring
3. **No dependency graph**: MCP servers don't declare datum dependencies (only string constraints)
4. **Manual whitelist validation**: LLMs can't safely determine which packages are approved
5. **No auto-wiring**: Installed MCP servers aren't automatically connected to apps

### User Stories

**Story 1: Developer installs MCP server**
```bash
# User wants browser automation
b00t-cli mcp install browser-use claudecode

# Expected: Should detect missing uvx, install it, then install MCP server
# Current: Fails with "uvx not found"
```

**Story 2: DevOps deploys database stack**
```bash
# User wants full database environment
b00t-cli stack install postgres-dev

# Expected: Installs postgres, pgadmin, redis, monitoring, creates compose file
# Current: No concept of stacks exists
```

**Story 3: LLM agent installs from whitelist**
```bash
# LLM sees user needs filesystem access
LLM: "I notice you need file operations. Installing filesystem MCP server..."
b00t-cli registry install-deps io.b00t/filesystem --validate-whitelist

# Expected: Checks whitelist, installs npx if needed, installs MCP
# Current: No whitelist validation system
```

---

## Proposed Architecture

### 1. Enhanced Datum Structure

**Add dependency references to BootDatum** (lib.rs):

```toml
# browser-use.mcp.toml
[b00t]
name = "browser-use"
type = "mcp"
hint = "Browser automation MCP server"

# NEW: Datum-based dependencies (not just string constraints)
depends_on = [
    "python.cli",      # Requires Python CLI datum
    "uvx.cli",         # Requires uvx CLI datum
]

# Existing constraint system (for env vars, OS, etc.)
require = ["OPENAI_API_KEY"]

[[b00t.mcp.stdio]]
command = "uvx"
args = ["browser-use[cli]", "--mcp"]
```

**Benefits**:
- `depends_on` references actual datum IDs (file basename without extension)
- Can auto-install from local datum files
- Builds a proper dependency graph
- Backward compatible with existing `require` constraints

### 2. Software Stack/Recipe System

**New datum type: DatumType::Stack**

```toml
# postgres-dev-stack.stack.toml
[b00t]
name = "postgres-dev-stack"
type = "stack"
hint = "Full PostgreSQL development environment with monitoring"
keywords = ["database", "postgres", "dev", "monitoring"]

# Stack members - datums to install
members = [
    "postgres-enhanced.docker",   # Main database
    "pgadmin.docker",              # Web UI
    "redis.docker",                # Cache layer
    "prometheus.docker",           # Monitoring
]

# Stack-wide configuration
[b00t.stack]
compose_path = "stacks/postgres-dev/docker-compose.yml"
namespace = "postgres-dev"  # For k8s stacks

# Stack-wide environment
[b00t.env]
STACK_NETWORK = "postgres-dev-net"
POSTGRES_PASSWORD = "${POSTGRES_PASSWORD}"
```

**Implementation**:
```rust
// lib.rs
pub enum DatumType {
    Stack,  // NEW
    Mcp,
    Docker,
    // ... existing types
}

// datum_stack.rs
pub struct StackDatum {
    pub datum: BootDatum,
    pub members: Vec<String>,  // Datum IDs to install
}

impl StackDatum {
    pub async fn install(&self, path: &str) -> Result<()> {
        // 1. Resolve all member datums
        // 2. Build dependency graph
        // 3. Install in topological order
        // 4. Generate docker-compose.yml or k8s manifests
        // 5. Wire services together
    }
}
```

### 3. Dependency Resolution Algorithm

**New module: b00t-cli/src/dependency_resolver.rs**

```rust
pub struct DependencyResolver {
    datum_registry: HashMap<String, BootDatum>,
    installed_cache: HashSet<String>,
}

impl DependencyResolver {
    pub fn resolve(&self, datum_id: &str) -> Result<Vec<String>> {
        // 1. Load datum from file
        // 2. Extract depends_on field
        // 3. Recursively resolve dependencies
        // 4. Return topological sort (install order)
        // 5. Detect circular dependencies
    }

    pub async fn install_with_deps(&mut self, datum_id: &str) -> Result<()> {
        let install_order = self.resolve(datum_id)?;

        for dep_id in install_order {
            if self.is_installed(&dep_id) {
                println!("✓ {} already installed", dep_id);
                continue;
            }

            self.install_single_datum(&dep_id).await?;
            self.installed_cache.insert(dep_id);
        }

        Ok(())
    }

    fn install_single_datum(&self, datum_id: &str) -> Result<()> {
        // Parse datum type from ID: "python.cli" -> DatumType::Cli
        // Call appropriate installer: cli_install, docker_run, etc.
    }
}
```

**Topological sort with cycle detection**:
```rust
fn topological_sort(&self, datum_id: &str) -> Result<Vec<String>> {
    let mut visited = HashSet::new();
    let mut stack = Vec::new();
    let mut recursion_stack = HashSet::new();  // Cycle detection

    self.visit(datum_id, &mut visited, &mut stack, &mut recursion_stack)?;

    stack.reverse();
    Ok(stack)
}

fn visit(
    &self,
    node: &str,
    visited: &mut HashSet<String>,
    stack: &mut Vec<String>,
    recursion_stack: &mut HashSet<String>,
) -> Result<()> {
    if recursion_stack.contains(node) {
        anyhow::bail!("Circular dependency detected: {}", node);
    }

    if visited.contains(node) {
        return Ok(());
    }

    recursion_stack.insert(node.to_string());
    visited.insert(node.to_string());

    // Visit dependencies first
    if let Some(datum) = self.datum_registry.get(node) {
        if let Some(deps) = &datum.depends_on {
            for dep in deps {
                self.visit(dep, visited, stack, recursion_stack)?;
            }
        }
    }

    recursion_stack.remove(node);
    stack.push(node.to_string());

    Ok(())
}
```

### 4. MCP Whitelist Validation System

**New module: b00t-c0re-lib/src/mcp_whitelist.rs**

```rust
#[derive(Serialize, Deserialize)]
pub struct McpWhitelist {
    /// Approved MCP servers by registry ID
    approved_servers: HashSet<String>,

    /// Approved datum IDs for dependency installation
    approved_dependencies: HashSet<String>,

    /// Approved package managers
    approved_package_managers: HashSet<String>,

    /// Approved docker images (by OCI URI pattern)
    approved_docker_patterns: Vec<Regex>,
}

impl McpWhitelist {
    pub fn validate_mcp_server(&self, server_id: &str) -> Result<()> {
        if !self.approved_servers.contains(server_id) {
            anyhow::bail!(
                "MCP server '{}' not in whitelist. Add to ~/.b00t/mcp_whitelist.json",
                server_id
            );
        }
        Ok(())
    }

    pub fn validate_dependency(&self, datum_id: &str) -> Result<()> {
        if !self.approved_dependencies.contains(datum_id) {
            anyhow::bail!(
                "Dependency '{}' not in whitelist",
                datum_id
            );
        }
        Ok(())
    }

    pub fn can_auto_install(&self, datum_id: &str) -> bool {
        self.approved_dependencies.contains(datum_id)
    }
}
```

**Example whitelist file** (`~/.b00t/mcp_whitelist.json`):
```json
{
  "approved_servers": [
    "io.b00t/filesystem",
    "io.b00t/github",
    "io.b00t/browser-use",
    "modelcontextprotocol.io/server-filesystem",
    "modelcontextprotocol.io/server-github"
  ],
  "approved_dependencies": [
    "node.cli",
    "python.cli",
    "rustc.cli",
    "docker.cli",
    "uvx.cli",
    "npx.cli"
  ],
  "approved_package_managers": [
    "npx",
    "uvx",
    "docker"
  ],
  "approved_docker_patterns": [
    "^docker\\.io/library/.*",
    "^postgres:.*",
    "^redis:.*"
  ]
}
```

### 5. Auto-Wiring System

**New module: b00t-cli/src/auto_wire.rs**

```rust
pub struct AutoWire {
    installed_servers: Vec<McpServerRegistration>,
}

impl AutoWire {
    pub async fn wire_to_apps(&self) -> Result<()> {
        // Discover which AI coding apps are installed
        let installed_apps = self.discover_apps();

        for app in installed_apps {
            match app.as_str() {
                "claudecode" => self.wire_to_claude_code().await?,
                "vscode" => self.wire_to_vscode().await?,
                "cursor" => self.wire_to_cursor().await?,
                _ => {}
            }
        }

        Ok(())
    }

    fn discover_apps(&self) -> Vec<String> {
        let mut apps = Vec::new();

        if check_command_available("claude") {
            apps.push("claudecode".to_string());
        }
        if check_command_available("code") {
            apps.push("vscode".to_string());
        }
        // ... more apps

        apps
    }

    async fn wire_to_claude_code(&self) -> Result<()> {
        for server in &self.installed_servers {
            // Check if already installed in Claude Code
            if self.is_already_wired("claudecode", &server.id) {
                continue;
            }

            // Install to Claude Code
            claude_code_install_mcp(&server.name, "~/.dotfiles/_b00t_")?;
            println!("🔗 Wired {} to Claude Code", server.name);
        }
        Ok(())
    }
}
```

---

## Implementation Phases

### Phase 1: Enhanced Datum Dependencies (Week 1)

**Tasks**:
1. Add `depends_on: Vec<String>` field to BootDatum (lib.rs:40)
2. Create `dependency_resolver.rs` with topological sort
3. Update datum TOML files to include `depends_on`
4. Write unit tests for circular dependency detection

**CLI Changes**:
```bash
# New flag for recursive dependency installation
b00t-cli mcp install browser-use claudecode --install-deps

# Shows dependency tree before installation
b00t-cli mcp deps browser-use
# Output:
# browser-use.mcp
#   └─ python.cli
#      └─ (system package)
#   └─ uvx.cli
#      └─ python.cli
```

**Deliverable**: MCP servers can declare datum dependencies and auto-install them

### Phase 2: Stack/Recipe System (Week 2)

**Tasks**:
1. Create `DatumType::Stack` enum variant
2. Implement `datum_stack.rs` module
3. Create `StackDatum` with member resolution
4. Add `b00t-cli stack install <name>` command
5. Generate docker-compose.yml from stack members

**CLI Changes**:
```bash
# Install entire stack
b00t-cli stack install postgres-dev-stack

# List available stacks
b00t-cli stack list

# Show stack members and dependencies
b00t-cli stack show postgres-dev-stack
```

**Example stack behavior**:
```bash
$ b00t-cli stack install postgres-dev-stack
📦 Resolving stack: postgres-dev-stack
  └─ postgres-enhanced.docker
  └─ pgadmin.docker
  └─ redis.docker
  └─ prometheus.docker

🔍 Checking dependencies...
  ✓ docker.cli (installed)
  ⏳ docker-compose.cli (not installed, installing...)
  ✅ docker-compose installed successfully

📥 Installing stack members...
  ✅ postgres-enhanced.docker
  ✅ pgadmin.docker
  ✅ redis.docker
  ✅ prometheus.docker

📝 Generating docker-compose.yml...
  📁 Created: stacks/postgres-dev/docker-compose.yml

🚀 Starting stack...
  docker-compose -f stacks/postgres-dev/docker-compose.yml up -d

✅ Stack installed successfully!
```

**Deliverable**: Can define and install multi-component software stacks

### Phase 2.5: Registry-as-View Integration (COMPLETED)

**Tasks**:
1. ✅ Create `sync_from_datums()` method in McpRegistry
2. ✅ Add `sync_datum_file()` to convert TOML to registry entries
3. ✅ Implement `convert_datum_deps_to_registry_deps()` mapping
4. ✅ Add `mcp registry sync-datums` CLI command
5. ✅ Test end-to-end flow: register → sync → query

**Implementation**:
- Registry now acts as a "view" over datum TOML files
- `mcp_registry.rs:428-605` - sync methods with server_id generation
- `commands/mcp.rs:120-124` - SyncDatums CLI command
- Datum TOML files remain single source of truth
- Registry populated via sync for runtime metadata and health checks

**Usage**:
```bash
# Sync registry from datum files
b00t-cli mcp registry sync-datums --path ~/.dotfiles/_b00t_

# Register new MCP (creates TOML + syncs to registry)
b00t-cli mcp register test-server -- uvx test-package

# Query from registry
b00t-cli mcp registry get local.b00t/test-server
b00t-cli mcp registry list
```

**Deliverable**: Registry and datum systems integrated with datums as source of truth

### Phase 3: Whitelist Validation (Week 3)

**Tasks**:
1. Create `mcp_whitelist.rs` in b00t-c0re-lib
2. Implement JSON-based whitelist loading from `~/.b00t/mcp_whitelist.json`
3. Add validation checks to registry install operations
4. Create default whitelist with official MCP servers
5. Add `--validate` flag to CLI commands

**CLI Changes**:
```bash
# Validate before installation
b00t-cli mcp install browser-use --validate-whitelist

# Add server to whitelist
b00t-cli whitelist add io.b00t/browser-use

# Show whitelist
b00t-cli whitelist list

# LLM-safe installation (requires whitelist)
b00t-cli registry install-deps io.b00t/filesystem --llm-safe
```

**Deliverable**: LLMs can safely install only approved packages

### Phase 4: Auto-Wiring (Week 4)

**Tasks**:
1. Create `auto_wire.rs` module
2. Implement app discovery (claudecode, vscode, cursor)
3. Add `--auto-wire` flag to install commands
4. Detect already-wired servers to avoid duplicates
5. Support `.mcp.json`, Claude config, VSCode config

**CLI Changes**:
```bash
# Wire all installed MCP servers to detected apps
b00t-cli mcp wire --all

# Wire specific server to specific app
b00t-cli mcp wire browser-use --to claudecode

# Show wiring status
b00t-cli mcp wiring-status
```

**Deliverable**: Installed MCP servers automatically appear in AI coding apps

### Phase 5: LLM Integration Testing (Week 5)

**Tasks**:
1. Create test scenarios for LLM-driven installation
2. Implement safety guardrails (budget limits, confirmation prompts)
3. Add MCP tool for LLM agents: `install_from_registry_with_deps`
4. Write integration tests with mock LLM calls
5. Documentation for LLM usage patterns

**New MCP Tool** (b00t-mcp/src/mcp_registry_tools.rs):
```rust
pub async fn llm_safe_install(params: LlmSafeInstallParams) -> Result<String> {
    // 1. Validate against whitelist
    let whitelist = McpWhitelist::load()?;
    whitelist.validate_mcp_server(&params.server_id)?;

    // 2. Show dependency tree and ask for confirmation
    let resolver = DependencyResolver::new();
    let deps = resolver.resolve(&params.server_id)?;

    // 3. Check budget/cost estimates
    // 4. Install with full dependency resolution
    // 5. Auto-wire to detected apps
}
```

**Deliverable**: End-to-end LLM-driven installation flows work safely

---

## File Structure

```
b00t-cli/
├── src/
│   ├── dependency_resolver.rs       # NEW: Topological sort, graph resolution
│   ├── datum_stack.rs               # NEW: Stack datum implementation
│   ├── auto_wire.rs                 # NEW: Auto-wiring to apps
│   ├── commands/
│   │   ├── stack.rs                 # NEW: Stack CLI commands
│   │   ├── whitelist.rs             # NEW: Whitelist management
│   │   └── mcp.rs                   # MODIFY: Add --install-deps, --validate flags
│   ├── datum_cli.rs                 # MODIFY: Support depends_on
│   ├── datum_mcp.rs                 # MODIFY: Support depends_on
│   ├── datum_docker.rs              # MODIFY: Support depends_on
│   └── lib.rs                       # MODIFY: Add depends_on to BootDatum

b00t-c0re-lib/
├── src/
│   ├── mcp_whitelist.rs             # NEW: Whitelist validation
│   ├── mcp_registry.rs              # MODIFY: Integrate resolver & whitelist
│   └── lib.rs                       # EXPORT: Whitelist and resolver

b00t-mcp/
├── src/
│   └── mcp_registry_tools.rs        # MODIFY: Add llm_safe_install tool

_b00t_/
├── stacks/                          # NEW: Stack definitions
│   ├── postgres-dev-stack.stack.toml
│   ├── python-ml-stack.stack.toml
│   └── rust-dev-stack.stack.toml
├── *.mcp.toml                       # MODIFY: Add depends_on
├── *.cli.toml                       # MODIFY: Add depends_on
└── *.docker.toml                    # MODIFY: Add depends_on

~/.b00t/
└── mcp_whitelist.json               # NEW: Approved packages
```

---

## Example Use Cases

### Use Case 1: Installing MCP Server with Dependencies

```bash
# Traditional way (manual)
b00t-cli cli install python
b00t-cli cli install uvx
b00t-cli mcp install browser-use claudecode

# New way (automatic)
b00t-cli mcp install browser-use claudecode --install-deps
```

**System behavior**:
1. Reads `browser-use.mcp.toml`
2. Sees `depends_on = ["python.cli", "uvx.cli"]`
3. Checks if python installed → No → Installs python.cli
4. Checks if uvx installed → No → Installs uvx.cli
5. Installs browser-use MCP server
6. Wires to Claude Code

### Use Case 2: Deploying Database Stack

```toml
# postgres-dev-stack.stack.toml
[b00t]
name = "postgres-dev-stack"
type = "stack"
hint = "Full PostgreSQL development environment"

members = [
    "postgres-enhanced.docker",
    "pgadmin.docker",
    "redis.docker",
]

[b00t.env]
POSTGRES_PASSWORD = "${POSTGRES_PASSWORD}"
```

```bash
$ b00t-cli stack install postgres-dev-stack

📦 Resolving stack...
  ✅ docker.cli (installed)
  ✅ docker-compose.cli (installed)

📥 Installing members...
  ⏳ postgres-enhanced.docker
  ⏳ pgadmin.docker
  ⏳ redis.docker

📝 Generated: stacks/postgres-dev/docker-compose.yml

🚀 Starting stack...
  ✅ postgres-enhanced (port 5432)
  ✅ pgadmin (port 5050)
  ✅ redis (port 6379)

✅ Stack ready!
  Database: postgresql://postgres:password@localhost:5432/testdb
  PgAdmin:  http://localhost:5050
  Redis:    redis://localhost:6379
```

### Use Case 3: LLM-Driven Installation

**Scenario**: LLM agent notices user trying to read files

```python
# LLM reasoning
user_code = "I want to list all Python files in my project"
llm_thinks = "User needs filesystem operations → install filesystem MCP"

# LLM calls via MCP tool
result = await mcp_tool_call(
    tool="b00t_mcp_llm_safe_install",
    params={
        "server_id": "io.b00t/filesystem",
        "validate_whitelist": True,
        "auto_wire": True,
    }
)

# System behavior
1. Check whitelist → ✓ io.b00t/filesystem approved
2. Resolve deps → ✓ node.cli, npx.cli already installed
3. Confirm with user:
   "Install filesystem MCP server? (Whitelist approved, no new dependencies)"
4. Install → ✅
5. Wire to Claude Code → ✅

print("Filesystem MCP server installed! You can now read project files.")
```

---

## Security & Safety Considerations

1. **Whitelist-first for LLMs**: Never allow LLMs to install arbitrary packages
2. **User confirmation**: Always prompt for high-impact operations (docker, sudo)
3. **Dry-run mode**: `--dry-run` flag shows what would be installed without doing it
4. **Audit log**: Record all installations in `~/.b00t/install_audit.log`
5. **Sandboxing**: Docker containers should run with resource limits
6. **Version pinning**: Stacks should specify exact versions to avoid supply chain attacks

---

## Testing Strategy

1. **Unit tests**: Dependency resolver, topological sort, cycle detection
2. **Integration tests**: End-to-end stack installation with mocked Docker
3. **Whitelist tests**: Ensure unauthorized installations are blocked
4. **LLM tests**: Mock LLM requests to test agent workflows
5. **Regression tests**: Ensure backward compatibility with existing datums

---

## Migration Path

### Existing Datums

1. **Optional field**: `depends_on` is optional, defaults to empty
2. **Backward compatible**: Existing TOML files work unchanged
3. **Gradual migration**: Add dependencies to high-priority datums first (mcp servers)
4. **Auto-detection**: Tool can suggest dependencies based on `requires` field

### Registry Integration

1. **Metadata sync**: Registry already tracks dependencies
2. **Discovery**: Can auto-populate `depends_on` from registry metadata
3. **Validation**: Registry validates dependency chains on sync

---

## Success Metrics

1. **Developer productivity**: 80% reduction in manual dependency installation steps
2. **LLM safety**: 100% of LLM installations pass whitelist validation
3. **Stack usage**: 10+ commonly-used stacks defined and tested
4. **Auto-wiring accuracy**: 95%+ success rate detecting and wiring apps
5. **Error clarity**: Clear error messages for missing dependencies and circular deps

---

## Future Enhancements

1. **Nix integration**: Use Nix for reproducible builds
2. **Remote registries**: Pull stacks from GitHub/GitLab repos
3. **Version constraints**: Semver ranges for dependencies (`python.cli >= 3.9`)
4. **Conditional dependencies**: Platform-specific deps (Windows vs Linux)
5. **Health checks**: Verify installed services are actually running
6. **Rollback**: Undo stack installations with snapshot restore
7. **Multi-arch**: ARM64 support for Docker images in stacks

---

## Conclusion

This plan transforms b00t from a package manager into a dependency-aware software stack platform. The phased approach ensures backward compatibility while adding powerful new capabilities for both manual and LLM-driven workflows.

**Key Innovation**: Datum-based dependency references create a unified graph where MCP servers, Docker containers, CLI tools, and AI configs all understand their relationships.

**LLM Safety**: Whitelist validation ensures AI agents can only install approved packages, preventing malicious or accidental misuse.

**Developer Experience**: One-command stack deployment eliminates toil and ensures consistency across development, staging, and production environments.
