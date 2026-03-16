# Vision: `just` as First-Class b00t Datum Type with CLI Executor Traits

**Date**: 2026-03-15
**Status**: Actionable alignment plan
**Scope**: b00t-cli, just-mcp, tomllm crate

---

## 1. Problem Statement

b00t uses justfiles everywhere but has no `Justfile` datum type. just-mcp runs any justfile it can find — no registration, no sandboxing, no integration with b00t's trait hierarchy. This is a gap: b00t's datum system is the source of truth for what agents can discover and execute, yet the primary execution surface (justfiles) is invisible to it.

---

## 2. Architecture Overview

```
                          ┌─────────────────────────────────────┐
                          │         tomllm crate                │
                          │  ┌───────────────────────────────┐  │
                          │  │ define_typed_registry! macro   │  │
                          │  │  + DatumType::Justfile         │  │
                          │  │  + .justfile suffix            │  │
                          │  └───────────────────────────────┘  │
                          │  ┌───────────────────────────────┐  │
                          │  │ TomllmSchema trait (new)       │  │
                          │  │  per-type LSP-like extensions  │  │
                          │  └───────────────────────────────┘  │
                          └──────────────┬──────────────────────┘
                                         │
              ┌──────────────────────────┼──────────────────────────┐
              │                          │                          │
    ┌─────────▼──────────┐   ┌───────────▼──────────┐   ┌──────────▼──────────┐
    │   b00t-cli         │   │   just-mcp            │   │  future MCP servers │
    │  ┌──────────────┐  │   │  ┌──────────────────┐ │   │  (same trait stack) │
    │  │ JustfileDatum │  │   │  │ JustMcpServer    │ │   └─────────────────────┘
    │  │  : CliExecutor│  │   │  │  reads registry  │ │
    │  │  : DatumCheck │  │   │  │  only registered │ │
    │  │  : Sandboxable│  │   │  │  justfiles run   │ │
    │  └──────────────┘  │   │  └──────────────────┘ │
    └────────────────────┘   └───────────────────────┘
```

---

## 3. Phase 1: `Justfile` Datum Type

### 3.1 Add `Justfile` to `DatumType` enum

**File**: `b00t-cli/src/lib.rs` (line ~235)

```rust
tomllm::define_typed_registry! {
    pub enum DatumType {
        // ... existing types ...
        Justfile    => ".justfile"    // NEW
    }
}
```

This single line gives us:
- `DatumType::Justfile` variant
- `.justfile.toml` / `.justfile.tomllm` file discovery
- `DatumType::from_filename("app4dog.justfile.tomllm")` → `DatumType::Justfile`
- `all_base_suffixes()` includes `.justfile`

### 3.2 Datum schema: `*.justfile.tomllm`

```toml
[b00t]
# 🤓 Justfile datum — registers a justfile for agent-executable recipes
# 🤓 Only registered justfiles are runnable via just-mcp
name      = "app4dog-workspace"
type      = "justfile"
path      = "justfile"                        # relative to project root
mcp_server = "just-mcp"                       # which MCP server introspects this

[b00t.justfile]
recipe_groups = ["dev", "db", "ci", "acr", "cvat", "ml"]
sandbox       = "local"                       # local | container | wasm
allow_side_effects = true                     # run_recipe is side-effecting
require       = ["docker", "just", "tofu"]    # runtime deps

[b00t.justfile.capabilities]
# 🤓 Declarative capability list — sandbox reads this before granting execution
network   = true
filesystem = [".", "/mnt/blobfuse2-ml-training"]
env_vars  = ["AZURE_*", "HUGGING_FACE_HUB_TOKEN"]
secrets   = ["ACR_ADMIN_PASSWORD"]            # must be injected, never logged
```

### 3.3 Create `datum_justfile.rs`

**File**: `b00t-cli/src/datum_justfile.rs`

```rust
use crate::traits::*;
use crate::BootDatum;

pub struct JustfileDatum {
    pub datum: BootDatum,
    pub justfile_path: PathBuf,
}

impl DatumChecker for JustfileDatum {
    fn is_installed(&self) -> bool {
        self.justfile_path.exists() && check_command_available("just")
    }
    fn current_version(&self) -> Option<String> {
        // hash of justfile content — changes trigger re-validation
        hash_file(&self.justfile_path).ok()
    }
}

impl StatusProvider for JustfileDatum { /* name, subsystem="justfile", hint */ }
impl FilterLogic for JustfileDatum { /* prerequisites = just + deps in require */ }
impl ConstraintEvaluator for JustfileDatum { /* env var checks, OS checks */ }
impl DatumProvider for JustfileDatum {
    fn datum(&self) -> &BootDatum { &self.datum }
}
```

---

## 4. Phase 2: `CliExecutor` Trait (Polymorphic Execution)

### 4.1 The trait hierarchy

**File**: `b00t-cli/src/traits.rs` (extend existing)

```rust
/// Monadic execution result — wraps output with metadata
pub struct ExecOutput<T> {
    pub value: T,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub sandbox: SandboxContext,
    pub side_effects: Vec<SideEffect>,
}

impl<T> ExecOutput<T> {
    /// Monadic bind — chain executions, propagate context
    pub fn and_then<U, F: FnOnce(T) -> Result<ExecOutput<U>>>(
        self, f: F
    ) -> Result<ExecOutput<U>> { ... }

    /// Map over value, preserve metadata
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ExecOutput<U> { ... }
}

/// Core executor trait — any datum type that runs commands
pub trait CliExecutor: DatumProvider {
    type Output;

    /// Execute with validated parameters
    fn execute(&self, args: &[String]) -> Result<ExecOutput<Self::Output>>;

    /// Dry-run: return what would execute without side effects
    fn dry_run(&self, args: &[String]) -> Result<ExecPlan>;

    /// List available subcommands/recipes
    fn list_commands(&self) -> Result<Vec<CommandSignature>>;

    /// Requirements for execution in a sandbox
    fn sandbox_requirements(&self) -> SandboxRequirements;
}

/// Implemented by justfile datums
impl CliExecutor for JustfileDatum {
    type Output = String;

    fn execute(&self, args: &[String]) -> Result<ExecOutput<String>> {
        // delegates to `just --justfile <path> <recipe> <args>`
    }

    fn list_commands(&self) -> Result<Vec<CommandSignature>> {
        // delegates to just-mcp list_recipes or `just -l --justfile <path>`
    }

    fn sandbox_requirements(&self) -> SandboxRequirements {
        // reads [b00t.justfile.capabilities] from datum
    }
}
```

### 4.2 Macro for executor boilerplate

**File**: `tomllm/src/executor_macro.rs`

```rust
/// Generates CliExecutor impl for datum types that wrap shell commands
macro_rules! impl_cli_executor {
    ($datum_type:ty, $command:expr) => {
        impl CliExecutor for $datum_type {
            type Output = String;

            fn execute(&self, args: &[String]) -> Result<ExecOutput<String>> {
                let plan = self.dry_run(args)?;
                let sandbox = self.sandbox_requirements();
                // validate sandbox allows this execution
                sandbox.validate(&plan)?;
                // execute
                let output = cmd!("bash", "-c", &plan.command_line)
                    .dir(&plan.working_dir)
                    .read()?;
                Ok(ExecOutput {
                    value: output,
                    exit_code: 0,
                    duration_ms: plan.elapsed(),
                    sandbox: sandbox.context(),
                    side_effects: plan.declared_effects(),
                })
            }
        }
    };
}
```

### 4.3 Trait objects for polymorphic dispatch

```rust
/// Registry of all executable datums (justfiles, CLIs, scripts, jobs)
pub fn load_executors(path: &str) -> Result<Vec<Box<dyn CliExecutor<Output=String>>>> {
    let datums = get_all_datums(path)?;
    datums.values()
        .filter(|d| matches!(d.datum_type_enum(),
            Some(DatumType::Justfile | DatumType::Cli | DatumType::Bash)))
        .map(|d| -> Box<dyn CliExecutor<Output=String>> {
            match d.datum_type_enum() {
                Some(DatumType::Justfile) => Box::new(JustfileDatum::from(d)),
                Some(DatumType::Cli) => Box::new(CliDatum { datum: d.clone() }),
                Some(DatumType::Bash) => Box::new(BashDatum { datum: d.clone() }),
                _ => unreachable!(),
            }
        })
        .collect()
}
```

---

## 5. Phase 3: just-mcp Registry Gate

### 5.1 Only run registered justfiles

**File**: `just-mcp-lib/src/mcp_server.rs` (modify)

Current behavior: just-mcp runs any justfile in `--directory`.

New behavior: just-mcp checks a registry before execution.

```rust
pub struct JustMcpServer {
    working_dir: PathBuf,
    tool_router: ToolRouter<Self>,
    registry: JustfileRegistry,        // NEW
}

pub struct JustfileRegistry {
    /// Registered justfile paths (absolute, canonicalized)
    allowed: HashSet<PathBuf>,
    /// Loaded from b00t _b00t_/*.justfile.tomllm datums
    datums: HashMap<PathBuf, JustfileDatumConfig>,
}

impl JustfileRegistry {
    /// Load from b00t datum directory
    pub fn from_b00t_path(path: &Path) -> Result<Self> {
        // scan _b00t_/ for *.justfile.tomllm
        // parse each, extract [b00t.justfile.path]
        // canonicalize and register
    }

    /// Check if a justfile path is registered
    pub fn is_allowed(&self, path: &Path) -> bool {
        self.allowed.contains(&path.canonicalize().unwrap_or_default())
    }

    /// Get sandbox requirements for a registered justfile
    pub fn sandbox_for(&self, path: &Path) -> Option<&SandboxRequirements> {
        self.datums.get(path).map(|d| &d.sandbox)
    }
}
```

### 5.2 Gate in tool handlers

```rust
#[tool(description = "Execute a recipe from a registered justfile")]
async fn run_recipe(
    &self,
    Parameters(params): Parameters<ExecuteRecipeParams>,
) -> Result<CallToolResult, McpError> {
    let justfile_path = self.resolve_justfile_path(&params)?;

    // GATE: only registered justfiles
    if !self.registry.is_allowed(&justfile_path) {
        return Err(McpError::invalid_params(
            format!("justfile not registered: {}", justfile_path.display()),
            None,
        ));
    }

    // ... existing execution logic ...
}
```

---

## 6. Phase 4: Sandbox Abstraction

### 6.1 `Sandboxable` trait

```rust
/// Execution environments for agentic sandboxing
pub enum SandboxKind {
    Local,              // direct execution (current)
    Container(String),  // OCI container image
    Wasm(String),       // WASM module path
    Nsjail(NsjailConfig), // Linux namespace jail (future)
}

pub trait Sandboxable: CliExecutor {
    /// Declare what this executor needs from the sandbox
    fn sandbox_requirements(&self) -> SandboxRequirements;

    /// Execute within sandbox constraints
    fn execute_sandboxed(
        &self,
        args: &[String],
        sandbox: &SandboxKind,
    ) -> Result<ExecOutput<Self::Output>>;
}

pub struct SandboxRequirements {
    pub network: bool,
    pub filesystem: Vec<PathBuf>,     // allowed read/write paths
    pub env_vars: Vec<String>,        // allowed env var patterns (glob)
    pub secrets: Vec<String>,         // must be injected, never logged
    pub max_duration: Duration,       // kill after this
    pub max_memory_mb: u64,
    pub capabilities: Vec<String>,    // Linux capabilities needed
}
```

### 6.2 Datum-declared capabilities as sandbox contracts

The justfile datum's `[b00t.justfile.capabilities]` section maps directly to `SandboxRequirements`. This isn't a "request" — it's a **tested contract**. The capabilities have been verified/validated/tested to be both necessary and sufficient for the recipe to execute at the required assurance level.

The enforcement stack:
1. **Datum registration** — only registered justfiles exist in the agent's filesystem view
2. **eBPF syscall interception** — `openat()`, `execve()`, `connect()` filtered by capabilities
3. **Context scoping** — the agent receives only the tomllm annotations relevant to its task
4. **Deterministic reduction** — not a firewall (block/allow) but a lens (in-scope / not-in-scope)

The sandbox doesn't "deny access" — it shapes what the agent can perceive. An agent scoped to `app4dog.justfile` sees the recipes, their dependencies, and the filesystem paths declared in `capabilities.filesystem`. Everything else is simply absent.

---

## 7. Phase 5: `.tomllm` as Ontological Meta-Superset

### 7.1 Per-type schema extensions

**File**: `tomllm/src/schema.rs` (new)

```rust
/// Each datum type declares its own TOML subsections
pub trait TomllmSchema: Sized {
    /// The base suffix this schema handles (e.g., ".justfile")
    const SUFFIX: &'static str;

    /// Required [b00t] fields for this type
    fn required_fields() -> &'static [&'static str];

    /// Optional [b00t.*] subsections with their schemas
    fn subsections() -> Vec<SubsectionSchema>;

    /// Validate a parsed datum against this schema
    fn validate(datum: &BootDatum) -> Vec<SchemaViolation>;

    /// LSP-like completion items for this type's subsections
    fn completions(context: &CompletionContext) -> Vec<CompletionItem>;
}

pub struct SubsectionSchema {
    pub path: String,              // e.g., "b00t.justfile.capabilities"
    pub fields: Vec<FieldSchema>,
    pub description: String,
}

pub struct FieldSchema {
    pub name: String,
    pub toml_type: TomlType,
    pub required: bool,
    pub description: String,
    pub default: Option<toml::Value>,
}
```

### 7.2 Macro-driven schema declaration

```rust
tomllm::define_datum_schema! {
    JustfileSchema for DatumType::Justfile {
        required: [name, type, path],
        subsections: {
            "b00t.justfile" => {
                recipe_groups: Array<String>,
                sandbox: String = "local",
                allow_side_effects: Bool = true,
                require: Array<String> = [],
            },
            "b00t.justfile.capabilities" => {
                network: Bool = false,
                filesystem: Array<String> = ["."],
                env_vars: Array<String> = [],
                secrets: Array<String> = [],
            },
        }
    }
}
```

### 7.3 The `.tomllm` intentional-break convention

`.tomllm` files use `#` comments with emoji jargon (`🤓`, `💡例`, `⚠️需`, `🔗参`) that are valid TOML comments but intentionally encode structured metadata that standard TOML parsers ignore. This creates a **happy path**: agents that use the `tomllm` crate get rich context (tribal knowledge, examples, prerequisites, references); agents that parse raw TOML get correct but impoverished data.

The `tomllm` crate's `parser.rs` already extracts these comments. The next step is to formalize them as **ontological annotations**:

```rust
pub enum TomllmAnnotation {
    /// 🤓 Tribal knowledge — explains "why", not "what"
    TribalKnowledge(String),
    /// 💡例 Usage example — executable context
    Example(String),
    /// ⚠️需 Prerequisite — must be true before this datum is valid
    Prerequisite(String),
    /// 🔗参 Reference — pointer to external resource
    Reference(String),
}
```

Each annotation associates with the next TOML key or section. Schema validators can check that prerequisites are met, examples are valid, and references are reachable.

---

## 8. Phase 6: Monadic Composition

### 8.1 Recipe execution as monadic pipeline

```rust
/// A recipe execution plan — composable via and_then/map
pub struct RecipePlan<T> {
    steps: Vec<RecipeStep>,
    sandbox: SandboxRequirements,
    phantom: PhantomData<T>,
}

impl<T> RecipePlan<T> {
    /// Chain: run this plan, then another using its output
    pub fn and_then<U, F>(self, f: F) -> RecipePlan<U>
    where F: FnOnce(T) -> RecipePlan<U> { ... }

    /// Map: transform the output without changing execution
    pub fn map<U, F>(self, f: F) -> RecipePlan<U>
    where F: FnOnce(T) -> U { ... }

    /// Merge sandbox requirements (union of capabilities)
    pub fn merge_sandbox(self, other: &SandboxRequirements) -> Self { ... }
}
```

### 8.2 Example: composing app4dog recipes

```rust
// Agent builds a pipeline by composing registered justfile recipes
let pipeline = workspace.recipe("db-is-up")     // ensure DB
    .and_then(|_| workspace.recipe("ml-train-local"))  // train
    .and_then(|model| {
        pupper_ml.recipe("export")             // export ONNX
            .with_arg("model", &model.path)
    })
    .map(|onnx| onnx.path);                   // extract path

// Sandbox requirements auto-merge across the chain
let sandbox = pipeline.sandbox;
// sandbox.filesystem = union of all recipe filesystems
// sandbox.network = any recipe needs network
// sandbox.secrets = union of all recipe secrets

// Execute within merged sandbox
let result = pipeline.execute_sandboxed(&SandboxKind::Local)?;
```

---

## 9. Implementation Order

| Phase | What | Where | Effort | Depends On |
|-------|------|-------|--------|------------|
| 1a | Add `Justfile` to `DatumType` enum | `b00t-cli/src/lib.rs` | 1 line | — |
| 1b | Create `datum_justfile.rs` | `b00t-cli/src/` | ~100 lines | 1a |
| 1c | Write `app4dog.justfile.tomllm` | `_b00t_/` | ~30 lines | 1a |
| 2a | Define `CliExecutor` trait | `b00t-cli/src/traits.rs` | ~60 lines | — |
| 2b | `impl CliExecutor for JustfileDatum` | `datum_justfile.rs` | ~50 lines | 1b, 2a |
| 2c | `impl CliExecutor for CliDatum` | `datum_cli.rs` | ~40 lines | 2a |
| 3 | just-mcp registry gate | `just-mcp-lib/src/` | ~120 lines | 1c |
| 4 | `Sandboxable` trait + requirements | `b00t-cli/src/traits.rs` | ~80 lines | 2a |
| 5a | `TomllmSchema` trait | `tomllm/src/schema.rs` | ~150 lines | — |
| 5b | `define_datum_schema!` macro | `tomllm/src/` | ~200 lines | 5a |
| 5c | `TomllmAnnotation` extraction | `tomllm/src/parser.rs` | ~80 lines | — |
| 6 | Monadic `RecipePlan` | `b00t-cli/src/` | ~100 lines | 2b, 4 |

**Total estimated**: ~1000 lines of Rust across 6 phases.

---

## 10. Key Design Decisions

### Why traits, not enums?
The `BootDatum` struct is already a "god struct" with every possible field. The traits (`DatumChecker`, `StatusProvider`, `CliExecutor`, `Sandboxable`) provide the polymorphic dispatch that the struct cannot. Trait objects (`Box<dyn CliExecutor>`) enable heterogeneous collections of executable datums.

### Why monadic `ExecOutput`?
Recipe execution is inherently composable — "run A, then B with A's output". The monadic pattern (`and_then`, `map`) makes this explicit and carries sandbox requirements through the chain. This is idiomatic Rust (cf. `Result`, `Option`) and avoids imperative error-handling spaghetti.

### Why intentionally break TOML?
Standard TOML parsers see `# 🤓 this is tribal knowledge` as a comment and discard it. The `tomllm` crate extracts it as structured metadata. This is not about information asymmetry or rewarding "good" agents — it's about **deterministic context reduction for assurance**.

The tomllm annotations declare precisely what context an agent needs to perform a task. An eBPF-based sandbox intercepts filesystem syscalls and uses these declarations to scope visibility: if a datum isn't registered for this agent's task, the files don't appear at all. The agent doesn't receive degraded data — it receives exactly the context that has been verified, validated, and tested to be requisite for the task at the required assurance/compliance level.

This is a compliance model, not a carrot/stick model:
- **Registered + demonstrated capability** → full context visible, execution permitted
- **Unregistered** → invisible at the syscall level (not hidden, not degraded — simply not in scope)
- The jargon comments (`🤓`, `💡例`, `⚠️需`, `🔗参`) are the verification layer that declares what "in scope" means for each datum type

### Why registry-gated execution?
Unrestricted `just-mcp` is a security risk in agentic sandboxes — an agent could point it at any justfile and run arbitrary shell commands. The registry gate ensures only b00t-registered justfiles are executable, and their `[b00t.justfile.capabilities]` section declares what the sandbox must provide.

But the deeper reason is assurance: the registry is the set of justfiles whose recipes have been verified to perform correctly within declared sandbox constraints. An unregistered justfile isn't "blocked" — it doesn't exist in the agent's filesystem view. The eBPF syscall layer ensures this deterministically. The `[b00t.justfile.capabilities]` section isn't just a request for resources — it's a tested contract between the recipe and the sandbox, validated at the datum level before the agent ever sees the file.

---

## 11. Datum Relationships (Entanglement)

```
app4dog.justfile.tomllm
  ├── entangled_mcp = ["just-mcp.mcp"]        # introspected by this MCP server
  ├── entangled_cli = ["just.cli", "tofu.cli"] # runtime deps
  └── depends_on = ["docker.cli"]              # must be installed

just-mcp.mcp.tomllm
  ├── datum_type_served = "justfile"           # this MCP serves justfile datums
  └── entangled_cli = ["just.cli"]             # needs just binary

app4dog--workspace.stack.tomllm
  ├── members = ["app4dog.justfile", "middleware.justfile", ...]
  └── justfiles listed with recipe_groups for agent navigation
```

---

## 12. Future: LSP-like Type Extensions

Each `.tomllm` datum type gets its own completion/validation schema. When an editor (or agent) opens `foo.justfile.tomllm`, the tomllm LSP (future) knows:

- `[b00t.justfile]` is a valid subsection
- `recipe_groups` must be `Array<String>`
- `sandbox` must be one of `"local" | "container" | "wasm"`
- `[b00t.justfile.capabilities]` has known fields with defaults

This is the **ontological meta-superset**: `.tomllm` schemas are versioned (`desires = "0.1.0"`) and each application (b00t, just-mcp, app4dog) can extend the schema for its own datum types while remaining TOML-compatible.

---

*This plan is grounded in the existing b00t-cli architecture (BootDatum, define_typed_registry!, trait hierarchy, tomllm crate) and extends it with minimal surface area. Each phase is independently shippable.*
