#![allow(dead_code, async_fn_in_trait)]
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use duct::cmd;
use regex::Regex;
use std::io::Write;
use std::collections::HashSet;
use std::sync::OnceLock;
use toml;
use shellexpand;

// 🤓 write_event moved to b00t-c0re-lib::events — unified telemetry writer
pub use b00t_c0re_lib::write_event;

/// ANSI color helpers — auto-disable when stdout is not a terminal.
pub mod ansi {
    pub fn enabled() -> bool {
        use std::io::IsTerminal;
        std::io::stdout().is_terminal()
    }
    pub fn green(s: &str) -> String { if enabled() { format!("\x1b[32m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn yellow(s: &str) -> String { if enabled() { format!("\x1b[33m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn red(s: &str) -> String { if enabled() { format!("\x1b[31m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn cyan(s: &str) -> String { if enabled() { format!("\x1b[36m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn dim(s: &str) -> String { if enabled() { format!("\x1b[2m{}\x1b[0m", s) } else { s.to_string() } }
    pub fn bold(s: &str) -> String { if enabled() { format!("\x1b[1m{}\x1b[0m", s) } else { s.to_string() } }
}

// warn-once registry — one warning per unknown datum type string per process
static DATUM_TYPE_WARNED: OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> = OnceLock::new();

/// Returns true if the value is a well-known content tag (not a typed datum).
fn is_known_content_tag(s: &str) -> bool {
    matches!(s, "okr" | "prd" | "pattern" | "datum" | "reference" | "learn" | "hardware" | "tomllmd"
        | "specification" | "topic" | "soul" | "install" | "github_org" | "ai_provider" | "pyinfra" | "wow")
}

/// Load incubating datum types from a runtime‑defined datum.
/// The datum is expected at `$_B00T_Path/incubating.tomllm` (default: `~/.b00t/_b00t_/incubating.tomllm`)
/// with TOML shape:
/// ```toml
/// incubating = ["routing", "agent.cli", ...]
/// ```
/// Missing or malformed files yield an empty set.
fn get_incubating_set() -> &'static HashSet<String> {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        let base_path = std::env::var("_B00T_Path")
            .unwrap_or_else(|_| "~/.b00t/_b00t_".to_string());
        let expanded = shellexpand::tilde(&base_path).to_string();
        let file_path = std::path::Path::new(&expanded).join("incubating.tomllm");
        if let Ok(content) = std::fs::read_to_string(&file_path) {
            #[derive(serde::Deserialize)]
            struct Config {
                incubating: Vec<String>,
            }
            if let Ok(cfg) = toml::from_str::<Config>(&content) {
                return cfg.incubating.into_iter().collect();
            }
        }
        HashSet::new()
    })
}

pub mod exit_code {
    /// Generic / unknown error
    pub const ERROR: i32 = 1;
    /// Datum or resource not found
    pub const NOT_FOUND: i32 = 2;
    /// Invalid arguments or syntax
    pub const USAGE: i32 = 3;
    /// Permission / auth / credential failure
    pub const ACCESS: i32 = 4;
    /// Gate precondition not satisfied (command/env/file missing)
    pub const GATE: i32 = 10;
    /// Dependency resolution failure
    pub const DEP: i32 = 11;
    /// MCP server not found or install failed
    pub const MCP: i32 = 20;
    /// Network / connectivity failure
    pub const NETWORK: i32 = 30;
}
use serde::{Deserialize, Deserializer, Serialize};

pub mod agentic_role;
pub mod ansible;
pub mod blessing;
pub mod datum_schema;
pub mod bootstrap;
pub mod budget_controller;
pub mod cloud_sync;
pub mod assimilate;
pub mod commands;
pub mod datum_ai;
pub mod datum_ai_model;
pub mod datum_api;
pub mod datum_apt;
pub mod datum_bash;
pub mod datum_cli;
pub mod datum_config;
pub mod datum_database;
pub mod datum_docker;
pub mod datum_guard;
pub mod datum_gemini;
pub mod training_examples;
pub mod datum_job;
pub mod datum_justfile;
pub mod datum_pipeline;
pub mod datum_k8s;
pub mod datum_mcp;
pub mod datum_repo;
pub mod datum_skill;
pub mod datum_stack;
pub mod datum_triples;
pub mod datum_proof;
pub mod datum_store;
pub mod datum_utils;
pub mod datum_vscode;
pub mod query_sources;
#[cfg(feature = "dbus")]
pub mod dbus_dispatch;
pub mod dependency_resolver;
pub mod entanglement;
pub mod erp;
pub mod errors;
pub mod governance;
pub mod guards;
pub mod hive;
pub mod hook_engine;
pub mod install;
pub mod inventory;
pub mod job_executor;
pub mod job_ipc;
pub mod job_state;
pub mod just_ast;
pub mod k0mmand3r;
pub mod k8s;
pub mod memory_provider;
pub mod model_manager;
pub mod model_registry;
pub mod orchestrator;
pub mod runtime_sandbox;
pub mod scheduler;
pub mod session_memory;
pub mod skill_resolver;
pub mod semantic_patch;
pub mod soul_writer;
pub mod step;
pub mod traits;
pub mod utils;
pub mod variant;
pub mod viz;
pub mod whoami;
pub mod wow;
pub mod calorie_tracker;
pub mod cake_ledger;
pub mod a2a_gates;
#[cfg(feature = "rpa")]
pub mod rpa_cdp;
#[cfg(feature = "rpa")]
pub mod rpa_tui;
#[cfg(any(feature = "rpa", feature = "rpa-playwright"))]
pub mod rpa_backend;
#[cfg(feature = "rpa")]
pub mod rpa_rhai;
pub use traits::*;

pub const PRUNE_DIRS: &[&str] = &[
    "node_modules",
    ".cache",
    ".cargo",
    ".rustup",
    "target",
    ".git",
    "vendor",
    "_archive_",
    ".local",
    ".npm",
    ".pnpm-store",
    ".mozilla",
    ".vscode",
    ".codeium",
    ".config",
    "snap",
];

pub fn sweep_backup_files(root: &std::path::Path) -> usize {
    use walkdir::WalkDir;
    let mut removed = 0usize;
    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !PRUNE_DIRS.contains(&name)
        })
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_file() && name.ends_with('~') {
            if let Err(e) = std::fs::remove_file(path) {
                eprintln!("  ⚠️  {}: {e}", path.display());
            } else {
                removed += 1;
            }
        }
    }
    removed
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct ApiProvides {
    pub capability: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub hint: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct McpConfig {
    pub mcp: McpServer,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct UnifiedConfig {
    pub b00t: BootDatum,
    pub env: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub sections: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct UsageExample {
    pub description: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct LearnMeta {
    pub topic: Option<String>,
    pub inline: Option<String>,
    pub auto_digest: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct BudgetConstraint {
    pub daily_limit: Option<f64>,
    pub cost_per_job: Option<f64>,
    pub on_budget_exceeded: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct GpuRequirements {
    pub count: Option<u32>,
    pub memory: Option<String>,
    pub gpu_type: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct OrchestrationConfig {
    pub budget_constraint: Option<BudgetConstraint>,
    pub budget_currency: Option<String>,
    pub resource_requirements: Option<std::collections::HashMap<String, String>>,
    pub gpu_requirements: Option<GpuRequirements>,
    pub schedule_type: Option<String>,
    pub gpu_batch_group: Option<String>,
    pub requires_stacks: Option<Vec<String>>,
    pub queue_name: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum InstallSpec {
    Command(String),
    Package(PackageInstallSpec),
    Tool(ToolInstallSpec),
    Metadata { requires: Option<Vec<String>> },
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PackageInstallSpec {
    pub package: String,
    pub binary: Option<String>,
    pub apt: Option<String>,
    pub dnf: Option<String>,
    pub pacman: Option<String>,
    pub brew: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolInstallSpec {
    pub cargo: Option<String>,
    pub go: Option<String>,
    pub npm_global: Option<String>,
    pub uv_tool: Option<String>,
    pub binary: Option<String>,
    pub version: Option<String>,
}

impl InstallSpec {
    pub fn command(&self) -> Option<&str> {
        match self {
            InstallSpec::Command(command) => Some(command),
            InstallSpec::Package(_) => None,
            InstallSpec::Tool(_) => None,
            InstallSpec::Metadata { .. } => None,
        }
    }

    pub fn command_string(&self) -> Option<String> {
        match self {
            InstallSpec::Command(command) => Some(command.clone()),
            InstallSpec::Package(package) => Some(package.install_script()),
            InstallSpec::Tool(tool) => tool.install_script(),
            InstallSpec::Metadata { .. } => None,
        }
    }
}

impl PackageInstallSpec {
    fn install_script(&self) -> String {
        let binary = shell_quote(self.binary.as_deref().unwrap_or(&self.package));
        let apt = shell_quote(self.apt.as_deref().unwrap_or(&self.package));
        let dnf = shell_quote(self.dnf.as_deref().unwrap_or(&self.package));
        let pacman = shell_quote(self.pacman.as_deref().unwrap_or(&self.package));
        let brew = shell_quote(self.brew.as_deref().unwrap_or(&self.package));

        format!(
            r#"set -euo pipefail
if command -v {binary} >/dev/null 2>&1; then
  {binary} --version || true
  exit 0
fi

if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update
  sudo apt-get install -y {apt}
elif command -v dnf >/dev/null 2>&1; then
  sudo dnf install -y {dnf}
elif command -v pacman >/dev/null 2>&1; then
  sudo pacman -S --needed --noconfirm {pacman}
elif command -v brew >/dev/null 2>&1; then
  brew install {brew}
else
  echo "No supported package manager found for {binary} (apt-get, dnf, pacman, brew)." >&2
  exit 127
fi
"#
        )
    }
}

impl ToolInstallSpec {
    fn install_script(&self) -> Option<String> {
        if let Some(crate_name) = &self.cargo {
            let binary = shell_quote(self.binary.as_deref().unwrap_or(crate_name));
            let crate_name = shell_quote(crate_name);
            let version_arg = self
                .version
                .as_ref()
                .map(|version| format!(" --version {}", shell_quote(version)))
                .unwrap_or_default();
            return Some(format!(
                r#"set -euo pipefail
if command -v {binary} >/dev/null 2>&1; then
  {binary} --version || true
  exit 0
fi
command -v cargo >/dev/null 2>&1 || {{ echo "cargo is required to install {binary}" >&2; exit 127; }}
cargo install {crate_name}{version_arg}
"#
            ));
        }

        if let Some(module) = &self.go {
            let binary = self
                .binary
                .clone()
                .unwrap_or_else(|| infer_go_binary(module));
            let binary = shell_quote(&binary);
            let module = shell_quote(module);
            return Some(format!(
                r#"set -euo pipefail
if command -v {binary} >/dev/null 2>&1; then
  {binary} --version || true
  exit 0
fi
command -v go >/dev/null 2>&1 || {{ echo "go is required to install {binary}" >&2; exit 127; }}
GO111MODULE=on go install {module}
"#
            ));
        }

        if let Some(package) = &self.npm_global {
            let binary = self
                .binary
                .clone()
                .unwrap_or_else(|| infer_npm_binary(package));
            let binary = shell_quote(&binary);
            let package = shell_quote(package);
            return Some(format!(
                r#"set -euo pipefail
if command -v {binary} >/dev/null 2>&1; then
  {binary} --version || true
  exit 0
fi
command -v npm >/dev/null 2>&1 || {{ echo "npm is required to install {binary}" >&2; exit 127; }}
npm install -g {package}
"#
            ));
        }

        if let Some(package) = &self.uv_tool {
            let binary = shell_quote(self.binary.as_deref().unwrap_or(package));
            let package = shell_quote(package);
            return Some(format!(
                r#"set -euo pipefail
if command -v {binary} >/dev/null 2>&1; then
  {binary} --version || true
  exit 0
fi
command -v uv >/dev/null 2>&1 || {{ echo "uv is required to install {binary}" >&2; exit 127; }}
uv tool install {package}
"#
            ));
        }

        None
    }
}

fn infer_go_binary(module: &str) -> String {
    module
        .trim_end_matches("@latest")
        .rsplit('/')
        .next()
        .unwrap_or(module)
        .to_string()
}

fn infer_npm_binary(package: &str) -> String {
    package
        .split('@')
        .next()
        .unwrap_or(package)
        .rsplit('/')
        .next()
        .unwrap_or(package)
        .to_string()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct K0mmand3rDatumConfig {
    /// Slash command exposed for this datum (for example "/gh" or "/docker").
    /// If omitted, defaults to "/<b00t.name>".
    pub slash: Option<String>,
    /// When true, omit from discovery listings (still invokable directly).
    pub hidden: Option<bool>,
    /// Optional dispatch hint for operator-facing help output.
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct KnowledgeConfig {
    pub backend: Option<String>,
    pub namespace: Option<String>,
    pub data_path: Option<String>,
    pub xor_group: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct MaintenanceConfig {
    /// How many days between checking for a newer version.
    pub check_interval_days: Option<u64>,
    /// Deterministic command (no LLM) that outputs the latest version string.
    /// e.g. `curl -s https://api.github.com/repos/casey/just/releases/latest | jq -r .tag_name`
    pub check_command: Option<String>,
    /// Human-readable source description, e.g. "github:casey/just" or "crates.io"
    pub version_source: Option<String>,
    /// Regex to extract semver from check_command output (default: same as version_regex)
    pub check_regex: Option<String>,
}

/// Mount entry for filesystem isolation in a runtime wrapper profile.
/// Mirrors bubblewrap's `--ro-bind`, `--bind`, `--tmpfs`, `--dev`, `--proc`.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct MountEntry {
    #[serde(default)]
    pub src: String,
    pub dest: String,
    #[serde(rename = "type")]
    pub mount_type: String, // "ro-bind", "bind", "tmpfs", "dev", "proc", "symlink"
}

/// Seccomp filter profile for runtime sandboxing.
/// If present, a strict allowlist is applied; absent = no seccomp filter.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct SeccompProfile {
    #[serde(default)]
    pub allow: Vec<String>, // syscall names to allow (e.g. "read", "write", "exit")
    #[serde(default)]
    pub default_action: Option<String>, // "allow" | "kill" | "errno" (default: "kill")
}

/// Isolation profile for a runtime wrapper — declarative sandbox config.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct IsolationConfig {
    /// Bind mounts / filesystem entries.
    pub mounts: Option<Vec<MountEntry>>,
    /// Share network namespace with host (default: true).
    pub share_net: Option<bool>,
    /// Share IPC namespace with host (default: false).
    pub share_ipc: Option<bool>,
    /// Share PID namespace with host (default: false).
    pub share_pid: Option<bool>,
    /// Share UTS namespace with host (default: false).
    pub share_uts: Option<bool>,
    /// New session (setsid) — detach from controlling terminal.
    pub new_session: Option<bool>,
    /// Seccomp filter profile.
    pub seccomp: Option<SeccompProfile>,
    /// Capabilities to retain (whitelist; if empty, drop all).
    pub caps_retain: Option<Vec<String>>,
    /// Working directory inside the sandbox (default: /).
    pub cwd: Option<String>,
    /// Hostname inside UTS namespace.
    pub hostname: Option<String>,
}

/// Runtime wrapper config — declarative sandboxed application launch profile.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct RuntimeConfig {
    /// Binary to launch (resolved from PATH or absolute path).
    pub binary: String,
    /// Arguments to pass to the binary before passthrough args.
    pub args: Option<Vec<String>>,
    /// Working directory before entering sandbox.
    pub workdir: Option<String>,
    /// Isolation / sandbox profile.
    pub isolation: Option<IsolationConfig>,
    /// Pre-launch Rhai hook script (executed before sandbox entry).
    pub hook_pre: Option<String>,
    /// Post-launch hook (NOT sandboxed — runs after the child exits).
    pub hook_post: Option<String>,
}

/// A single artifact reference within a polyseme datum.
/// Each ref resolves one meaning of the polysemous name to a concrete datum.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct PolysemeRef {
    /// Short disambiguating name (e.g. "bubblewrap-sandbox", "bubblewrap-android")
    pub name: String,
    /// Canonical upstream identifier (e.g. "github:containers/bubblewrap")
    pub canonical: String,
    /// Concrete datum name this ref resolves to (e.g. "bubblewrap-sandbox.cli")
    pub datum: String,
    /// Human-readable description of this specific meaning
    pub description: String,
}

/// Polyseme config — blackhole/box of artifact references.
/// A polyseme datum contains NO install/run logic itself; it is a
/// knowledge-graph branch point that maps a name to its possible meanings.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct PolysemeConfig {
    /// Named artifact references — each one is a distinct resolution.
    pub refs: Option<Vec<PolysemeRef>>,
    /// Source URLs that were assimilated to build this polyseme.
    pub sources: Option<Vec<String>>,
}

/// Measured composition metric — one `{metric=..., value=...}` entry in
/// `[b00t.compose]` `measured`. Emitted as a `b00t:measured` triple with
/// object "metric=value" (e.g. "context_savings_record_lesson=94%").
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct MeasuredMetric {
    pub metric: String,
    pub value: String,
}

/// Composition knowledge — `[b00t.compose]` table.
/// Makes capability composition graph-visible: datum_triples emits
/// b00t:composes_with / b00t:audits / b00t:supersedes / b00t:measured
/// triples from these fields (previously comment-prose only).
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct ComposeConfig {
    /// Datums this capability composes with into a larger capability.
    pub composes_with: Option<Vec<String>>,
    /// Datums whose output this capability audits/verifies.
    pub audits: Option<Vec<String>>,
    /// Datums (or approaches) this capability makes obsolete.
    pub supersedes: Option<Vec<String>>,
    /// Measured evidence for the composition (e.g. token-savings metrics).
    pub measured: Option<Vec<MeasuredMetric>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct BootDatum {
    pub name: String,
    #[serde(rename = "type", deserialize_with = "deserialize_datum_type")]
    pub datum_type: Option<DatumType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_msg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replacement: Option<String>,
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub git_attributes: std::collections::HashMap<String, String>,
    pub desires: Option<String>,
    #[serde(default)]
    pub auto_install: Option<bool>,
    pub hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance: Option<Vec<String>>,

    pub install: Option<InstallSpec>,
    pub update: Option<String>,
    pub version: Option<String>,
    pub version_regex: Option<String>,
    #[serde(default)]
    pub requires_sudo: bool,

    // MCP server fields
    pub command: Option<String>,
    pub args: Option<Vec<String>>,

    // VSCode extension fields
    pub vsix_id: Option<String>,

    // Bash script fields
    pub script: Option<String>,

    // Docker fields
    pub image: Option<String>,
    pub docker_args: Option<Vec<String>>,
    pub oci_uri: Option<String>,
    pub resource_path: Option<String>, // Path to Dockerfile/compose relative to _b00t_/

    // K8s fields
    pub chart_path: Option<String>, // Path to helm chart relative to REPO_ROOT
    pub namespace: Option<String>,
    pub values_file: Option<String>, // Path to values.yaml relative to chart_path

    // Common metadata fields
    pub keywords: Option<Vec<String>>,
    pub package_name: Option<String>,

    // Ansible playbook metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ansible: Option<crate::ansible::AnsibleConfig>,

    // Environment variables
    pub env: Option<std::collections::HashMap<String, String>>,

    // Require constraints
    pub require: Option<Vec<String>>,

    // Aliases for CLI commands
    pub aliases: Option<Vec<String>>,

    // Slash-command orchestration metadata for datum-driven /k0mmand3r dispatch
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k0mmand3r: Option<K0mmand3rDatumConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge: Option<KnowledgeConfig>,

    // MCP-specific multi-method support - these will be handled by datum_mcp module
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpMethods>,

    // Gate preconditions — late-binding conditions evaluated by install pipeline.
    // Each gate is a struct with one or more condition kinds; all must pass.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gate: Option<Vec<GateSpec>>,

    // Source control metadata
    pub url: Option<String>,
    pub branch: Option<String>,
    pub clone_path: Option<String>,

    // Entanglement references
    pub entangled_agents: Option<Vec<String>>,
    pub entangled_cli: Option<Vec<String>>,
    pub entangled_mcp: Option<Vec<String>>,
    pub entangled_ai_models: Option<Vec<String>>,
    pub entangled_apis: Option<Vec<String>>,
    pub entangled_docker: Option<Vec<String>>,
    pub entangled_k8s: Option<Vec<String>>,
    // Role-based channel prefix for Redis pub/sub delegation
    pub channel_prefix: Option<String>,

    // Dependency graph
    pub depends_on: Option<Vec<String>>,
    pub members: Option<Vec<String>>,

    // Classifier hints for orphan matching
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_words: Option<Vec<String>>,

    // Orchestration / stack / job / skill metadata
    pub orchestration: Option<OrchestrationConfig>,
    pub stack: Option<serde_json::Value>,

    // Model cache guard — disk-space gate + usage tracking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_hf_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_size_gb: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_size_4bit_gb: Option<f64>,
    pub job: Option<serde_json::Value>,
    pub skill: Option<serde_json::Value>,

    // Database connection
    pub dsn: Option<String>,

    // Justfile datum configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub justfile: Option<JustfileConfig>,

    // Pipeline datum configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline: Option<PipelineConfig>,

    // RAG / learn metadata
    pub learn: Option<LearnMeta>,
    pub lfmf_category: Option<String>,
    pub usage: Option<Vec<UsageExample>>,

    // API metadata
    pub provides: Option<ApiProvides>,
    pub protocol: Option<String>,
    pub implements: Option<Vec<String>>,

    // Rhai hook scripts — run at specific lifecycle points
    // 🤓 hook_detect:  runs before version detection; return "ok" | "warn: <msg>" | "redirect:<datum>"
    // 🤓 hook_install: runs before install; can abort/redirect (e.g. terraform→opentofu)
    // 🤓 hook_update:  runs before update; same protocol as hook_detect
    // 🤓 hook_learn:   runs during `b00t learn <topic>`; return value appended to learn output
    pub hook_detect: Option<String>,
    pub hook_install: Option<String>,
    pub hook_update: Option<String>,
    pub hook_learn: Option<String>,
    // Uninstall lifecycle
    // 🤓 uninstall: shell script executed by duct::cmd("bash", "-c", script) — same executor as install
    // 🤓 hook_uninstall: POST-hook (runs AFTER uninstall script, unlike other hooks which are pre-hooks);
    //    Rhai script errors are surfaced as Warn("hook script error: ...") by run_hook() and treated as fatal by uninstall_datum()
    //    All other HookResult variants (non-error Warn/Redirect/Info/Missing) are non-fatal: logged and execution continues
    pub uninstall: Option<String>,
    pub hook_uninstall: Option<String>,

    // Blessing system: tool authorization
    // 🤓 unlocks: tool globs this datum authorizes when learned (e.g. ["cargo.*", "rustfmt"])
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocks: Option<Vec<String>>,
    // 🤓 type_tags: content classification (transferable, domain, etc.) — distinct from datum_type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_tags: Option<Vec<String>>,
}

/// Handle datum types that are marked as *incubating*.
///
/// Currently these types are treated as untyped content‑tags, but the
/// function provides a single place to add bespoke logic when a concrete
/// implementation becomes available.
fn handle_incubating_type(value: &str) -> Option<DatumType> {
    // Placeholder – return None to keep existing behaviour.
    // When a real implementation is added, replace this stub with the
    // appropriate mapping or side‑effects.
    let _ = value; // silence unused‑variable warning.
    None
}

fn deserialize_datum_type<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<DatumType>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    match raw {
        None => Ok(None),
        Some(value) if value == "model" => Ok(Some(DatumType::Ai)),
        Some(value) => {
            // Try hard-coded types first, then incubating placeholder.
            let resolved = DatumType::from_type_token(&value)
                .or_else(|| handle_incubating_type(&value));

            if resolved.is_none()
                && !is_known_content_tag(&value)
                && !get_incubating_set().contains(&value)
                && std::env::var("B00T_DATUM_WARN")
                    .map(|v| v != "0")
                    .unwrap_or(true)
            {
                let warned = DATUM_TYPE_WARNED
                    .get_or_init(|| std::sync::Mutex::new(std::collections::HashSet::new()));
                if let Ok(mut set) = warned.lock() {
                    if set.insert(value.clone()) {
                        // warn once per unknown type string per process
                        eprintln!(
                            "⚠️  b00t: unknown datum type token '{value}' — not a typed datum or known content-tag; silence: B00T_DATUM_WARN=0"
                        );

                    }
                }
            }

            Ok(resolved)
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct McpMethods {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdio: Option<Vec<std::collections::HashMap<String, serde_json::Value>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub httpstream: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// A single gate precondition — late-binding condition evaluated at install time.
/// All fields are optional; any present field must pass for the gate to open.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct GateSpec {
    /// Command that must exist on PATH for this gate to pass
    pub command: Option<String>,
    /// File path (supports ~) that must exist
    pub file: Option<String>,
    /// Environment variable (or .env key) that must be set to a non-empty value
    pub env: Option<String>,
    /// Rhai expression to evaluate; must return true for gate to pass
    /// Available vars: name, datum_type, path
    pub rhai: Option<String>,
    /// Knowledge backend that must match the compiled b00t-c0re-lib backend
    pub knowledge_backend: Option<String>,
    /// Freeform description shown when gate fails
    pub hint: Option<String>,
}

/// Result of evaluating a single gate precondition.
#[derive(Debug, Clone, PartialEq)]
pub struct GateResult {
    pub passed: bool,
    pub reason: String,
}

/// Evaluate all gates for a datum. Returns a Vec of GateResults, one per gate.
/// If any gate fails, the datum should be skipped.
pub fn evaluate_gates(gates: &[GateSpec], path: &str) -> Vec<GateResult> {
    let mut results = Vec::new();
    for gate in gates {
        let mut passed = true;
        let mut reasons = Vec::new();

        // Command gate: check if command exists on PATH
        if let Some(ref cmd) = gate.command {
            if !check_command_available(cmd) {
                passed = false;
                reasons.push(format!("command '{}' not found on PATH", cmd));
            }
        }

        // File gate: check if file exists (supports ~ expansion and relative paths)
        if let Some(ref file) = gate.file {
            let expanded = shellexpand::tilde(file).to_string();
            let exists = if std::path::Path::new(&expanded).is_absolute() {
                std::path::Path::new(&expanded).exists()
            } else {
                // Try relative to datum directory (path may be a file; use parent if so).
                // Fall back to current working directory.
                let base = {
                    let p = std::path::Path::new(path);
                    if p.is_dir() { p.to_path_buf() } else { p.parent().map(|q| q.to_path_buf()).unwrap_or_else(|| std::path::PathBuf::from(".")) }
                };
                base.join(&expanded).exists()
                    || std::path::Path::new(&expanded).exists()
            };
            if !exists {
                passed = false;
                reasons.push(format!("file '{}' does not exist", file));
            }
        }

        // Env gate: check if env var or .env entry is set
        if let Some(ref env_var) = gate.env {
            let direct = std::env::var(env_var);
            let env_ok = direct.is_ok() && !direct.unwrap_or_default().is_empty();
            if !env_ok {
                // Fallback: check .env at WORKSPACE_ROOT
                let ws = std::env::var("WORKSPACE_ROOT")
                    .or_else(|_| std::env::var("HOME"))
                    .unwrap_or_default();
                let env_path = std::path::Path::new(&ws).join(".env");
                let env_file_ok = if env_path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&env_path) {
                        let prefix = format!("{}=", env_var);
                        content.lines().any(|line| {
                            let trimmed = line.trim();
                            trimmed.starts_with(&prefix)
                                && !trimmed[prefix.len()..].trim().is_empty()
                                && !trimmed[prefix.len()..].trim().starts_with('#')
                        })
                    } else {
                        false
                    }
                } else {
                    false
                };
                if !env_file_ok {
                    passed = false;
                    reasons.push(format!("env var '{}' not set", env_var));
                }
            }
        }

        // Rhai gate: evaluate rhai expression
        if let Some(ref rhai_expr) = gate.rhai {
            let (rhai_ok, rhai_err) = evaluate_rhai_gate(rhai_expr);
            if !rhai_ok {
                passed = false;
                let reason = if let Some(err) = rhai_err {
                    format!("rhai gate '{rhai_expr}' failed: {err}")
                } else {
                    format!("rhai gate '{rhai_expr}' returned false")
                };
                reasons.push(reason);
            }
        }

        if let Some(ref backend) = gate.knowledge_backend {
            if b00t_c0re_lib::compiled_knowledge_backend() != backend {
                passed = false;
                reasons.push(format!(
                    "knowledge backend '{}' does not match compiled backend '{}'",
                    backend,
                    b00t_c0re_lib::compiled_knowledge_backend()
                ));
            }
        }

        let reason = if reasons.is_empty() {
            gate.hint.clone().unwrap_or_else(|| "gate passed".to_string())
        } else {
            gate.hint.clone().map(|h| format!("{}: {}", h, reasons.join("; ")))
                .unwrap_or_else(|| reasons.join("; "))
        };

        results.push(GateResult { passed, reason });
    }
    results
}

/// Evaluate a simple rhai boolean expression.
fn evaluate_rhai_gate(expr: &str) -> (bool, Option<String>) {
    use rhai::Engine;
    let engine = Engine::new();
    match engine.eval::<bool>(expr) {
        Ok(true) => (true, None),
        Ok(false) => (false, None),
        Err(e) => (false, Some(e.to_string())),
    }
}

/// A single gate with its origin (explicit or auto-derived)
#[derive(Debug, Clone, Serialize)]
pub struct GateReport {
    pub datum: String,
    pub kind: String,
    pub spec: String,
    pub origin: String, // "explicit" or "auto:requires" or "auto:env"
    pub hint: Option<String>,
    /// "pass" | "fail" | "unknown" — populated at scan time
    pub status: &'static str,
}

/// Expand a leading `~/` in a path using the HOME env var.
fn expand_tilde_path(spec: &str) -> std::path::PathBuf {
    if spec.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_default();
        std::path::Path::new(&home)
            .join(spec.strip_prefix("~/").unwrap_or(spec))
    } else {
        std::path::Path::new(spec).to_path_buf()
    }
}

/// Returns "pass", "fail", or "unknown" for a gate condition checked at scan time.
pub fn eval_gate_status(kind: &str, spec: &str) -> &'static str {
    match kind {
        "command" => {
            if check_command_available(spec) { "pass" } else { "fail" }
        }
        "env" => {
            if std::env::var(spec).ok().map_or(false, |v| !v.is_empty()) {
                return "pass";
            }
            // check .env in workspace root
            let ws = std::env::var("WORKSPACE_ROOT")
                .or_else(|_| std::env::var("HOME"))
                .unwrap_or_default();
            let env_path = std::path::Path::new(&ws).join(".env");
            if env_path.exists() {
                if let Ok(content) = std::fs::read_to_string(env_path) {
                    let prefix = format!("{}=", spec);
                    for line in content.lines() {
                        let trimmed = line.trim();
                        if let Some(rest) = trimmed.strip_prefix(prefix.as_str()) {
                            let val = rest.trim();
                            if !val.is_empty() && !val.starts_with('#') {
                                return "pass";
                            }
                        }
                    }
                }
            }
            "fail"
        }
        "file" => {
            if expand_tilde_path(spec).exists() { "pass" } else { "fail" }
        }
        "knowledge_backend" => {
            if b00t_c0re_lib::compiled_knowledge_backend() == spec { "pass" } else { "fail" }
        }
        "rhai" => "unknown",
        _ => "unknown",
    }
}

/// Scan datum files in `path` (.mcp.toml, .mcp.tomllm, .mcp.tomllmd),
/// extract explicit [[b00t.gate]] declarations and auto-derived gates,
/// and evaluate their current status.
pub fn list_gates(path: &str, search: Option<&str>) -> Result<Vec<GateReport>> {
    let expanded = get_expanded_path(path)?;
    let mut gates = Vec::new();

    for entry in std::fs::read_dir(&expanded)
        .map_err(|e| anyhow::anyhow!("Error reading {}: {}", expanded.display(), e))?
    {
        let entry = entry?;
        let fpath = entry.path();
        let fname = match fpath.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Accept .mcp.toml, .mcp.tomllm, .mcp.tomllmd
        let is_mcp_datum = fname.ends_with(".mcp.toml")
            || fname.ends_with(".mcp.tomllm")
            || fname.ends_with(".mcp.tomllmd");
        if !is_mcp_datum {
            continue;
        }

        let name = fname
            .trim_end_matches(".tomllmd")
            .trim_end_matches(".tomllm")
            .trim_end_matches(".toml")
            .trim_end_matches(".mcp")
            .to_string();
        if name.is_empty() {
            continue;
        }

        let content = std::fs::read_to_string(&fpath)?;
        let config: Result<UnifiedConfig, _> = toml::from_str(&content);
        let datum = match config {
            Ok(c) => c.b00t,
            Err(_) => continue,
        };

        // apply search filter
        if let Some(q) = search {
            if !name.to_lowercase().contains(&q.to_lowercase())
                && !datum.hint.to_lowercase().contains(&q.to_lowercase())
            {
                continue;
            }
        }

        let mut push_gate = |kind: &str, spec: &str, origin: &str, hint: Option<String>| {
            gates.push(GateReport {
                datum: name.clone(),
                kind: kind.to_string(),
                spec: spec.to_string(),
                origin: origin.to_string(),
                hint,
                status: eval_gate_status(kind, spec),
            });
        };

        // explicit gates from [[b00t.gate]]
        if let Some(explicit) = &datum.gate {
            for g in explicit {
                if let Some(cmd) = &g.command {
                    push_gate("command", cmd, "explicit", g.hint.clone());
                }
                if let Some(f) = &g.file {
                    push_gate("file", f, "explicit", g.hint.clone());
                }
                if let Some(e) = &g.env {
                    push_gate("env", e, "explicit", g.hint.clone());
                }
                if let Some(r) = &g.rhai {
                    push_gate("rhai", r, "explicit", g.hint.clone());
                }
                if let Some(backend) = &g.knowledge_backend {
                    push_gate("knowledge_backend", backend, "explicit", g.hint.clone());
                }
            }
        }

        if let Some(knowledge) = &datum.knowledge {
            if let Some(backend) = &knowledge.backend {
                push_gate(
                    "knowledge_backend",
                    backend,
                    "auto:knowledge",
                    Some("datum knowledge backend must match compiled b00t-c0re-lib backend".to_string()),
                );
            }
        }

        // auto-derived from requires
        if let Some(req) = &datum.require {
            for r in req {
                if r != "internet" {
                    push_gate("command", r, "auto:requires", None);
                }
            }
        }

        // auto-derived from top-level env
        if let Some(env_map) = &datum.env {
            for (k, _) in env_map {
                if !k.starts_with("LOG_") && !k.starts_with("FAST") {
                    push_gate("env", k, "auto:env", None);
                }
            }
        }
    }

    Ok(gates)
}

/// Sandbox capabilities declared by a justfile datum.
/// These are tested contracts, not access-control requests.
/// An eBPF sandbox uses these to shape the agent's filesystem view —
/// undeclared paths are absent, not denied.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct JustfileCapabilities {
    /// Whether network egress is permitted during recipe execution
    pub network: Option<bool>,
    /// Filesystem paths visible to the executing agent (globs supported)
    pub filesystem: Option<Vec<String>>,
    /// Environment variable patterns the recipe may read (globs supported)
    pub env_vars: Option<Vec<String>>,
    /// Secret names that must be injected by the sandbox, never logged
    pub secrets: Option<Vec<String>>,
}

/// Justfile datum configuration — declares recipes, sandbox, and executor metadata.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct JustfileConfig {
    /// Path to the justfile, relative to the project root
    pub path: Option<String>,
    /// MCP server datum name that introspects this justfile (e.g. "just-mcp")
    pub mcp_server: Option<String>,
    /// Role pattern that selects this justfile — role-aware path resolution.
    /// Simple exact match or `*` wildcard for all roles. When set, this justfile
    /// is only returned by `justfile_for_role()` when the role name matches.
    pub role_pattern: Option<String>,
    /// Logical recipe groups for agent navigation (e.g. ["dev", "ci", "ml"])
    pub recipe_groups: Option<Vec<String>>,
    /// Execution context: "local" | "container" | "wasm" (backward compat — single preferred sandbox)
    pub sandbox: Option<String>,
    /// Ordered subset of sandbox kinds this justfile is compatible with (e.g. ["none", "ebpf"])
    pub allowed_sandboxes: Option<Vec<String>>,
    /// Whether run_recipe has side effects (default: true — conservative)
    pub allow_side_effects: Option<bool>,
    /// eBPF-scoped capabilities — tested contract, not a request
    pub capabilities: Option<JustfileCapabilities>,
}

/// Sandbox capabilities declared by a pipeline datum.
/// Same contract semantics as `JustfileCapabilities` — tested, not requested.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct PipelineCapabilities {
    /// Whether network egress is permitted while dispatching pipeline stages
    pub network: Option<bool>,
    /// Filesystem paths visible while dispatching pipeline stages (globs supported)
    pub filesystem: Option<Vec<String>>,
    /// Environment variable patterns readable during dispatch (globs supported)
    pub env_vars: Option<Vec<String>>,
    /// Secret names that must be injected by the sandbox, never logged
    pub secrets: Option<Vec<String>>,
}

/// Pipeline datum configuration — declares stages and executor metadata for a
/// `*.pipeline.tomllm` multi-stage pipeline.
/// 🤓 Mirrors `JustfileConfig`'s shape. Divergence: `stages` replaces
///    `recipe_groups`/`role_pattern` (a pipeline's "recipes" are its ordered
///    stages, not role-filtered justfile groups) and there is no
///    `allow_side_effects` toggle — dispatching a pipeline stage is always
///    declared as a side effect (see `datum_pipeline.rs`), the way `job`/`gate`
///    datums don't expose a toggle either. Full stage contracts (input/output/
///    depends_on) are modeled by the consuming application, not here — see
///    PIPELINE-DATUM-UFO-FOUNDATION.tomllmd.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct PipelineConfig {
    /// Path to the pipeline definition file, relative to project root.
    /// Defaults to the discovered `*.pipeline.tomllm` datum file itself —
    /// unlike a justfile, a pipeline has no separate externally-executable
    /// artifact at this layer.
    pub path: Option<String>,
    /// MCP server (or job-dispatch surface) datum name that runs this pipeline's
    /// stages (e.g. "job-mcp"). Real dispatch is wired in a later task.
    pub mcp_server: Option<String>,
    /// Ordered stage names. No per-stage contract (input/output/depends_on) at
    /// this layer — see `PIPELINE-DATUM-UFO-FOUNDATION.tomllmd` for the full
    /// stage-contract design owned by the consuming application.
    pub stages: Option<Vec<String>>,
    /// Execution context: "local" | "container" | "wasm" (single preferred sandbox)
    pub sandbox: Option<String>,
    /// Ordered subset of sandbox kinds this pipeline is compatible with
    pub allowed_sandboxes: Option<Vec<String>>,
    /// eBPF-scoped capabilities — tested contract, not a request
    pub capabilities: Option<PipelineCapabilities>,
}

// DatumType — b00t's typed datum registry.
// 🤓 single source of truth: add new variants ONLY here. The macro below derives:
//    from_type_token, base_suffix, all_base_suffixes, from_filename, extension_for_type.
//    DO NOT add manual match arms elsewhere — use the generated methods.
//
// 🎨 Display classification: each variant belongs to one SemanticClass.
//    Shape, color, icon are derived from the class — NO per-variant hardcoding.
//    New variants: just pick a class.  Runtime registration: toggle tracing per class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DatumType {
    Database,
    HiveProfile,
    Agent,
    Config,
    Docker,
    Skill,
    Stack,
    Repo,
    Role,
    Bash,
    Vscode,
    K8s,
    Apt,
    Nix,
    Mcp,
    Cli,
    Api,
    Job,
    Ai,
    Justfile,
    Pipeline,
    Hardware,
    Overlay,
    Runtime,
    Polyseme,
    Credential,
    Gate,
    Hook,
    McpServer,
    Plan,
    Schema,
    Training,
    Vendor,
    Ooda,
    Unknown,
}

// ── Semantic classification — the display derives from what a type IS ─────
// 🤓 Single reasonable default per class.  New variants: just pick a class.
//    Runtime registration: SemanticClass::tracing_enabled() per class.

/// Semantic taxonomy for datum types.  Shape/color/icon are derived from class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SemanticClass {
    /// Infrastructure — nodes that provision, host, or configure systems
    Infra,
    /// Agent — autonomous actors, roles, models, learning
    Agent,
    /// Protocol — MCP servers, APIs, schemas, wire formats
    Protocol,
    /// Skill — executable capabilities, jobs, hooks, gates
    Skill,
    /// Tool — CLI tools, configs, build scripts, plans, vendored deps
    Tool,
    /// Repo — source trees, workspaces, editor configs, packages
    Repo,
    /// Data — databases, store profiles
    Data,
    /// Secret — encrypted credentials, polyseme black boxes
    Secret,
    /// Fallback for unclassified or incubating types
    Unknown,
}

// ── SVG shape templates ────────────────────────────────────────────────────
const SVG_CIRCLE: &str    = "<circle r='24' cx='28' cy='28' />";
const SVG_RECTANGLE: &str = "<rect x='4' y='4' width='48' height='48' rx='8' />";
const SVG_DIAMOND: &str   = "<polygon points='28,4 52,28 28,52 4,28' />";
const SVG_HEXAGON: &str   = "<polygon points='28,4 48,16 48,40 28,52 8,40 8,16' />";
const SVG_TRIANGLE: &str  = "<polygon points='28,6 50,48 6,48' />";
const SVG_VEE: &str       = "<polygon points='4,4 28,52 52,4' />";

impl SemanticClass {
    /// Shape for graph/chart rendering.
    pub const fn shape(&self) -> &'static str {
        match self {
            Self::Infra => "hexagon",
            Self::Agent => "circle",
            Self::Protocol => "diamond",
            Self::Skill => "triangle",
            Self::Tool => "rectangle",
            Self::Repo => "vee",
            Self::Data => "rectangle",
            Self::Secret => "circle",
            Self::Unknown => "rectangle",
        }
    }

    /// Fill color (hex).
    pub const fn color(&self) -> &'static str {
        match self {
            Self::Infra => "#326ce5",
            Self::Agent => "#059669",
            Self::Protocol => "#7c3aed",
            Self::Skill => "#d97706",
            Self::Tool => "#0d9488",
            Self::Repo => "#be123c",
            Self::Data => "#475569",
            Self::Secret => "#1e293b",
            Self::Unknown => "#1e293b",
        }
    }

    /// Border/stroke color.
    pub const fn border_color(&self) -> &'static str {
        match self {
            Self::Infra => "#5b9cf5",
            Self::Agent => "#34d399",
            Self::Protocol => "#a78bfa",
            Self::Skill => "#fbbf24",
            Self::Tool => "#2dd4bf",
            Self::Repo => "#fb7185",
            Self::Data => "#94a3b8",
            Self::Secret => "#475569",
            Self::Unknown => "#475569",
        }
    }

    /// Emoji or unicode icon for compact rendering.
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Infra => "☸",
            Self::Agent => "🤖",
            Self::Protocol => "🔌",
            Self::Skill => "🛠️",
            Self::Tool => "⌨️",
            Self::Repo => "📁",
            Self::Data => "🗄️",
            Self::Secret => "🔐",
            Self::Unknown => "❓",
        }
    }

    /// SVG shape template fragment.
    pub const fn svg_template(&self) -> &'static str {
        match self {
            Self::Infra => SVG_HEXAGON,
            Self::Agent => SVG_CIRCLE,
            Self::Protocol => SVG_DIAMOND,
            Self::Skill => SVG_TRIANGLE,
            Self::Tool => SVG_RECTANGLE,
            Self::Repo => SVG_VEE,
            Self::Data => SVG_RECTANGLE,
            Self::Secret => SVG_CIRCLE,
            Self::Unknown => SVG_RECTANGLE,
        }
    }

    /// CSS class for styling hooks.
    pub const fn css_class(&self) -> &'static str {
        match self {
            Self::Infra => "sc-infra",
            Self::Agent => "sc-agent",
            Self::Protocol => "sc-protocol",
            Self::Skill => "sc-skill",
            Self::Tool => "sc-tool",
            Self::Repo => "sc-repo",
            Self::Data => "sc-data",
            Self::Secret => "sc-secret",
            Self::Unknown => "sc-unknown",
        }
    }
}

impl DatumType {
    /// Classify this datum type into its semantic class.
    /// 🎨 This is the ONLY place variant→class mapping lives.
    ///    New variants: add one line here. No other code changes needed.
    pub const fn semantic_class(&self) -> SemanticClass {
        match self {
            Self::K8s | Self::Docker | Self::Hardware | Self::Overlay
            | Self::Runtime | Self::Nix => SemanticClass::Infra,
            Self::Agent | Self::Role | Self::Ai | Self::Training => SemanticClass::Agent,
            Self::Mcp | Self::McpServer | Self::Api | Self::Schema => SemanticClass::Protocol,
            Self::Skill | Self::Job | Self::Hook | Self::Gate | Self::Pipeline => SemanticClass::Skill,
            Self::Config | Self::Bash | Self::Cli | Self::Justfile
            | Self::Plan | Self::Vendor | Self::Ooda => SemanticClass::Tool,
            Self::Stack | Self::Repo | Self::Vscode | Self::Apt => SemanticClass::Repo,
            Self::Database | Self::HiveProfile => SemanticClass::Data,
            Self::Polyseme | Self::Credential => SemanticClass::Secret,
            Self::Unknown => SemanticClass::Unknown,
        }
    }

    /// Stereotype hierarchy: which types does this type imply?
    /// e.g. McpServer implies Mcp (server → protocol), Runtime implies Cli (can run → can check)
    pub const fn implies(&self) -> &'static [DatumType] {
        match self {
            Self::McpServer => &[Self::Mcp],
            Self::Runtime   => &[Self::Cli],
            Self::Agent     => &[Self::Runtime],
            Self::Ai        => &[Self::Agent],
            Self::Role      => &[Self::Agent],
            _ => &[],
        }
    }

    /// Is this type implied by (i.e., less specific than) another?
    pub fn is_implied_by(&self, other: &DatumType) -> bool {
        other.implies().contains(self)
    }

    // 🎨 display() defined below with DatumDisplay struct
}

macro_rules! datum_type_table {
    ($($variant:ident => [$($token:literal),+] => $suffix:literal),* $(,)?) => {
        /// Map a TOML type token string to a DatumType variant; returns None for unknown tokens.
        pub fn from_type_token(s: &str) -> Option<Self> {
            match s {
                $($($token => Some(Self::$variant),)+)*
                _ => None,
            }
        }

        /// File suffix for this datum type (e.g., ".cli", ".mcp").
        pub fn base_suffix(&self) -> &'static str {
            match self {
                $(Self::$variant => $suffix,)*
                Self::Unknown => ".toml",
            }
        }

        /// All known base suffixes (excluding Unknown).
        pub fn all_base_suffixes() -> Vec<&'static str> {
            vec![$($suffix,)*]
        }

        /// All non-Unknown variants as a const slice.
        pub fn all_variants() -> &'static [Self] {
            &[$(Self::$variant,)*]
        }

        /// Determine DatumType from a filename (e.g. "mold.cli.toml" → Cli).
        /// 🤓 legacy .ai_model.toml suffix handled explicitly — model is sub-class of Ai.
        pub fn from_filename(filename: &str) -> Self {
            for t in Self::all_variants() {
                let base = t.base_suffix();
                if filename.ends_with(base)
                    || filename.ends_with(&format!("{base}.toml"))
                    || filename.ends_with(&format!("{base}.tomllmd"))
                    || filename.ends_with(&format!("{base}.tomllm"))
                {
                    return *t;
                }
            }
            // Legacy: .ai_model.toml / .ai_model.tomllmd / .ai_model.tomllm → Ai
            if filename.ends_with(".ai_model.toml")
                || filename.ends_with(".ai_model.tomllmd")
                || filename.ends_with(".ai_model.tomllm")
                || filename.ends_with(".ai_model")
            {
                return Self::Ai;
            }
            Self::Unknown
        }

        /// Preferred file extension for this type (e.g., ".cli.toml").
        pub fn extension(&self) -> &'static str {
            match self {
                $(Self::$variant => concat!($suffix, ".toml"),)*
                Self::Unknown => ".toml",
            }
        }
    };
}

impl DatumType {
    datum_type_table! {
        Database    => ["database", "db"]             => ".database",
        HiveProfile => ["hive", "hive_profile"]      => ".hive",
        Agent       => ["agent"]                     => ".agent",
        Config      => ["config"]                    => ".config",
        Docker      => ["docker"]                    => ".docker",
        Skill       => ["skill"]                     => ".skill",
        Stack       => ["stack"]                     => ".stack",
        Repo        => ["repo"]                      => ".repo",
        Role        => ["role"]                      => ".role",
        Bash        => ["bash"]                      => ".bash",
        Vscode      => ["vscode"]                    => ".vscode",
        K8s         => ["k8s"]                       => ".k8s",
        Apt         => ["apt"]                       => ".apt",
        Nix         => ["nix"]                       => ".nix",
        Mcp         => ["mcp"]                       => ".mcp",
        Cli         => ["cli"]                       => ".cli",
        Api         => ["api"]                       => ".api",
        Job         => ["job"]                       => ".job",
        // Ai is the umbrella; model/ai_model tokens map here (reverse dot: name.model.ai.tomllmd)
        Ai          => ["ai", "model", "ai_model"]   => ".ai",
        Justfile    => ["justfile"]                  => ".justfile",
        Pipeline    => ["pipeline"]                  => ".pipeline",
        Hardware    => ["hardware"]                  => ".hardware",
        Overlay     => ["overlay"]                   => ".overlay",
        Runtime     => ["runtime", "wrap", "launcher"] => ".runtime",
        Polyseme    => ["polyseme", "poly"]            => ".polyseme",
        Credential  => ["credential", "credentials"]  => ".credential",
        Gate        => ["gate"]                      => ".gate",
        Hook        => ["hook"]                      => ".hook",
        McpServer   => ["mcp_server"]                => ".mcp_server",
        Plan        => ["plan"]                      => ".plan",
        Schema      => ["schema"]                    => ".schema",
        Training    => ["training"]                  => ".training",
        Vendor      => ["vendor"]                    => ".vendor",
        Ooda        => ["ooda"]                      => ".ooda",
    }

    /// Preferred file extension for writing new datum files.
    /// Defaults to `{base_suffix}.toml`; special-cases Role (bare .toml),
    /// Justfile (bare .justfile).
    /// 🤓 Model datums use reverse-dot: <name>.model.ai.tomllmd
    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Role => ".toml",
            Self::Justfile => ".justfile",
            Self::Unknown => ".toml",
            other => other.extension(),
        }
    }
}

impl DatumType {
    /// mti TypeID prefix inferred from base_suffix() — e.g. ".skill" → "skill", ".mcp" → "mcp".
    /// No manual mapping needed; stays in sync with datum_type_table! automatically.
    pub fn type_prefix(&self) -> &'static str {
        self.base_suffix().trim_start_matches('.')
    }

    /// Type-graph nodes for all DatumType variants, auto-derived from datum_type_table!.
    /// Zero-maintenance: adding a new variant automatically appears here.
    pub fn datum_nodes() -> Vec<b00t_reflect_types::HolonNode> {
        Self::all_variants()
            .iter()
            .map(|v| b00t_reflect_types::HolonNode {
                id: format!("datum_type::{}", v.type_prefix()),
                label: v.type_prefix().to_string(),
                kind: "datum_type".to_string(),
                z_layer: None,
                semantic_type: None,
            })
            .collect()
    }
}

impl std::fmt::Display for DatumType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.base_suffix())
    }
}

/// Visual display descriptor — derived from [SemanticClass] at compile time.
/// 🤓 Rust types own their display.  Subsystems with code provide CSS animations
///    via css_class.  No TOML overrides — code is the single source of truth.
#[derive(Serialize, Debug, Clone)]
pub struct DatumDisplay {
    pub shape: String,
    pub color: String,
    pub border_color: String,
    pub icon: String,
    pub css_class: String,
    pub svg: String,
}

impl DatumType {
    /// Display descriptor derived from semantic class.
    pub fn display(&self) -> DatumDisplay {
        let sc = self.semantic_class();
        DatumDisplay {
            shape: sc.shape().into(),
            color: sc.color().into(),
            border_color: sc.border_color().into(),
            icon: sc.icon().into(),
            css_class: sc.css_class().into(),
            svg: sc.svg_template().into(),
        }
    }
}

impl DatumDisplay {
    pub fn to_cytoscape_style(&self) -> serde_json::Value {
        serde_json::json!({
            "shape": self.shape,
            "background-color": self.color,
            "border-color": self.border_color,
            "icon": self.icon,
            "css_class": self.css_class,
        })
    }
}

#[derive(Serialize, Debug)]
pub struct McpListOutput {
    pub servers: Vec<McpListItem>,
    pub path: String,
    pub truncated: bool,
    pub threshold: i64,
    pub total_count: usize,
}

#[derive(Serialize, Debug)]
pub struct McpListItem {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub hint: Option<String>,
    pub error: Option<String>,
    pub is_installed: bool,
    pub is_running: bool,
    pub is_suspended: bool,
    pub transport: Option<String>,
    pub restart_hint: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct McpListFilter {
    pub search: Option<String>,
    pub is_installed: Option<bool>,
    pub is_running: Option<bool>,
    pub is_suspended: Option<bool>,
    pub max_threshold: Option<i64>,
    pub bypass_threshold: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct AiConfig {
    pub b00t: BootDatum,
    pub models: Option<std::collections::HashMap<String, serde_json::Value>>,
    pub env: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize, Debug)]
pub struct AiListOutput {
    pub providers: Vec<AiListItem>,
    pub path: String,
}

#[derive(Serialize, Debug)]
pub struct AiListItem {
    pub name: String,
    pub models: Option<Vec<String>>,
    pub env_keys: Option<Vec<String>>,
    pub error: Option<String>,
}

// Session tracking structures
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SessionState {
    pub session_id: String,
    pub start_time: DateTime<Utc>,
    pub commands_run: u32,
    pub estimated_cost: f64,
    pub budget_limit: Option<f64>,
    pub time_limit_minutes: Option<u32>,
    pub agent_info: Option<AgentInfo>,
    pub hints: Vec<String>,
    pub last_activity: DateTime<Utc>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub model_size: Option<String>,
    pub role: Option<String>,
    pub pid: u32,
    pub privacy_level: Option<String>,
}

pub fn extract_comments_and_clean_json(input: &str) -> (String, Option<String>) {
    let comment_re = Regex::new(r"//.*$").unwrap();
    let block_comment_re = Regex::new(r"/\*.*?\*/").unwrap();

    let (mut cleaned_input, mut first_comment) = (String::new(), None);

    // First, remove block comments /* ... */
    let input_without_blocks = block_comment_re.replace_all(input, "").to_string();

    // Then process line comments
    for line in input_without_blocks.lines() {
        if let Some(cap) = comment_re.find(line) {
            if first_comment.is_none() {
                let comment_text = cap.as_str().trim_start_matches("//").trim();
                if !comment_text.is_empty() {
                    first_comment = Some(comment_text.to_string());
                }
            }
            let line_without_comment = line[..cap.start()].trim_end();
            if !line_without_comment.is_empty() {
                cleaned_input.push_str(line_without_comment);
                cleaned_input.push('\n');
            }
        } else {
            cleaned_input.push_str(line);
            cleaned_input.push('\n');
        }
    }

    // Also handle trailing commas (JSON5 style) - both objects and arrays
    let trailing_comma_re = Regex::new(r",(\s*[}\]])").unwrap();
    cleaned_input = trailing_comma_re
        .replace_all(&cleaned_input, "$1")
        .to_string();

    // Handle trailing commas at end of lines more aggressively
    let lines: Vec<String> = cleaned_input
        .lines()
        .map(|line| {
            let trimmed = line.trim_end();
            if trimmed.ends_with(',')
                && (line.contains('}')
                    || line.contains(']')
                    || cleaned_input
                        .lines()
                        .skip_while(|l| l != &line)
                        .nth(1)
                        .map(|next| next.trim().starts_with('}') || next.trim().starts_with(']'))
                        .unwrap_or(false))
            {
                trimmed.strip_suffix(',').unwrap_or(trimmed).to_string()
            } else {
                line.to_string()
            }
        })
        .collect();
    cleaned_input = lines.join("\n");

    (cleaned_input.trim().to_string(), first_comment)
}

pub fn clean_json_for_dwiw(input: &str) -> String {
    extract_comments_and_clean_json(input).0
}

/// Convert legacy JSON command/args to new multi-method format
fn create_mcp_datum_from_json(
    name: String,
    hint: Option<String>,
    server_config: &serde_json::Value,
) -> BootDatum {
    let command = server_config
        .get("command")
        .and_then(|v| v.as_str())
        .unwrap_or("npx")
        .to_string();
    let args = server_config
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_else(|| vec![]);

    // Detect transport type and requirements based on command
    let (requires, transport_type) = match command.as_str() {
        "docker" => (vec!["docker".to_string()], "stdio"),
        "uvx" | "python" | "python3" => (vec!["python".to_string()], "stdio"),
        "npx" | "node" => (vec!["node".to_string()], "stdio"),
        _ => (vec![], "stdio"),
    };

    let cli_method = serde_json::json!({
        "command": command,
        "args": args,
        "priority": 0,
        "requires": requires,
        "transport": transport_type
    });

    BootDatum { model_hf_id: None, model_size_gb: None, model_size_4bit_gb: None, 
        name,
        datum_type: Some(DatumType::Mcp),
        hint: hint.or_else(|| server_config.get("hint").and_then(|v| v.as_str()).map(|s| s.to_string())).unwrap_or_else(|| "MCP server".to_string()),
        env: server_config
            .get("env")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            }),
        require: server_config
            .get("require")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            }),
        // Convert legacy command/args to new multi-method format
        mcp: Some(McpMethods {
            stdio: Some(vec![
                cli_method
                    .as_object()
                    .unwrap()
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ]),
            httpstream: None,
        }),
        // Parse gates from JSON if present
        gate: server_config
            .get("gate")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|g| {
                    let cmd = g.get("command").and_then(|v| v.as_str());
                    let file = g.get("file").and_then(|v| v.as_str());
                    let env = g.get("env").and_then(|v| v.as_str());
                    let rhai = g.get("rhai").and_then(|v| v.as_str());
                    let knowledge_backend = g.get("knowledge_backend").and_then(|v| v.as_str());
                    let hint = g.get("hint").and_then(|v| v.as_str()).map(|s| s.to_string());
                    if cmd.is_none() && file.is_none() && env.is_none() && rhai.is_none() && knowledge_backend.is_none() {
                        return None;
                    }
                    Some(GateSpec {
                        command: cmd.map(|s| s.to_string()),
                        file: file.map(|s| s.to_string()),
                        env: env.map(|s| s.to_string()),
                        rhai: rhai.map(|s| s.to_string()),
                        knowledge_backend: knowledge_backend.map(|s| s.to_string()),
                        hint,
                    })
                }).collect()
            }),
        ..BootDatum::default()
    }
}

pub fn normalize_mcp_json(input: &str, dwiw: bool) -> Result<BootDatum> {
    let (cleaned_input, hint) = if dwiw {
        extract_comments_and_clean_json(input)
    } else {
        (input.to_string(), None)
    };

    let json_value: serde_json::Value = serde_json::from_str(&cleaned_input)?;

    // 🤓 YET-ANOTHER-STANDARD SYNDROME: AI tooling JSON format chaos
    // Different MCP ecosystems use different JSON formats:
    // 1. Flat format: {"name": "server", "command": "npx", "args": [...]}
    // 2. Nested format: {"server-name": {"command": "npx", "args": [...]}}
    // 3. mcpServers wrapper: {"mcpServers": {"server-name": {...}}}
    // We auto-detect and support all three because... modern AI tooling. 🙄

    // Handle direct format: {"name": "...", "command": "...", "args": [...]} or {"name": "...", "url": "..."}
    if let Some(name) = json_value.get("name") {
        let name_str = name.as_str().unwrap_or("unknown").to_string();

        // Check if this is an HTTP server (has URL field)
        if let Some(url) = json_value.get("url") {
            let http_method = serde_json::json!({
                "url": url.as_str().unwrap_or(""),
                "priority": 0,
                "requires": ["internet"],
                "requires_internet": true,
                "requires_auth": false,
                "transport": "httpstream"
            });

            return Ok(BootDatum { model_hf_id: None, model_size_gb: None, model_size_4bit_gb: None, 
                name: name_str,
                datum_type: Some(DatumType::Mcp),
                hint: hint
                    .clone()
                    .unwrap_or_else(|| "MCP HTTP server".to_string()),
                env: json_value
                    .get("env")
                    .and_then(|v| v.as_object())
                    .map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect()
                    }),
                require: json_value
                    .get("require")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    }),
                mcp: Some(McpMethods {
                    stdio: None,
                    httpstream: Some(
                        http_method
                            .as_object()
                            .unwrap()
                            .iter()
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect(),
                    ),
                }),
                ..BootDatum::default()
            });
        }

        // Otherwise, treat as CLI/stdio server
        return Ok(create_mcp_datum_from_json(
            name_str,
            hint.clone(),
            &json_value,
        ));
    }

    // Handle mcpServers wrapper format: {"mcpServers": {"server_name": {...}}}
    // 🤓 This is the "official" Claude Desktop format
    if let Some(mcp_servers) = json_value.get("mcpServers") {
        let keys: Vec<_> = mcp_servers
            .as_object()
            .map(|obj| obj.keys().collect())
            .unwrap_or_default();

        if keys.len() == 1 {
            let server_name = keys[0].clone();
            let server_config = &mcp_servers[&server_name];
            return Ok(create_mcp_datum_from_json(
                server_name,
                hint.clone(),
                server_config,
            ));
        } else if keys.len() > 1 {
            // Multiple servers in mcpServers - take the first one and warn
            let server_name = keys[0].clone();
            let server_config = &mcp_servers[&server_name];
            eprintln!(
                "⚠️  Multiple servers found in mcpServers, using first: {}",
                server_name
            );
            eprintln!("💡 To register multiple servers, use separate commands for each");
            return Ok(create_mcp_datum_from_json(
                server_name,
                hint.clone(),
                server_config,
            ));
        }
    }

    // Handle single server format: {"server_name": {...}}
    // 🤓 Legacy format from early MCP tools, also used by some test fixtures
    let keys: Vec<_> = json_value
        .as_object()
        .map(|obj| obj.keys().collect())
        .unwrap_or_default();

    if keys.len() == 1 {
        let server_name = keys[0].clone();
        let server_config = &json_value[&server_name];
        return Ok(create_mcp_datum_from_json(server_name, hint, server_config));
    }

    anyhow::bail!("Unable to parse MCP server configuration from JSON");
}

pub fn create_ai_toml_config(ai_config: &AiConfig, path: &str) -> Result<()> {
    let toml_content =
        toml::to_string(ai_config).context("Failed to serialize AI config to TOML")?;

    let mut path_buf = std::path::PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}.ai.toml", ai_config.b00t.name));

    std::fs::write(&path_buf, toml_content).context(format!(
        "Failed to write AI config to {}",
        path_buf.display()
    ))?;

    println!("Created AI config: {}", path_buf.display());
    Ok(())
}

pub fn create_unified_toml_config(datum: &BootDatum, path: &str) -> Result<()> {
    let config = UnifiedConfig {
        b00t: datum.clone(),
        env: None,
        sections: None,
    };

    let toml_content = toml::to_string(&config).context("Failed to serialize config to TOML")?;

    // Use explicit datum_type or default to Unknown
    let datum_type = datum.datum_type.clone().unwrap_or(DatumType::Unknown);
    let suffix = datum_type.file_extension();

    let mut path_buf = std::path::PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());
    path_buf.push(format!("{}{}", datum.name, suffix));

    std::fs::write(&path_buf, toml_content)
        .context(format!("Failed to write config to {}", path_buf.display()))?;

    println!(
        "Created {} config: {}",
        datum_type.to_string(),
        path_buf.display()
    );
    Ok(())
}

impl BootDatum {
    pub fn get_datum_type(&self, filename: Option<&str>) -> DatumType {
        self.datum_type.clone().unwrap_or_else(|| {
            filename
                .map(DatumType::from_filename)
                .unwrap_or(DatumType::Unknown)
        })
    }

    pub fn install_command(&self) -> Option<&str> {
        self.install.as_ref().and_then(InstallSpec::command)
    }

    pub fn install_command_string(&self) -> Option<String> {
        self.install.as_ref().and_then(InstallSpec::command_string)
    }
}

pub fn create_mcp_toml_config(package: &BootDatum, path: &str) -> Result<()> {
    create_unified_toml_config(package, path)
}

pub fn check_command_available(command: &str) -> bool {
    use duct::cmd;
    cmd!("which", command).read().is_ok()
}

pub fn get_expanded_path(path: &str) -> Result<std::path::PathBuf> {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};

    static WARNED_LEGACY: AtomicBool = AtomicBool::new(false);

    let primary = PathBuf::from(shellexpand::tilde(path).to_string());
    if primary.exists() {
        return Ok(primary);
    }

    let legacy = PathBuf::from(shellexpand::tilde("~/.dotfiles/_b00t_").to_string());
    if legacy.exists() {
        if !WARNED_LEGACY.swap(true, Ordering::SeqCst) {
            eprintln!("⚠️ Using legacy b00t path at {}", legacy.display());
        }
        return Ok(legacy);
    }

    Ok(primary)
}

/// Find a project by walking up from cwd to git root looking for .git/🥾.tomllmd
fn find_project_b00t() -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut current = cwd.as_path();
    loop {
        let marker = current.join(".git").join("🥾.tomllmd");
        if marker.exists() {
            return current.join("_b00t_").is_dir().then(|| current.join("_b00t_"));
        }
        if current.join(".git").exists() {
            break;
        }
        current = current.parent()?;
    }
    None
}

/// Load project-local version overrides from .git/🥾.tomllmd
pub fn load_project_overrides() -> std::collections::HashMap<String, String> {
    let mut overrides = std::collections::HashMap::new();
    let path = {
        let cwd = match std::env::current_dir() { Ok(d) => d, Err(_) => return overrides };
        let mut cur = cwd.as_path();
        loop {
            let git_boot = cur.join(".git").join("🥾.tomllmd");
            if git_boot.exists() {
                break Some(git_boot);
            }
            if cur.join(".git").exists() { break None; }
            cur = match cur.parent() { Some(p) => p, None => break None };
        }
    };
    if let Some(path) = path {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(toml) = content.parse::<toml::Table>() {
                if let Some(o) = toml.get("overrides").and_then(|v| v.as_table()) {
                    for (k, v) in o {
                        if let Some(val) = v.as_str() {
                            overrides.insert(k.clone(), val.to_string());
                        }
                    }
                }
            }
        }
    }
    overrides
}

pub fn get_ai_tools_status(path: &str) -> Result<Vec<Box<dyn StatusProvider>>> {
    use crate::datum_ai::AiDatum;
    let mut tools: Vec<Box<dyn StatusProvider>> = Vec::new();
    let expanded_path = get_expanded_path(path)?;

    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(".ai.toml") {
                        if let Some(tool_name) = file_name.strip_suffix(".ai.toml") {
                            if let Ok(datum) = AiDatum::try_from((tool_name, path)) {
                                tools.push(Box::new(datum));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(tools)
}

pub fn get_config(
    command: &str,
    path: &str,
) -> Result<(UnifiedConfig, String), Box<dyn std::error::Error>> {
    // Try project-local _b00t_/ first, then fall back to global path
    let dirs: Vec<std::path::PathBuf> = {
        let mut v = Vec::new();
        if let Some(project) = find_project_b00t() {
            v.push(project);
        }
        if let Ok(expanded) = get_expanded_path(path) {
            v.push(expanded);
        }
        v
    };

    for dir in &dirs {
        for base in DatumType::all_base_suffixes() {
            for ext in [".tomllmd", ".tomllm", ".toml"] {
                let p = dir.join(format!("{}{}{}", command, base, ext));
                if p.exists() {
                    let filename = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    let content = std::fs::read_to_string(&p)?;
                    let mut config: UnifiedConfig = toml::from_str(&content)?;
                    crate::datum_utils::apply_git_attributes_to_config(&mut config, &p);
                    return Ok((config, filename));
                }
            }
        }
        for ext in [".tomllmd", ".tomllm", ".toml"] {
            let plain = dir.join(format!("{}{}", command, ext));
            if plain.exists() {
                let filename = format!("{}{}", command, ext);
                let content = std::fs::read_to_string(&plain)?;
                let mut config: UnifiedConfig = toml::from_str(&content)?;
                crate::datum_utils::apply_git_attributes_to_config(&mut config, &plain);
                return Ok((config, filename));
            }
        }
    }

    Err(format!("{} UNDEFINED", command).into())
}

pub fn get_mcp_config(name: &str, path: &str) -> Result<BootDatum> {
    use anyhow::Context;
    use std::fs;

    let mut path_buf = get_expanded_path(path)?;
    path_buf.push(format!("{}.mcp.toml", name));

    if !path_buf.exists() {
        anyhow::bail!(
            "MCP server '{}' not found. Use 'b00t-cli mcp add' to create it first.",
            name
        );
    }

    let content = fs::read_to_string(&path_buf).context(format!(
        "Failed to read MCP config from {}",
        path_buf.display()
    ))?;

    let mut config: UnifiedConfig =
        toml::from_str(&content).context("Failed to parse MCP config TOML")?;
    crate::datum_utils::apply_git_attributes_to_config(&mut config, &path_buf);

    Ok(config.b00t)
}

/// Load a runtime wrapper datum, returning the parsed [RuntimeConfig].
/// Searches `path` for `<name>.runtime.toml`, `<name>.runtime.tomllmd`, etc.
pub fn load_runtime_datum(name: &str, path: &str) -> Result<RuntimeConfig> {
    use anyhow::Context;
    use std::fs;

    let expanded_path = get_expanded_path(path)?;
    let suffixes = &[".runtime.toml", ".runtime.tomllmd", ".runtime.tomllm"];
    let mut found: Option<std::path::PathBuf> = None;

    for suffix in suffixes {
        let candidate = expanded_path.join(format!("{name}{suffix}"));
        if candidate.exists() {
            found = Some(candidate);
            break;
        }
    }

    let file_path = found.ok_or_else(|| {
        anyhow::anyhow!(
            "runtime datum '{name}' not found (tried {}/{name}.runtime.toml[lmd|lm])",
            expanded_path.display()
        )
    })?;

    let content = fs::read_to_string(&file_path)
        .context(format!("Failed to read runtime config from {}", file_path.display()))?;
    let config: UnifiedConfig =
        toml::from_str(&content).context(format!("Failed to parse {}", file_path.display()))?;

    config
        .b00t
        .runtime
        .ok_or_else(|| anyhow::anyhow!("datum '{}' missing [b00t.runtime] section", name))
}

/// Datum dispatch resolution — what action to take for `b00t <name>`.
#[derive(Clone)]
pub enum DatumDispatch {
    /// Launch via sandbox (runtime datum)
    Runtime(RuntimeConfig),
    /// Execute the datum's command + passthrough args (cli datum)
    CliPassthrough { command: String, args: Vec<String> },
    /// Show polyseme resolution options
    Polyseme { name: String, refs: Vec<crate::PolysemeRef> },
    /// Found but not directly dispatchable (mcp, ai, etc.)
    Info(String),
}

/// Search the datum space for `candidate` and resolve ALL matching dispatch actions.
/// Returns multiple matches when a name is polysemous or has multiple datum types.
pub fn resolve_all_datum_dispatches(candidate: &str, path: &str) -> Vec<DatumDispatch> {
    let mut results = Vec::new();

    let expanded = match get_expanded_path(path) {
        Ok(p) => p,
        Err(_) => return results,
    };

    // Runtime — if a runtime datum exists, it's the primary dispatch.
    // CLI datum is NOT added when runtime exists (avoids disambiguation prompt).
    let mut has_runtime = false;
    let runtime_suffixes = [".runtime.toml", ".runtime.tomllmd", ".runtime.tomllm"];
    for suffix in &runtime_suffixes {
        let p = expanded.join(format!("{candidate}{suffix}"));
        if p.exists() {
            if let Ok(cfg) = load_runtime_datum(candidate, path) {
                results.push(DatumDispatch::Runtime(cfg));
                has_runtime = true;
                break;
            }
        }
    }

    // CLI — only auto-dispatched when NO runtime datum exists.
    // CLI operations (check/install) always go through explicit `b00t cli <cmd>`.
    if !has_runtime {
        let cli_suffixes = [".cli.toml", ".cli.tomllmd", ".cli.tomllm"];
        for suffix in &cli_suffixes {
            let p = expanded.join(format!("{candidate}{suffix}"));
            if p.exists() {
                if let Ok(datum) = load_cli_datum(candidate, path) {
                    let cmd = datum.command.unwrap_or_else(|| candidate.to_string());
                    let args: Vec<String> = datum.args.unwrap_or_default();
                    results.push(DatumDispatch::CliPassthrough {
                        command: cmd,
                        args,
                    });
                    break;
                }
            }
        }
    }

    // Polyseme
    let poly_suffixes = [".polyseme.toml", ".polyseme.tomllmd", ".polyseme.tomllm"];
    for suffix in &poly_suffixes {
        let p = expanded.join(format!("{candidate}{suffix}"));
        if p.exists() {
            if let Ok(refs) = load_polyseme_refs(candidate, path) {
                results.push(DatumDispatch::Polyseme {
                    name: candidate.to_string(),
                    refs,
                });
                break;
            }
        }
    }

    // OODA — execute the observe/orient/decide/act loop
    let ooda_suffixes = [".ooda.toml", ".ooda.tomllmd", ".ooda.tomllm"];
    for suffix in &ooda_suffixes {
        let p = expanded.join(format!("{candidate}{suffix}"));
        if p.exists() {
            results.push(DatumDispatch::Info(format!(
                "ooda loop '{}' — run with: b00t ooda run {}", candidate, candidate
            )));
            break;
        }
    }

    // MCP
    let mcp_suffixes = [".mcp.toml", ".mcp.tomllmd", ".mcp.tomllm"];
    for suffix in &mcp_suffixes {
        let p = expanded.join(format!("{candidate}{suffix}"));
        if p.exists() {
            results.push(DatumDispatch::Info(format!(
                "mcp datum '{}' — use 'b00t mcp list' or 'b00t mcp execute {} <tool>'",
                candidate, candidate
            )));
            break;
        }
    }

    // ── Stereotype hierarchy: eliminate less-specific matches ──────────────
    if results.len() > 1 {
        let mut filtered: Vec<DatumDispatch> = Vec::new();
        for result in &results {
            let is_implied = results.iter().any(|other| {
                std::mem::discriminant(result) != std::mem::discriminant(other)
                    && result_is_implied_by(result, other)
            });
            if !is_implied {
                filtered.push(result.clone());
            }
        }
        if !filtered.is_empty() {
            results = filtered;
        }
    }

    results
}

/// Returns true if `a` is implied by `b` (a is less specific than b).
fn result_is_implied_by(a: &DatumDispatch, b: &DatumDispatch) -> bool {
    use DatumDispatch::*;
    match (a, b) {
        (CliPassthrough { .. }, Runtime(_)) => true,
        (Info(_), _) | (_, Info(_)) => false,
        _ => false,
    }
}

/// Single-match convenience — returns the first runtime or CLI dispatch, or the polyseme if present.
/// Returns None if no datum matches.
pub fn resolve_datum_dispatch(candidate: &str, path: &str) -> Option<DatumDispatch> {
    let mut all = resolve_all_datum_dispatches(candidate, path);
    if all.is_empty() {
        return None;
    }
    // Prefer runtime over CLI over polyseme for single-match
    if let Some(pos) = all.iter().position(|d| matches!(d,
        DatumDispatch::Runtime(_) | DatumDispatch::CliPassthrough { .. } | DatumDispatch::Polyseme { .. }
    )) {
        return Some(all.swap_remove(pos));
    }
    all.into_iter().next()
}

/// Interactive polyseme selection prompt (#580).
/// Returns the selected ref name, or None if non-interactive / cancelled.
pub fn prompt_polyseme_selection(name: &str, refs: &[crate::PolysemeRef]) -> Option<String> {
    use std::io::{self, BufRead, IsTerminal, Write};

    if !io::stdin().is_terminal() {
        return None; // non-interactive — caller handles display + exit
    }

    eprintln!("\n🔀 '{name}' has multiple resolutions:");
    for (i, r) in refs.iter().enumerate() {
        eprintln!("  {}) {} — {}", i + 1, r.name, r.description);
    }
    eprint!("  Select [1-{}]: ", refs.len());
    io::stdout().flush().ok();

    let mut input = String::new();
    io::stdin().lock().read_line(&mut input).ok()?;
    let choice: usize = input.trim().parse().ok()?;
    refs.get(choice.wrapping_sub(1)).map(|r| r.name.clone())
}

/// Load a CLI datum and return its BootDatum.
fn load_cli_datum(name: &str, path: &str) -> Result<BootDatum> {
    use anyhow::Context;

    let expanded = get_expanded_path(path)?;
    let suffixes = [".cli.toml", ".cli.tomllmd", ".cli.tomllm"];
    let mut found = None;
    for suffix in &suffixes {
        let p = expanded.join(format!("{name}{suffix}"));
        if p.exists() {
            found = Some(p);
            break;
        }
    }
    let file_path = found
        .ok_or_else(|| anyhow::anyhow!("CLI datum '{name}' not found"))?;
    let content = std::fs::read_to_string(&file_path)
        .context(format!("read {}", file_path.display()))?;
    let config: UnifiedConfig =
        toml::from_str(&content).context(format!("parse {}", file_path.display()))?;
    Ok(config.b00t)
}

/// Load a polyseme datum and return its refs.
fn load_polyseme_refs(name: &str, path: &str) -> Result<Vec<crate::PolysemeRef>> {
    use anyhow::Context;

    let expanded = get_expanded_path(path)?;
    let suffixes = [".polyseme.toml", ".polyseme.tomllmd", ".polyseme.tomllm"];
    let mut found = None;
    for suffix in &suffixes {
        let p = expanded.join(format!("{name}{suffix}"));
        if p.exists() {
            found = Some(p);
            break;
        }
    }
    let file_path = found
        .ok_or_else(|| anyhow::anyhow!("polyseme datum '{name}' not found"))?;
    let content = std::fs::read_to_string(&file_path)
        .context(format!("read {}", file_path.display()))?;
    let config: UnifiedConfig =
        toml::from_str(&content).context(format!("parse {}", file_path.display()))?;
    Ok(config
        .b00t
        .polyseme
        .and_then(|p| p.refs)
        .unwrap_or_default())
}

pub fn get_mcp_toml_files(path: &str) -> Result<Vec<String>> {
    use anyhow::Context;
    use std::fs;

    let expanded_path = get_expanded_path(path)?;
    let entries = fs::read_dir(&expanded_path)
        .with_context(|| format!("Error reading directory {}", expanded_path.display()))?;

    let mut mcp_files = Vec::new();
    for entry in entries {
        if let Ok(entry) = entry {
            let entry_path = entry.path();
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(".mcp.toml") {
                    if let Some(server_name) = file_name.strip_suffix(".mcp.toml") {
                        mcp_files.push(server_name.to_string());
                    }
                }
            }
        }
    }
    Ok(mcp_files)
}

pub fn mcp_list(path: &str, json_output: bool, filter: McpListFilter) -> Result<()> {
    use anyhow::Context;
    
    

    let mcp_files = get_mcp_toml_files(path)?;
    let total_count = mcp_files.len();
    let mut mcp_items: Vec<McpListItem> = Vec::new();

    if total_count > 20 && !json_output {
        eprint!("🔍 Checking {} MCP servers...", total_count);
        let _ = std::io::stderr().flush();
    }
    let mut checked = 0usize;

    for server_name in mcp_files {
        checked += 1;
        if total_count > 20 && !json_output && checked % 10 == 0 {
            eprint!(" {}/{}", checked, total_count);
            let _ = std::io::stderr().flush();
        }
        match get_mcp_config(&server_name, path) {
            Ok(datum) => {
                let (command, args) =
                    if let Some(mcp) = &datum.mcp {
                        if let Some(stdio_methods) = &mcp.stdio {
                            if let Some(first_method) = stdio_methods.first() {
                                let command = first_method
                                    .get("command")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let args = first_method.get("args").and_then(|v| v.as_array()).map(
                                    |arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str())
                                            .map(|s| s.to_string())
                                            .collect::<Vec<String>>()
                                    },
                                );
                                (command, args)
                            } else {
                                (None, None)
                            }
                        } else if let Some(httpstream) = &mcp.httpstream {
                            let url = httpstream
                                .get("url")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            (Some("HTTP".to_string()), url.map(|u| vec![u]))
                        } else {
                            (None, None)
                        }
                    } else {
                        (datum.command.clone(), datum.args.clone())
                    };

                // is_installed: command on PATH OR registered in claude/vscode config
                let is_installed = command
                    .as_deref()
                    .map(|c| {
                        if c == "HTTP" {
                            return true;
                        }
                        if check_command_available(c) {
                            return true;
                        }
                        // also check if registered in claude config
                        let claude_cfg = dirs::home_dir()
                            .map(|h| h.join(".claude").join("settings.json"))
                            .filter(|p| p.exists());
                        if let Some(cfg) = claude_cfg {
                            if let Ok(content) = std::fs::read_to_string(&cfg) {
                                if content.contains(&format!("\"{}\"", server_name)) {
                                    return true;
                                }
                            }
                        }
                        false
                    })
                    .unwrap_or(false);
                // is_running: check if a process with the command name exists
                let is_running = command
                    .as_deref()
                    .and_then(|c| {
                        if c == "HTTP" { return Some(false); }
                        let cname = std::path::Path::new(c)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(c);
                        // try exact match first, then -f for full cmdline
                        let pgrep_result = duct::cmd!("pgrep", "-x", cname)
                            .stderr_null()
                            .read()
                            .ok()
                            .map(|s| !s.trim().is_empty());
                        if pgrep_result == Some(true) {
                            return Some(true);
                        }
                        duct::cmd!("pgrep", "-f", &format!("[{}]{}", &cname[..1], &cname[1..]))
                            .stderr_null()
                            .read()
                            .ok()
                            .map(|s| !s.trim().is_empty())
                    })
                    .unwrap_or(false);

                // Check if the datum has a suspended marker
                let is_suspended = datum
                    .mcp
                    .as_ref()
                    .and_then(|m| m.stdio.as_ref())
                    .and_then(|methods| methods.first())
                    .and_then(|m| m.get("enabled"))
                    .and_then(|v| v.as_bool())
                    .map(|enabled| !enabled)
                    .unwrap_or(false);

                // Generate restart hint for not-running servers
                let restart_hint = if is_installed && !is_running && !is_suspended {
                    let cmd = command.as_deref().unwrap_or("");
                    if let Some(mcp) = &datum.mcp {
                        if let Some(http) = &mcp.httpstream {
                            http.get("url")
                                .and_then(|v| v.as_str())
                                .map(|url| format!("restart: connect to httpstream at {}", url))
                        } else if let Some(methods) = &mcp.stdio {
                            methods.first().map(|m| {
                                let cmd = m.get("command").and_then(|v| v.as_str()).unwrap_or(cmd);
                                let args: Vec<&str> = m.get("args")
                                    .and_then(|v| v.as_array())
                                    .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
                                    .unwrap_or_default();
                                format!("restart: {} {}", cmd, args.join(" "))
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Determine transport type
                let transport = datum.mcp.as_ref().map(|m| {
                    if m.httpstream.is_some() {
                        "httpstream"
                    } else {
                        "stdio"
                    }
                }).map(|s| s.to_string());

                let item = McpListItem {
                    name: server_name.clone(),
                    command,
                    args,
                    hint: Some(datum.hint.clone()),
                    error: None,
                    is_installed,
                    is_running,
                    is_suspended,
                    transport,
                    restart_hint,
                };

                // Apply filters
                let search_pass = filter
                    .search
                    .as_ref()
                    .map(|q| item.name.to_lowercase().contains(&q.to_lowercase()))
                    .unwrap_or(true);
                let installed_pass = filter
                    .is_installed
                    .map(|f| item.is_installed == f)
                    .unwrap_or(true);
                let running_pass = filter
                    .is_running
                    .map(|f| item.is_running == f)
                    .unwrap_or(true);
                let suspended_pass = filter
                    .is_suspended
                    .map(|f| item.is_suspended == f)
                    .unwrap_or(true);

                if search_pass && installed_pass && running_pass && suspended_pass {
                    mcp_items.push(item);
                }
            }
            Err(e) => {
                let item = McpListItem {
                    name: server_name,
                    command: None,
                    args: None,
                    hint: None,
                    error: Some(e.to_string()),
                    is_installed: false,
                    is_running: false,
                    is_suspended: false,
                    transport: None,
                    restart_hint: None,
                };
                mcp_items.push(item);
            }
        }
    }

    // Threshold guard: if no explicit filter and count exceeds threshold, warn and demand filter
    let threshold = filter.max_threshold.unwrap_or_else(|| {
        session_memory::SessionMemory::load()
            .ok()
            .and_then(|m| {
                let t = m.config.mcp_list_threshold;
                if t > 0 { Some(t) } else { None }
            })
            .unwrap_or(10)
    });

    let has_active_filter = filter.search.is_some()
        || filter.is_installed.is_some()
        || filter.is_running.is_some()
        || filter.is_suspended.is_some();

    let truncated = !filter.bypass_threshold
        && !has_active_filter
        && mcp_items.len() > threshold as usize;

    if truncated {
        // Show filtered items that match the threshold guard (application-role aware)
        // We limit to threshold items and show a warning
        mcp_items.truncate(threshold as usize);
        // Check if role-based filtering can auto-select
        if let Ok(memory) = session_memory::SessionMemory::load() {
            if let Some(role) = memory.strings.get("last_role") {
                let role_lower = role.to_lowercase();
                let matching: Vec<&McpListItem> = mcp_items
                    .iter()
                    .filter(|item| {
                        item.name.to_lowercase().contains(&role_lower)
                            || item
                                .hint
                                .as_deref()
                                .unwrap_or("")
                                .to_lowercase()
                                .contains(&role_lower)
                    })
                    .collect();
                if !matching.is_empty() {
                    eprintln!("🎯 {role} role matched {count}/{total} MCP servers", role = role, count = matching.len(), total = total_count);
                }
            }
        }
    }

    if json_output {
        let expanded_path = get_expanded_path(path)?;
        let output = McpListOutput {
            servers: mcp_items,
            path: expanded_path.display().to_string(),
            truncated,
            threshold,
            total_count,
        };
        let json_str = serde_json::to_string_pretty(&output)
            .context("Failed to serialize MCP list to JSON")?;
        println!("{}", json_str);
    } else {
        // clear progress line if we showed one
        if total_count > 20 {
            eprint!("\r\x1b[K");
        }
        let expanded_path = get_expanded_path(path)?;
        if mcp_items.is_empty() && total_count == 0 {
            println!(
                "{}  No MCP server configurations found in {}",
                crate::ansi::yellow("⚠️"),
                expanded_path.display()
            );
            println!("   Use 'b00t-cli mcp add <json>' to add MCP server configurations.");
        } else if mcp_items.is_empty() && total_count > 0 {
            println!(
                "{}  No MCP servers match your filters ({} total available).",
                crate::ansi::yellow("⚠️"),
                crate::ansi::bold(&total_count.to_string()),
            );
            println!("   {}  Try: b00t-cli mcp list --all", crate::ansi::dim("💡"));
        } else {
            if truncated {
                println!(
                    "{}  Showing {shown}/{total} MCP servers (threshold={threshold}). Use --search or --installed/--running/--suspended to filter.",
                    crate::ansi::yellow("⚠️"),
                    shown = mcp_items.len(),
                    total = total_count,
                    threshold = threshold,
                );
                println!("   {}  Override: --max-threshold <N> or --all to bypass guard.", crate::ansi::dim("ℹ️"));
                println!();
            }
            // count summary
            let installed_count = mcp_items.iter().filter(|i| i.is_installed).count();
            let running_count = mcp_items.iter().filter(|i| i.is_running).count();
            let suspended_count = mcp_items.iter().filter(|i| i.is_suspended).count();
            if has_active_filter || total_count > threshold as usize {
                println!(
                    "{}  {} shown  {}  {} total  {}  {} installed  {}  {} running  {}  {} suspended",
                    crate::ansi::bold("📊"),
                    crate::ansi::cyan(&mcp_items.len().to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::bold(&total_count.to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::green(&installed_count.to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::green(&running_count.to_string()),
                    crate::ansi::dim("|"),
                    crate::ansi::yellow(&suspended_count.to_string()),
                );
            } else {
                println!("{}  Available MCP servers in {}:  ({})",
                    crate::ansi::bold("📋"),
                    crate::ansi::cyan(&expanded_path.display().to_string()),
                    crate::ansi::bold(&format!("{} total", total_count)),
                );
            }
            if !truncated && total_count > threshold as usize {
                println!("  (all {total} shown)", total = total_count);
            }
            println!();
            for item in &mcp_items {
                let status = if item.is_suspended {
                    "⏸️"
                } else if item.is_running {
                    "▶️"
                } else if item.is_installed {
                    "📋"
                } else {
                    "❌"
                };
                match (&item.command, &item.args) {
                    (Some(command), Some(args)) => {
                        println!("{status} {} ({command})", item.name);
                        if !args.is_empty() {
                            println!("   args: {}", args.join(" "));
                        }
                        if item.is_suspended {
                            println!("   ⏸️  SUSPENDED — enable with: b00t-cli mcp register --restore {}", item.name);
                        }
                        if !item.is_running && !item.is_suspended && item.is_installed {
                            if let Some(hint) = &item.restart_hint {
                                println!("   🔄  Not running — {hint}");
                            }
                        }
                    }
                    _ => {
                        println!("{status} {} (error reading config)", item.name);
                    }
                }
            }
            if truncated {
                println!();
                println!("💡 {total} total servers, showing first {threshold}. Use --search, --installed, --is-running, or --all to see more.", total = total_count, threshold = threshold);
            }
            // log view to telemetry (non-fatal)
            let _ = session_memory::SessionMemory::load().map(|mut m| {
                let key = format!("mcp_list_view_{}", chrono::Utc::now().format("%Y%m%d"));
                let _ = m.incr(&key);
                let _ = m.set("mcp_last_view_count", &total_count.to_string());
            });
            // also write to unified events.jsonl
            write_event("mcp_list_view", &total_count.to_string());
            println!();
            println!("To install to VSCode: b00t-cli vscode install mcp <name>");
            println!("To install to Claude Code: b00t-cli claude-code install mcp <name>");
        }
    }

    Ok(())
}

/// Register an MCP server configuration from JSON input
///
/// Creates a new multi-method MCP server configuration using the modern format
/// with [[b00t.cli]] sections and proper requirement specifications.
///
/// # Arguments
///
/// * `json` - JSON string containing MCP server configuration, or "-" to read from stdin
/// * `dwiw` - "Do What I Want" flag to auto-cleanup and format JSON comments
/// * `path` - Path to the _b00t_ directory where configuration will be stored
///
/// # Examples
///
/// ```rust,ignore
/// // Register from JSON string
/// let json = r#"{"name":"filesystem","command":"npx","args":["-y","@modelcontextprotocol/server-filesystem"]}"#;
/// b00t_cli::mcp_add_json(json, false, "~/.dotfiles/_b00t_").unwrap();
///
/// // Register with DWIW to strip comments
/// let json_with_comments = r#"{"name":"github","command":"npx","args":["-y","@modelcontextprotocol/server-github"]} // GitHub MCP server"#;
/// b00t_cli::mcp_add_json(json_with_comments, true, "~/.dotfiles/_b00t_").unwrap();
///
/// // CLI usage examples:
/// // b00t-cli mcp register '{"name":"filesystem","command":"npx","args":["-y","@modelcontextprotocol/server-filesystem"]}'
/// // b00t-cli mcp register brave-search -- npx -y @modelcontextprotocol/server-brave-search
/// // echo '{"name":"test"}' | b00t-cli mcp register -
/// ```
pub fn mcp_add_json(json: &str, dwiw: bool, path: &str) -> Result<()> {
    use std::io::{self, IsTerminal, Read};

    let json_content = if json == "-" {
        let mut buffer = String::new();

        // Check if reading from terminal (interactive) vs pipe
        if io::stdin().is_terminal() {
            eprintln!("📋 Paste your MCP server JSON configuration and press Ctrl+D when done:");
            eprintln!("💡 Supported formats:");
            eprintln!("   • Direct: {{\"name\":\"server\",\"command\":\"npx\",\"args\":[...]}}");
            eprintln!("   • mcpServers: {{\"mcpServers\":{{\"server\":{{...}}}}}}");
            eprintln!("   • Named: {{\"server-name\":{{\"command\":\"npx\",...}}}}");
            eprintln!("");
        }

        match io::stdin().read_to_string(&mut buffer) {
            Ok(_) => {
                let trimmed = buffer.trim();
                if trimmed.is_empty() {
                    anyhow::bail!(
                        "No input provided. Pipe JSON content or press Ctrl+D after pasting."
                    );
                }
                trimmed.to_string()
            }
            Err(e) => {
                anyhow::bail!(
                    "Failed to read from stdin: {}. Pipe JSON content or use Ctrl+D after input.",
                    e
                );
            }
        }
    } else {
        json.trim().to_string()
    };

    let datum = normalize_mcp_json(&json_content, dwiw)?;

    create_mcp_toml_config(&datum, path)?;

    println!("MCP server '{}' configuration saved.", datum.name);
    println!(
        "To install to VSCode: b00t-cli vscode install mcp {}",
        datum.name
    );

    Ok(())
}

/// Remove an MCP server configuration by name
///
/// # Examples
///
/// ```rust,ignore
/// // Remove an MCP server configuration from the _b00t_ directory
/// b00t_cli::mcp_remove("filesystem", "~/.dotfiles/_b00t_").unwrap();
///
/// // CLI usage:
/// // b00t-cli mcp register --remove filesystem
/// ```
pub fn mcp_remove(name: &str, path: &str) -> Result<()> {
    use std::fs;
    use std::path::PathBuf;

    let expanded_path = get_expanded_path(path)?;
    let mut mcp_path = PathBuf::from(expanded_path);

    // Construct the filename
    let filename = format!("{}.mcp.toml", name);
    mcp_path.push(filename);

    if mcp_path.exists() {
        fs::remove_file(&mcp_path).with_context(|| {
            format!(
                "Failed to remove MCP server configuration: {}",
                mcp_path.display()
            )
        })?;
        println!("Removed MCP server configuration: {}", name);
    } else {
        anyhow::bail!("MCP server configuration not found: {}", name);
    }

    Ok(())
}

pub fn mcp_output(path: &str, use_mcp_servers_wrapper: bool, servers: &str) -> Result<()> {
    use anyhow::Context;

    let requested_servers: Vec<&str> = servers.split(',').map(|s| s.trim()).collect();
    let mut server_configs = serde_json::Map::new();

    for server_name in requested_servers {
        if server_name.is_empty() {
            continue;
        }

        match get_mcp_config(server_name, path) {
            Ok(datum) => {
                let (command, args) = extract_mcp_command_args(&datum);
                let mut server_config = serde_json::Map::new();
                server_config.insert("command".to_string(), serde_json::Value::String(command));
                server_config.insert(
                    "args".to_string(),
                    serde_json::Value::Array(
                        args.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );

                server_configs.insert(
                    server_name.to_string(),
                    serde_json::Value::Object(server_config),
                );
            }
            Err(_) => {
                // Create a cute poopy log error indicator instead of stderr warning
                let mut error_config = serde_json::Map::new();
                error_config.insert(
                    "command".to_string(),
                    serde_json::Value::String("b00t:💩🪵".to_string()),
                );

                let utc_timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let utc_time = chrono::DateTime::from_timestamp(utc_timestamp as i64, 0)
                    .unwrap()
                    .format("%Y-%m-%dT%H:%M:%SZ")
                    .to_string();

                error_config.insert(
                    "args".to_string(),
                    serde_json::Value::Array(vec![
                        serde_json::Value::String(utc_time),
                        serde_json::Value::String(format!(
                            "server '{}' not found in _b00t_ directory",
                            server_name
                        )),
                    ]),
                );

                server_configs.insert(
                    server_name.to_string(),
                    serde_json::Value::Object(error_config),
                );
            }
        }
    }

    let output = if use_mcp_servers_wrapper {
        let mut wrapper = serde_json::Map::new();
        wrapper.insert(
            "mcpServers".to_string(),
            serde_json::Value::Object(server_configs),
        );
        serde_json::Value::Object(wrapper)
    } else {
        serde_json::Value::Object(server_configs)
    };

    let json_str =
        serde_json::to_string_pretty(&output).context("Failed to serialize MCP servers to JSON")?;
    println!("{}", json_str);

    Ok(())
}

/// Extract command and args from MCP datum, handling both new multi-method and legacy formats
fn extract_mcp_command_args(datum: &BootDatum) -> (String, Vec<String>) {
    if let Some(mcp) = &datum.mcp {
        if let Some(stdio_methods) = &mcp.stdio {
            if let Some(first_method) = stdio_methods.first() {
                let command = first_method
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("npx")
                    .to_string();
                let args = first_method
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default();
                return (command, args);
            }
        }
    }

    // Fallback to legacy fields for backwards compatibility
    (
        datum.command.clone().unwrap_or_else(|| "npx".to_string()),
        datum.args.clone().unwrap_or_default(),
    )
}

/// Resolve the active MCP method (stdio/httpstream) and return command details.
fn select_mcp_method(
    datum: &BootDatum,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<(
    String,
    Vec<String>,
    Option<std::collections::HashMap<String, String>>,
    &'static str,
)> {
    if let Some(methods) = &datum.mcp {
        if use_httpstream {
            if let Some(httpstream_method) = &methods.httpstream {
                let url = httpstream_method
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing url in httpstream method"))?;

                return Ok((url.to_string(), vec![], None, "httpstream"));
            } else {
                anyhow::bail!("No httpstream method available for MCP '{}'", datum.name);
            }
        }

        if let Some(stdio_command_filter) = stdio_command {
            if let Some(stdio_methods) = &methods.stdio {
                let matching_method = stdio_methods.iter().find(|method| {
                    method
                        .get("command")
                        .and_then(|v| v.as_str())
                        .map(|cmd| cmd == stdio_command_filter)
                        .unwrap_or(false)
                });

                if let Some(method) = matching_method {
                    let command = method
                        .get("command")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow::anyhow!("Missing command in stdio method"))?;
                    let args = method
                        .get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default();
                    let env = method.get("env").and_then(|v| v.as_object()).map(|obj| {
                        obj.iter()
                            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                            .collect::<std::collections::HashMap<String, String>>()
                    });

                    return Ok((command.to_string(), args, env, "stdio"));
                } else {
                    anyhow::bail!(
                        "No stdio method with command '{}' found for MCP '{}'. Available commands: {}",
                        stdio_command_filter,
                        datum.name,
                        stdio_methods
                            .iter()
                            .filter_map(|m| m.get("command").and_then(|v| v.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            } else {
                anyhow::bail!("No stdio methods available for MCP '{}'", datum.name);
            }
        } else if let Some(stdio_methods) = &methods.stdio {
            if stdio_methods.is_empty() {
                anyhow::bail!("No stdio methods available for MCP '{}'", datum.name);
            }

            let method = &stdio_methods[0];
            let command = method
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("Missing command in stdio method"))?;
            let args = method
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            let env = method.get("env").and_then(|v| v.as_object()).map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect::<std::collections::HashMap<String, String>>()
            });

            return Ok((command.to_string(), args, env, "stdio"));
        } else {
            anyhow::bail!("No stdio methods available for MCP '{}'", datum.name);
        }
    }

    let (command, args) = extract_mcp_command_args(datum);
    Ok((command, args, datum.env.clone(), "stdio"))
}

// MCP Installation Functions
pub fn claude_code_install_mcp(name: &str, path: &str) -> Result<()> {
    use duct::cmd;

    let datum = get_mcp_config(name, path)?;
    let (command, args) = extract_mcp_command_args(&datum);

    let claude_json = serde_json::json!({
        "name": datum.name,
        "command": command,
        "args": args
    });

    let json_str =
        serde_json::to_string(&claude_json).context("Failed to serialize JSON for Claude Code")?;

    let result = cmd!("claude", "mcp", "add-json", &datum.name, &json_str).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to Claude Code",
                datum.name
            );
            println!(
                "Claude Code command: claude mcp add-json {} '{}'",
                datum.name, json_str
            );
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to Claude Code: {}", e);
            eprintln!(
                "Manual command: claude mcp add-json {} '{}'",
                datum.name, json_str
            );
            return Err(anyhow::anyhow!("Claude Code installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn vscode_install_mcp(name: &str, path: &str) -> Result<()> {
    use duct::cmd;

    let datum = get_mcp_config(name, path)?;
    let (command, args) = extract_mcp_command_args(&datum);

    let vscode_json = serde_json::json!({
        "name": datum.name,
        "command": command,
        "args": args
    });

    let json_str =
        serde_json::to_string(&vscode_json).context("Failed to serialize JSON for VSCode")?;

    let result = cmd!("code", "--add-mcp", &json_str).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to VSCode",
                datum.name
            );
            println!("VSCode command: code --add-mcp '{}'", json_str);
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to VSCode: {}", e);
            eprintln!("Manual command: code --add-mcp '{}'", json_str);
            return Err(anyhow::anyhow!("VSCode installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn gemini_install_mcp(name: &str, path: &str, use_repo: bool) -> Result<()> {
    use duct::cmd;

    let datum = get_mcp_config(name, path)?;
    let (command, args) = extract_mcp_command_args(&datum);

    let gemini_json = serde_json::json!({
        "name": datum.name,
        "command": command,
        "args": args
    });

    let json_str =
        serde_json::to_string(&gemini_json).context("Failed to serialize JSON for Gemini CLI")?;

    let location_flag = if use_repo { "--repo" } else { "--user" };
    let result = cmd!(
        "gemini",
        "mcp",
        "add-json",
        location_flag,
        &datum.name,
        &json_str
    )
    .run();

    match result {
        Ok(_) => {
            let location = if use_repo {
                "repository"
            } else {
                "user global"
            };
            println!(
                "Successfully installed MCP server '{}' to Gemini CLI ({})",
                datum.name, location
            );
            println!(
                "Gemini CLI command: gemini mcp add-json {} {} '{}'",
                location_flag, datum.name, json_str
            );
        }
        Err(e) => {
            let location = if use_repo {
                "repository"
            } else {
                "user global"
            };
            eprintln!(
                "Failed to install MCP server to Gemini CLI ({}): {}",
                location, e
            );
            eprintln!(
                "Manual command: gemini mcp add-json {} {} '{}'",
                location_flag, datum.name, json_str
            );
            return Err(anyhow::anyhow!("Gemini CLI installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn codex_install_mcp(
    name: &str,
    path: &str,
    _use_repo: bool,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<()> {
    let datum = get_mcp_config(name, path)?;
    let (command, args, env, method_type) =
        select_mcp_method(&datum, stdio_command, use_httpstream)?;

    let mut codex_args = vec!["mcp".to_string(), "add".to_string()];

    if let Some(env_map) = env {
        for (key, value) in env_map {
            codex_args.push("--env".to_string());
            codex_args.push(format!("{key}={value}"));
        }
    }

    codex_args.push(name.to_string());
    if method_type == "httpstream" {
        codex_args.push("--url".to_string());
        codex_args.push(command.clone());
    } else {
        codex_args.push("--".to_string());
        codex_args.push(command.clone());
        codex_args.extend(args.clone());
    }

    let result = cmd("codex", &codex_args).run();

    match result {
        Ok(_) => {
            println!(
                "Successfully installed MCP server '{}' to Codex",
                datum.name
            );
            println!("Codex command: codex {}", codex_args.join(" "));
        }
        Err(e) => {
            eprintln!("Failed to install MCP server to Codex: {}", e);
            eprintln!("Manual command: codex {}", codex_args.join(" "));
            return Err(anyhow::anyhow!("Codex installation failed: {}", e));
        }
    }

    Ok(())
}

pub fn dotmcpjson_install_mcp(
    name: &str,
    path: &str,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<()> {
    use crate::utils::get_workspace_root;

    // Get MCP configuration from b00t-cli
    let datum = get_mcp_config(name, path)?;

    // Find the repo root and .mcp.json file
    let repo_root = get_workspace_root();
    let mcp_json_path = std::path::Path::new(&repo_root).join(".mcp.json");

    if !mcp_json_path.exists() {
        anyhow::bail!("No .mcp.json file found in repo root: {}", repo_root);
    }

    // Load existing .mcp.json
    let existing_content =
        std::fs::read_to_string(&mcp_json_path).context("Failed to read .mcp.json file")?;

    let mut mcp_config: serde_json::Value =
        serde_json::from_str(&existing_content).context("Failed to parse .mcp.json file")?;

    // Ensure mcpServers object exists
    if !mcp_config.is_object() {
        mcp_config = serde_json::json!({});
    }
    if !mcp_config["mcpServers"].is_object() {
        mcp_config["mcpServers"] = serde_json::json!({});
    }

    // Handle multi-source selection if available
    let (command, args, env, method_type) =
        select_mcp_method(&datum, stdio_command, use_httpstream)?;

    // Create MCP server entry for .mcp.json format
    let server_config = if method_type == "httpstream" {
        // For httpstream, use url instead of command/args
        serde_json::json!({
            "url": command
        })
    } else {
        // For stdio, use command and args
        serde_json::json!({
            "command": command,
            "args": args
        })
    };

    // Add optional env if present
    if let Some(method_env) = env {
        if let Some(server_obj) = server_config.as_object() {
            let mut new_config = server_obj.clone();
            new_config.insert("env".to_string(), serde_json::to_value(method_env)?);
            mcp_config["mcpServers"][&datum.name] = serde_json::Value::Object(new_config);
        }
    } else {
        mcp_config["mcpServers"][&datum.name] = server_config;
    }

    // Write back to .mcp.json with pretty formatting
    let updated_content = serde_json::to_string_pretty(&mcp_config)
        .context("Failed to serialize updated .mcp.json")?;

    std::fs::write(&mcp_json_path, updated_content)
        .context("Failed to write updated .mcp.json file")?;

    println!(
        "✅ Successfully installed MCP server '{}' to .mcp.json",
        datum.name
    );

    if method_type == "httpstream" {
        println!("🌐 Used httpstream method");
    } else if let Some(cmd) = stdio_command {
        println!("🎯 Used stdio method with command: {}", cmd);
    } else {
        println!("📡 Used default stdio method");
    }

    println!("📁 Updated: {}", mcp_json_path.display());

    Ok(())
}

/// Install an MCP server to opencode's config (~/.config/opencode/opencode.json).
pub fn opencode_install_mcp(
    name: &str,
    path: &str,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<()> {
    let datum = get_mcp_config(name, path)?;
    let (command, args, env, method_type) =
        select_mcp_method(&datum, stdio_command, use_httpstream)?;

    // Build the server entry — opencode uses "command" as an array of args
    // stdio: ["/path/to/binary"] or ["npx", "-y", "@package"]
    // httpstream: not currently opencode-native, but we store it for reference
    let mut command_arr = vec![command.clone()];
    command_arr.extend(args.clone());

    let mut server_entry = serde_json::json!({
        "enabled": true,
        "type": "local",
        "command": command_arr
    });

    // Add optional env if present
    if let Some(env_map) = &env {
        if let Some(obj) = server_entry.as_object_mut() {
            obj.insert("env".to_string(), serde_json::to_value(env_map)?);
        }
    }

    // Determine opencode config path
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let config_path = home.join(".config").join("opencode").join("opencode.json");

    // If the config doesn't exist, try the JSONC variant
    let config_path = if config_path.exists() {
        config_path
    } else {
        let jsonc_path = home.join(".config").join("opencode").join("opencode.jsonc");
        jsonc_path
    };

    // Read existing config or start fresh
    let mut config: serde_json::Value = if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)
            .context("Failed to read opencode config")?;
        serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse opencode config: {}", e))?
    } else {
        serde_json::json!({})
    };

    // Ensure mcp object exists
    if !config["mcp"].is_object() {
        config["mcp"] = serde_json::json!({});
    }

    // Add/update the server entry
    config["mcp"][name] = server_entry;

    // Write back with pretty formatting
    let updated = serde_json::to_string_pretty(&config)
        .context("Failed to serialize opencode config")?;
    std::fs::write(&config_path, updated)
        .context("Failed to write opencode config")?;

    println!(
        "✅ Successfully installed MCP server '{}' to opencode",
        name
    );
    println!("📁 Config: {}", config_path.display());

    if method_type == "httpstream" {
        println!("🌐 Used httpstream method");
    } else if let Some(cmd) = stdio_command {
        println!("🎯 Used stdio method with command: {}", cmd);
    } else {
        println!("📡 Used stdio method");
    }

    Ok(())
}

/// Push all repo .mcp.json servers into Codex CLI config via `codex mcp add`.
pub fn codex_sync_dotmcpjson(path: &str, use_repo: bool) -> Result<()> {
    use crate::utils::get_workspace_root;
    use std::path::Path;

    let _ = path; // retained for interface parity with other installers

    let repo_root = get_workspace_root();
    let mcp_json_path = Path::new(&repo_root).join(".mcp.json");

    if !mcp_json_path.exists() {
        anyhow::bail!("No .mcp.json file found in repo root: {}", repo_root);
    }

    let content = std::fs::read_to_string(&mcp_json_path)
        .context("Failed to read .mcp.json for Codex sync")?;
    let value: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse .mcp.json for Codex sync")?;
    let servers = value
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow::anyhow!("Missing mcpServers in {}", mcp_json_path.display()))?;

    if servers.is_empty() {
        anyhow::bail!("No MCP servers present in {}", mcp_json_path.display());
    }

    let mut failures = Vec::new();

    for (name, config) in servers {
        let mut codex_cmd = std::process::Command::new("codex");
        codex_cmd.args(["mcp", "add"]);

        if let Some(env) = config.get("env").and_then(|v| v.as_object()) {
            for (key, value) in env {
                if let Some(value) = value.as_str() {
                    codex_cmd.args(["--env", &format!("{}={}", key, value)]);
                }
            }
        }

        if let Some(url) = config.get("url").and_then(|v| v.as_str()) {
            codex_cmd.args([name, "--url", url]);
        } else {
            let command = match config.get("command").and_then(|v| v.as_str()) {
                Some(command) => command,
                None => {
                    failures.push((name.clone(), "missing command or url".to_string()));
                    continue;
                }
            };
            codex_cmd.arg(name).arg("--").arg(command);
            if let Some(args) = config.get("args").and_then(|v| v.as_array()) {
                codex_cmd.args(args.iter().filter_map(|v| v.as_str()));
            }
        }

        match codex_cmd.status() {
            Ok(status) if status.success() => println!("Codex synced '{}'", name),
            Ok(status) => failures.push((name.clone(), format!("exited with status {}", status))),
            Err(e) => failures.push((name.clone(), e.to_string())),
        }
    }

    if failures.is_empty() {
        let location = if use_repo {
            "repository"
        } else {
            "user global"
        };
        println!(
            "✅ Synced {} MCP servers from {} into Codex ({})",
            servers.len(),
            mcp_json_path.display(),
            location
        );
        Ok(())
    } else {
        let details = failures
            .iter()
            .map(|(name, err)| format!("{}: {}", name, err))
            .collect::<Vec<_>>()
            .join("; ");
        Err(anyhow::anyhow!(
            "Failed to sync {} servers to Codex: {}",
            failures.len(),
            details
        ))
    }
}

/// Bidirectional MCP sync between b00t datums and external agent platforms.
///
/// Current behavior:
/// - `push`: requires `source == "b00t"`
/// - `pull`: requires `dest == "b00t"`
/// - legacy targets can be expanded over time
pub fn mcp_sync_bidirectional(
    path: &str,
    operation: &str,
    source: &str,
    dest: &str,
    _agent: Option<&str>,
) -> Result<()> {
    let is_known_platform = |platform: &str| {
        matches!(
            platform,
            "b00t" | "kiro" | "claude" | "claudecode" | "codex" | "dotmcpjson" | "roocode"
        )
    };

    let normalized_op = operation.to_lowercase();
    let normalized_source = source.to_lowercase();
    let normalized_dest = dest.to_lowercase();

    if !is_known_platform(&normalized_source) {
        anyhow::bail!("Unknown platform '{}'", source);
    }
    if !is_known_platform(&normalized_dest) {
        anyhow::bail!("Unknown platform '{}'", dest);
    }

    match normalized_op.as_str() {
        "push" => {
            if normalized_source != "b00t" {
                anyhow::bail!("Push operation requires source to be 'b00t'");
            }

            match normalized_dest.as_str() {
                "codex" => codex_sync_dotmcpjson(path, true),
                "dotmcpjson" | "roocode" => {
                    for server_name in get_mcp_toml_files(path)? {
                        dotmcpjson_install_mcp(&server_name, path, None, false)?;
                    }
                    Ok(())
                }
                _ => anyhow::bail!(
                    "Push to platform '{}' is not implemented yet. Supported push destinations: codex, dotmcpjson, roocode",
                    dest
                ),
            }
        }
        "pull" => {
            if normalized_dest != "b00t" {
                anyhow::bail!("Pull operation requires destination to be 'b00t'");
            }
            anyhow::bail!(
                "Pull from platform '{}' is not implemented yet. Use MCP register/output as workaround",
                source
            )
        }
        _ => anyhow::bail!("Invalid operation '{}'", operation),
    }
}

// Session management functions
impl SessionState {
    pub fn new(agent_name: Option<String>) -> Self {
        let session_id = format!("b00t_{}", chrono::Utc::now().timestamp_millis() % 100000000);

        let agent_info = agent_name.map(|name| AgentInfo {
            name: name.clone(),
            model_size: std::env::var("MODEL_SIZE").ok(),
            role: std::env::var("ROLE").ok(),
            pid: std::process::id(),
            privacy_level: std::env::var("PRIVACY").ok(),
        });

        SessionState {
            session_id,
            start_time: Utc::now(),
            commands_run: 0,
            estimated_cost: 0.0,
            budget_limit: std::env::var("B00T_BUDGET")
                .ok()
                .and_then(|s| s.parse().ok()),
            time_limit_minutes: std::env::var("B00T_TIME_LIMIT")
                .ok()
                .and_then(|s| s.parse().ok()),
            agent_info,
            hints: vec![],
            last_activity: Utc::now(),
        }
    }

    pub fn get_session_file_path() -> Result<std::path::PathBuf> {
        let session_id = std::env::var("B00T_SESSION_ID").unwrap_or_else(|_| "current".to_string());
        let tmp_dir = std::env::temp_dir();
        Ok(tmp_dir.join(format!("b00t_session_{}.json", session_id)))
    }

    pub fn load() -> Result<Self> {
        let path = Self::get_session_file_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path).context("Failed to read session file")?;
            serde_json::from_str(&content).context("Failed to parse session file")
        } else {
            Ok(Self::new(std::env::var("_B00T_Agent").ok()))
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_session_file_path()?;
        let content = serde_json::to_string_pretty(self).context("Failed to serialize session")?;
        std::fs::write(&path, content).context("Failed to write session file")?;
        Ok(())
    }

    pub fn increment_command(&mut self, estimated_cost: f64) {
        self.commands_run += 1;
        self.estimated_cost += estimated_cost;
        self.last_activity = Utc::now();
    }

    pub fn get_status_line(&self) -> String {
        let duration = Utc::now().signed_duration_since(self.start_time);
        let elapsed_mins = duration.num_minutes();

        let cost_info = if self.estimated_cost > 0.0 {
            format!(" ${:.3}", self.estimated_cost)
        } else {
            String::new()
        };

        let time_info = if elapsed_mins > 0 {
            format!(" {}m", elapsed_mins)
        } else {
            format!(" {}s", duration.num_seconds())
        };

        let agent_info = self
            .agent_info
            .as_ref()
            .map(|a| format!(" {}", a.name))
            .unwrap_or_default();

        format!(
            "🥾 {} cmds{}{}{}",
            self.commands_run, cost_info, time_info, agent_info
        )
    }
}

/// Generic loader for datum providers keyed by file extension.
pub fn load_datum_providers<T>(
    path: &str,
    extension: &str,
) -> Result<Vec<Box<dyn traits::DatumProvider>>>
where
    T: traits::DatumProvider + 'static,
    T: for<'a> TryFrom<(&'a str, &'a str), Error = anyhow::Error>,
{
    let mut tools: Vec<Box<dyn traits::DatumProvider>> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Scan project-local first, then global — project overrides global
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    if let Some(project) = find_project_b00t() {
        dirs.push(project);
    }
    if let Ok(global) = get_expanded_path(path) {
        dirs.push(global);
    }

    for dir in &dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                    if file_name.ends_with(extension) {
                        if let Some(tool_name) = file_name.strip_suffix(extension) {
                            if seen.insert(tool_name.to_string()) {
                                if let Ok(datum) = T::try_from((tool_name, path)) {
                                    tools.push(Box::new(datum));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(tools)
}

/// Initialize a session and persist its state.
pub fn handle_session_init(
    budget: &Option<f64>,
    time_limit: &Option<u32>,
    agent: Option<&str>,
) -> Result<()> {
    let agent_name = agent
        .map(|s| s.to_string())
        .or_else(|| std::env::var("_B00T_Agent").ok())
        .filter(|s| !s.is_empty());

    let mut session = SessionState::new(agent_name);

    if let Some(budget) = budget {
        session.budget_limit = Some(*budget);
    }

    if let Some(time_limit) = time_limit {
        session.time_limit_minutes = Some(*time_limit);
    }

    // Set session ID in environment
    unsafe {
        std::env::set_var("B00T_SESSION_ID", &session.session_id);
    }

    session.save()?;

    // Initialize session memory and check README.md
    let mut memory = session_memory::SessionMemory::load()?;
    check_readme_status(&mut memory)?;

    println!("🥾 Session {} initialized", session.session_id);

    if let Some(agent) = &session.agent_info {
        println!("🤖 Agent: {}", agent.name);
    }

    if let Some(budget) = session.budget_limit {
        println!("💰 Budget: ${:.2}", budget);
    }

    if let Some(time_limit) = session.time_limit_minutes {
        println!("⏱️  Time limit: {}m", time_limit);
    }

    Ok(())
}

/// Display current session status.
pub fn handle_session_status() -> Result<()> {
    let session = SessionState::load()?;
    println!("{}", session.get_status_line());

    if !session.hints.is_empty() {
        println!("💡 Hints:");
        for hint in &session.hints {
            println!("   • {}", hint);
        }
    }

    Ok(())
}

/// Update session state with cost and optional hint.
pub fn handle_session_update(cost: &Option<f64>, hint: Option<&str>) -> Result<()> {
    let mut session = SessionState::load()?;

    if let Some(cost) = cost {
        session.increment_command(*cost);
    } else {
        session.increment_command(0.0);
    }

    if let Some(hint) = hint {
        session.hints.push(hint.to_string());
    }

    session.save()?;
    Ok(())
}

/// End the current session and clear persisted state.
pub fn handle_session_end() -> Result<()> {
    let session = SessionState::load()?;
    let path = SessionState::get_session_file_path()?;

    println!("🥾 Session {} ended", session.session_id);
    println!("📊 Final stats: {}", session.get_status_line());

    if path.exists() {
        std::fs::remove_file(&path).context("Failed to remove session file")?;
    }

    unsafe {
        std::env::remove_var("B00T_SESSION_ID");
    }
    Ok(())
}

/// Print a one-line status prompt for the current session.
pub fn handle_session_prompt() -> Result<()> {
    let session = SessionState::load()?;
    print!("{}", session.get_status_line());
    Ok(())
}

fn check_readme_status(memory: &mut session_memory::SessionMemory) -> Result<()> {
    use crate::utils::get_workspace_root;
    let git_root = get_workspace_root();
    let readme_path = std::path::PathBuf::from(&git_root).join("README.md");

    if readme_path.exists() {
        if !memory.is_readme_read() {
            println!("📖 README.md found but not yet marked as read");
            println!("💡 Run `b00t-cli session mark-readme-read` after reading it");
        } else {
            println!("✅ README.md already read this session");
        }
    } else {
        println!("ℹ️  No README.md found in git root");
    }

    Ok(())
}

/// Crate-wide lock for tests that mutate process-wide environment variables.
/// A per-module lock cannot prevent concurrent mutations from tests in *other*
/// modules; this single static is the authoritative guard for all env-var
/// manipulation across the entire `b00t_cli` test suite.
#[cfg(test)]
pub mod test_env {
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    pub static ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));
}

#[cfg(test)]
mod tests {
    use crate::InstallSpec;
    use serde::Deserialize;
    use std::path::PathBuf;
    use std::sync::{Mutex, MutexGuard};

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct TempHome {
        _guard: MutexGuard<'static, ()>,
        old_home: Option<String>,
        _temp_dir: tempfile::TempDir,
        b00t_dir: PathBuf,
    }

    impl TempHome {
        fn new() -> Self {
            let guard = HOME_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let temp_dir = tempfile::tempdir().unwrap();
            let b00t_dir = temp_dir.path().join(".b00t");
            std::fs::create_dir_all(&b00t_dir).unwrap();
            let old_home = std::env::var("HOME").ok();
            unsafe {
                std::env::set_var("HOME", temp_dir.path().to_str().unwrap());
            }

            Self {
                _guard: guard,
                old_home,
                _temp_dir: temp_dir,
                b00t_dir,
            }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            if let Some(old) = &self.old_home {
                unsafe { std::env::set_var("HOME", old); }
            } else {
                unsafe { std::env::remove_var("HOME"); }
            }
        }
    }

    #[test]
    fn test_datum_type_from_filename_accepts_typed_toml_extensions() {
        assert_eq!(
            crate::DatumType::from_filename("b00t.cli"),
            crate::DatumType::Cli
        );
        assert_eq!(
            crate::DatumType::from_filename("b00t.cli.toml"),
            crate::DatumType::Cli
        );
        assert_eq!(
            crate::DatumType::from_filename("executive.role.tomllmd"),
            crate::DatumType::Role
        );
        assert_eq!(
            crate::DatumType::from_filename("executive.role.tomllm"),
            crate::DatumType::Role
        );
        assert_eq!(
            crate::DatumType::from_filename("irontology.mcp.toml"),
            crate::DatumType::Mcp
        );
        // 🤓 hardware datums use dotted SoC.subsystem namespace
        assert_eq!(
            crate::DatumType::from_filename("rk3588.npu.hardware.tomllmd"),
            crate::DatumType::Hardware
        );
        assert_eq!(
            crate::DatumType::from_filename("rtx3090.hardware.toml"),
            crate::DatumType::Hardware
        );
        // 🤓 overlay datums carry node-local state in git enclave
        assert_eq!(
            crate::DatumType::from_filename("models.overlay.toml"),
            crate::DatumType::Overlay
        );
        assert_eq!(
            crate::DatumType::from_filename("unknown.toml"),
            crate::DatumType::Unknown
        );
    }

    #[test]
    fn test_bootdatum_uninstall_fields_deserialize() {
        let toml_str = r#"
[b00t]
name = "ripgrep"
type = "cli"
hint = "fast grep"
install = "apt-get install -y ripgrep"
uninstall = "apt-get remove -y ripgrep"
hook_uninstall = "// Rhai: post-uninstall cleanup\nlet x = 1;"
"#;
        let config: crate::UnifiedConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(
            config.b00t.uninstall,
            Some("apt-get remove -y ripgrep".to_string())
        );
        assert_eq!(
            config.b00t.hook_uninstall,
            Some("// Rhai: post-uninstall cleanup\nlet x = 1;".to_string())
        );
    }

    #[test]
    fn test_bootdatum_uninstall_fields_default_none() {
        let toml_str = r#"
[b00t]
name = "docker"
type = "cli"
hint = "containers"
"#;
        let config: crate::UnifiedConfig = toml::from_str(toml_str).unwrap();
        assert!(config.b00t.uninstall.is_none());
        assert!(config.b00t.hook_uninstall.is_none());
    }

    // ── get_config tomllmd precedence ─────────────────────────────────────────

    #[test]
    fn test_get_config_prefers_tomllmd_over_tomllm_and_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        // Write all three extension variants for the same datum
        std::fs::write(
            dir.path().join("mytool.cli.toml"),
            "[b00t]\nname = \"mytool-toml\"\ntype = \"cli\"\nhint = \"toml\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("mytool.cli.tomllm"),
            "[b00t]\nname = \"mytool-tomllm\"\ntype = \"cli\"\nhint = \"tomllm\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("mytool.cli.tomllmd"),
            "[b00t]\nname = \"mytool-tomllmd\"\ntype = \"cli\"\nhint = \"tomllmd\"\n",
        )
        .unwrap();

        let (config, filename) = crate::get_config("mytool", path).unwrap();
        assert_eq!(
            config.b00t.name, "mytool-tomllmd",
            ".tomllmd must be returned first"
        );
        assert!(
            filename.ends_with(".tomllmd"),
            "filename must end with .tomllmd, got {}",
            filename
        );
    }

    #[test]
    fn test_get_config_falls_back_to_tomllm_when_no_tomllmd() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();

        std::fs::write(
            dir.path().join("mytool.cli.toml"),
            "[b00t]\nname = \"mytool-toml\"\ntype = \"cli\"\nhint = \"toml\"\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("mytool.cli.tomllm"),
            "[b00t]\nname = \"mytool-tomllm\"\ntype = \"cli\"\nhint = \"tomllm\"\n",
        )
        .unwrap();

        let (config, filename) = crate::get_config("mytool", path).unwrap();
        assert_eq!(config.b00t.name, "mytool-tomllm");
        assert!(filename.ends_with(".tomllm"), "got {}", filename);
    }

    // ── write_event re-export from c0re-lib ────────────────────────────────────

    #[test]
    fn test_write_event_reexport_from_c0re_lib() {
        use std::fs;

        let temp_home = TempHome::new();

        // write_event is re-exported from b00t_c0re_lib
        crate::write_event("mcp_list_view", "42");

        let events_path = temp_home.b00t_dir.join("events.jsonl");
        assert!(events_path.exists(), "events.jsonl should exist");
        let content = fs::read_to_string(&events_path).unwrap();
        let line = content.lines().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        assert_eq!(parsed["event"], "mcp_list_view");
        assert_eq!(parsed["detail"], "42");
    }

    // ── mti type_id tests ──────────────────────────────────────────────────
    use super::{BootDatum, DatumType};

    fn make_datum(name: &str, datum_type: Option<DatumType>) -> BootDatum {
        BootDatum {
            name: name.to_string(),
            datum_type,
            hint: String::new(),
            status: None, enabled: None, status_msg: None, replacement: None,
            git_attributes: Default::default(), desires: None, auto_install: None,
            skills: None, compliance: None, install: None, update: None,
            version: None, version_regex: None, requires_sudo: false,
            command: None, args: None, vsix_id: None, script: None,
            image: None, docker_args: None, oci_uri: None, resource_path: None,
            chart_path: None, namespace: None, values_file: None,
            keywords: None, package_name: None, ansible: None,
            env: None, require: None, aliases: None, k0mmand3r: None,
            knowledge: None, mcp: None, gate: None, url: None, branch: None,
            clone_path: None, entangled_agents: None, entangled_cli: None,
            entangled_mcp: None, entangled_ai_models: None, entangled_apis: None,
            entangled_docker: None, entangled_k8s: None, channel_prefix: None,
            depends_on: None, members: None, orchestration: None,
            model_hf_id: None, model_size_gb: None, model_size_4bit_gb: None,
            stack: None, job: None, skill: None, dsn: None, justfile: None, pipeline: None,
            learn: None, lfmf_category: None, usage: None, provides: None,
            protocol: None, implements: None, hook_detect: None,
            hook_install: None, hook_update: None, hook_learn: None,
            uninstall: None, hook_uninstall: None, unlocks: None,
            type_tags: None, maintenance: None, required_for_core: None,
            runtime: None, polyseme: None, trigger_words: None, compose: None,
        }
    }

    #[test]
    fn type_id_is_deterministic() {
        let d = make_datum("bayesian", Some(DatumType::Skill));
        assert_eq!(d.type_id(), d.type_id(), "same inputs must produce same TypeID");
    }

    #[test]
    fn type_id_prefix_inferred_from_datum_type() {
        let skill = make_datum("bayesian", Some(DatumType::Skill));
        let mcp   = make_datum("context7", Some(DatumType::Mcp));
        let cli   = make_datum("fdfind",   Some(DatumType::Cli));
        assert!(skill.type_id().starts_with("skill_"), "skill datum must have 'skill_' prefix");
        assert!(mcp.type_id().starts_with("mcp_"),   "mcp datum must have 'mcp_' prefix");
        assert!(cli.type_id().starts_with("cli_"),   "cli datum must have 'cli_' prefix");
    }

    #[test]
    fn type_id_different_names_produce_different_ids() {
        let a = make_datum("bayesian",     Some(DatumType::Skill));
        let b = make_datum("first-principles", Some(DatumType::Skill));
        assert_ne!(a.type_id(), b.type_id());
    }

    #[test]
    fn type_id_different_types_produce_different_ids() {
        let as_skill = make_datum("kaizen", Some(DatumType::Skill));
        let as_role  = make_datum("kaizen", Some(DatumType::Role));
        assert_ne!(as_skill.type_id(), as_role.type_id());
    }

    #[test]
    fn type_id_unknown_type_falls_back_to_dat_prefix() {
        let d = make_datum("mystery", None);
        assert!(d.type_id().starts_with("dat_"), "unknown type must fall back to 'dat_' prefix");
    }

    #[test]
    fn datum_type_prefix_inferred_not_hardcoded() {
        // base_suffix() → trim '.' → type_prefix() — all variants must round-trip
        for variant in DatumType::all_variants() {
            let prefix = variant.type_prefix();
            let suffix = variant.base_suffix().trim_start_matches('.');
            assert_eq!(prefix, suffix, "{variant:?}: type_prefix must equal base_suffix without leading dot");
        }
    }

    #[test]
    fn datum_nodes_covers_all_variants() {
        let nodes = DatumType::datum_nodes();
        let variant_count = DatumType::all_variants().len();
        assert_eq!(nodes.len(), variant_count, "datum_nodes() must emit one node per DatumType variant");
    }

    #[test]
    fn datum_nodes_no_duplicate_ids() {
        let nodes = DatumType::datum_nodes();
        let mut ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for n in &nodes {
            assert!(ids.insert(n.id.as_str()), "duplicate datum_node id: {}", n.id);
        }
    }

    #[test]
    fn datum_nodes_id_format_and_kind() {
        for node in DatumType::datum_nodes() {
            assert!(node.id.starts_with("datum_type::"), "id must be datum_type::<prefix>, got: {}", node.id);
            assert_eq!(node.kind, "datum_type", "kind must be 'datum_type', got: {}", node.kind);
            assert!(node.z_layer.is_none(), "datum_nodes must have no z_layer");
            assert!(node.semantic_type.is_none(), "datum_nodes must have no semantic_type");
        }
    }

    #[test]
    fn datum_nodes_label_matches_type_prefix() {
        for (node, variant) in DatumType::datum_nodes().iter().zip(DatumType::all_variants().iter()) {
            assert_eq!(node.label, variant.type_prefix(), "label must equal type_prefix for {variant:?}");
            assert_eq!(node.id, format!("datum_type::{}", variant.type_prefix()), "id format mismatch for {variant:?}");
        }
    }

    // ── Datum dispatch resolution tests ──────────────────────────────────

    use super::{DatumDispatch, resolve_datum_dispatch, resolve_all_datum_dispatches};

    fn write_cli_datum(dir: &std::path::Path, name: &str, command: &str) {
        let toml = format!(
            "[b00t]\nname = \"{name}\"\ntype = \"cli\"\ncommand = \"{command}\"\n"
        );
        std::fs::write(dir.join(format!("{name}.cli.toml")), toml).unwrap();
    }

    fn write_runtime_datum(dir: &std::path::Path, name: &str, binary: &str) {
        let toml = format!(
            "[b00t]\nname = \"{name}\"\ntype = \"runtime\"\n\n[b00t.runtime]\nbinary = \"{binary}\"\n"
        );
        std::fs::write(dir.join(format!("{name}.runtime.toml")), toml).unwrap();
    }

    fn write_polyseme_datum(dir: &std::path::Path, name: &str) {
        let toml = format!(
            "[b00t]\nname = \"{name}\"\ntype = \"polyseme\"\n\n\
             [[b00t.polyseme.refs]]\nname = \"{name}-example\"\n\
             canonical = \"github:example/{name}\"\n\
             datum = \"{name}-example.cli\"\n\
             description = \"example {name}\"\n"
        );
        std::fs::write(dir.join(format!("{name}.polyseme.toml")), toml).unwrap();
    }

    #[test]
    fn resolve_cli_datum_returns_passthrough() {
        let dir = tempfile::tempdir().unwrap();
        write_cli_datum(dir.path(), "testcli", "echo");
        let result = resolve_datum_dispatch("testcli", dir.path().to_str().unwrap());
        assert!(result.is_some());
        match result.unwrap() {
            DatumDispatch::CliPassthrough { command, .. } => assert_eq!(command, "echo"),
            _ => panic!("expected CliPassthrough"),
        }
    }

    #[test]
    fn resolve_runtime_datum_returns_runtime() {
        let dir = tempfile::tempdir().unwrap();
        write_runtime_datum(dir.path(), "testrt", "/bin/true");
        let result = resolve_datum_dispatch("testrt", dir.path().to_str().unwrap());
        assert!(result.is_some());
        assert!(matches!(result.unwrap(), DatumDispatch::Runtime(_)));
    }

    #[test]
    fn resolve_polyseme_returns_refs() {
        let dir = tempfile::tempdir().unwrap();
        write_polyseme_datum(dir.path(), "testpoly");
        let result = resolve_datum_dispatch("testpoly", dir.path().to_str().unwrap());
        assert!(result.is_some());
        match result.unwrap() {
            DatumDispatch::Polyseme { refs, .. } => assert_eq!(refs.len(), 1),
            _ => panic!("expected Polyseme"),
        }
    }

    #[test]
    fn resolve_missing_datum_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = resolve_datum_dispatch("__nonexistent__", dir.path().to_str().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn package_install_spec_generates_multi_os_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(r#"install = { package = "mold" }"#).unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'mold'"));
        assert!(command.contains("sudo apt-get install -y 'mold'"));
        assert!(command.contains("sudo dnf install -y 'mold'"));
        assert!(command.contains("sudo pacman -S --needed --noconfirm 'mold'"));
        assert!(command.contains("brew install 'mold'"));
    }

    #[test]
    fn package_install_spec_supports_package_manager_overrides() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum =
            toml::from_str(r#"install = { package = "fd", binary = "fdfind", apt = "fd-find" }"#)
                .unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'fdfind'"));
        assert!(command.contains("sudo apt-get install -y 'fd-find'"));
        assert!(command.contains("sudo dnf install -y 'fd'"));
    }

    #[test]
    fn tool_install_spec_generates_cargo_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(r#"install = { cargo = "eureka" }"#).unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'eureka'"));
        assert!(command.contains("command -v cargo"));
        assert!(command.contains("cargo install 'eureka'"));
    }

    #[test]
    fn tool_install_spec_generates_go_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(
            r#"install = { go = "github.com/go-task/task/v3/cmd/task@latest" }"#,
        )
        .unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'task'"));
        assert!(command.contains("command -v go"));
        assert!(command.contains("GO111MODULE=on go install 'github.com/go-task/task/v3/cmd/task@latest'"));
    }

    #[test]
    fn tool_install_spec_generates_npm_global_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(
            r#"install = { npm_global = "@google/gemini-cli", binary = "gemini" }"#,
        )
        .unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'gemini'"));
        assert!(command.contains("command -v npm"));
        assert!(command.contains("npm install -g '@google/gemini-cli'"));
    }

    #[test]
    fn tool_install_spec_generates_uv_tool_installer() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum = toml::from_str(r#"install = { uv_tool = "fastmcp" }"#).unwrap();
        let command = datum.install.command_string().unwrap();

        assert!(command.contains("command -v 'fastmcp'"));
        assert!(command.contains("command -v uv"));
        assert!(command.contains("uv tool install 'fastmcp'"));
    }

    #[test]
    fn install_metadata_is_not_parsed_as_tool_functor() {
        #[derive(Deserialize)]
        struct TestDatum {
            install: InstallSpec,
        }

        let datum: TestDatum =
            toml::from_str(r#"install = { requires = ["node", "npm"] }"#).unwrap();

        assert!(matches!(datum.install, InstallSpec::Metadata { .. }));
        assert!(datum.install.command_string().is_none());
    }

    #[test]
    fn resolve_prefers_runtime_over_cli() {
        let dir = tempfile::tempdir().unwrap();
        write_runtime_datum(dir.path(), "dual", "/bin/true");
        write_cli_datum(dir.path(), "dual", "echo");
        let result = resolve_datum_dispatch("dual", dir.path().to_str().unwrap());
        assert!(matches!(result.unwrap(), DatumDispatch::Runtime(_)));
    }

    #[test]
    fn resolve_all_returns_multiple_dispatches() {
        let dir = tempfile::tempdir().unwrap();
        write_cli_datum(dir.path(), "multi", "echo");
        write_polyseme_datum(dir.path(), "multi");
        let all = resolve_all_datum_dispatches("multi", dir.path().to_str().unwrap());
        assert!(all.len() >= 2);
        let has_cli = all.iter().any(|d| matches!(d, DatumDispatch::CliPassthrough { .. }));
        let has_poly = all.iter().any(|d| matches!(d, DatumDispatch::Polyseme { .. }));
        assert!(has_cli, "should have cli dispatch");
        assert!(has_poly, "should have polyseme dispatch");
    }
}
