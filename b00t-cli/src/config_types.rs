use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::BootDatum;

// ── Small configuration / data structs ──────────────────────────────────

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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub service_contract: Vec<ServiceContract>,
    pub env: Option<std::collections::HashMap<String, String>>,
    #[serde(default)]
    pub sections: Option<std::collections::HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct ServiceContract {
    pub capability: String,
    pub handler: String,
    pub evidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verifiable: Option<bool>,
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

// ── InstallSpec types ───────────────────────────────────────────────────

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

// ── Datum sub-configs ───────────────────────────────────────────────────

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
    pub check_command: Option<String>,
    /// Human-readable source description.
    pub version_source: Option<String>,
    /// Regex to extract semver from check_command output.
    pub check_regex: Option<String>,
}

// ── Runtime sandbox types ───────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct MountEntry {
    #[serde(default)]
    pub src: String,
    pub dest: String,
    #[serde(rename = "type")]
    pub mount_type: String,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct SeccompProfile {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub default_action: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct IsolationConfig {
    pub mounts: Option<Vec<MountEntry>>,
    pub share_net: Option<bool>,
    pub share_ipc: Option<bool>,
    pub share_pid: Option<bool>,
    pub share_uts: Option<bool>,
    pub new_session: Option<bool>,
    pub seccomp: Option<SeccompProfile>,
    pub caps_retain: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct RuntimeConfig {
    pub binary: String,
    pub args: Option<Vec<String>>,
    pub workdir: Option<String>,
    pub isolation: Option<IsolationConfig>,
    pub hook_pre: Option<String>,
    pub hook_post: Option<String>,
}

// ── MCP method types ───────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct McpMethods {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdio: Option<Vec<std::collections::HashMap<String, serde_json::Value>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub httpstream: Option<std::collections::HashMap<String, serde_json::Value>>,
}

// ── AI provisioning types ──────────────────────────────────────────────
// 🤓 Generic "this app needs an AI-backend credential" declaration. Any datum
//    that sets [b00t.ai_provision] gets a b00t-server API key minted and
//    injected as env at MCP-install time (see dispatch.rs's install_mcp
//    functions) — not rust-doc-specific, any future consumer opts in the
//    same way. scope is an ontology-class:action string b00t-server already
//    understands (b00t-mcp/src/server_llm.rs::ClassPermission::parse), e.g.
//    "b00t:EmbeddingModel:execute" or "b00t:ChatModel:execute".

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct AiProvisionConfig {
    pub scope: String,
    #[serde(default = "default_inject_key_as")]
    pub inject_key_as: String,
    #[serde(default = "default_inject_base_as")]
    pub inject_base_as: String,
    #[serde(default = "default_server_url")]
    pub server_url: String,
}

fn default_inject_key_as() -> String {
    "OPENAI_API_KEY".to_string()
}

fn default_inject_base_as() -> String {
    "OPENAI_API_BASE".to_string()
}

fn default_server_url() -> String {
    "http://127.0.0.1:5273/v1".to_string()
}

// ── Justfile types ─────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct JustfileCapabilities {
    pub network: Option<bool>,
    pub filesystem: Option<Vec<String>>,
    pub env_vars: Option<Vec<String>>,
    pub secrets: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct JustfileConfig {
    pub path: Option<String>,
    pub mcp_server: Option<String>,
    pub role_pattern: Option<String>,
    pub recipe_groups: Option<Vec<String>>,
    pub sandbox: Option<String>,
    pub allowed_sandboxes: Option<Vec<String>>,
    pub allow_side_effects: Option<bool>,
    pub capabilities: Option<JustfileCapabilities>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct PipelineCapabilities {
    pub network: Option<bool>,
    pub filesystem: Option<Vec<String>>,
    pub env_vars: Option<Vec<String>>,
    pub secrets: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
pub struct PipelineConfig {
    pub path: Option<String>,
    pub mcp_server: Option<String>,
    pub stages: Option<Vec<String>>,
    pub sandbox: Option<String>,
    pub allowed_sandboxes: Option<Vec<String>>,
    pub capabilities: Option<PipelineCapabilities>,
}

// ── MCP list types ─────────────────────────────────────────────────────

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

// ── AI config types ────────────────────────────────────────────────────

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

// ── Session state types ────────────────────────────────────────────────

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
