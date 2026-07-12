use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    ComposeConfig, DatumType, GateSpec, InstallSpec, JustfileConfig, K0mmand3rDatumConfig,
    KnowledgeConfig, LearnMeta, MaintenanceConfig, McpMethods, OrchestrationConfig,
    PipelineConfig, PolysemeConfig, RuntimeConfig, UsageExample,
};

// warn-once registry — one warning per unknown datum type string per process
static DATUM_TYPE_WARNED: OnceLock<std::sync::Mutex<HashSet<String>>> = OnceLock::new();

/// Returns true if the value is a well-known content tag (not a typed datum).
fn is_known_content_tag(s: &str) -> bool {
    matches!(s, "okr" | "prd" | "pattern" | "datum" | "reference" | "learn" | "hardware" | "tomllmd"
        | "specification" | "topic" | "soul" | "install" | "github_org" | "ai_provider" | "pyinfra" | "wow")
}

/// Load incubating datum types from a runtime‑defined datum.
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

/// Handle datum types that are marked as *incubating*.
fn handle_incubating_type(value: &str) -> Option<DatumType> {
    let _ = value;
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
                    .get_or_init(|| std::sync::Mutex::new(HashSet::new()));
                if let Ok(mut set) = warned.lock() {
                    if set.insert(value.clone()) {
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
    pub resource_path: Option<String>,

    // K8s fields
    pub chart_path: Option<String>,
    pub namespace: Option<String>,
    pub values_file: Option<String>,

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

    // Slash-command orchestration metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub k0mmand3r: Option<K0mmand3rDatumConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub knowledge: Option<KnowledgeConfig>,

    // MCP-specific multi-method support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpMethods>,

    // Gate preconditions
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
    pub channel_prefix: Option<String>,

    // Dependency graph
    pub depends_on: Option<Vec<String>>,
    pub members: Option<Vec<String>>,

    // Classifier hints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trigger_words: Option<Vec<String>>,

    // Orchestration / stack / job / skill metadata
    pub orchestration: Option<OrchestrationConfig>,
    pub stack: Option<serde_json::Value>,

    // Model cache guard
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

    // Rhai hook scripts
    pub hook_detect: Option<String>,
    pub hook_install: Option<String>,
    pub hook_update: Option<String>,
    pub hook_learn: Option<String>,
    pub uninstall: Option<String>,
    pub hook_uninstall: Option<String>,

    // Blessing system: tool authorization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unlocks: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maintenance: Option<MaintenanceConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub polyseme: Option<PolysemeConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose: Option<ComposeConfig>,
    // 🚩 required_for_core is a transitional field — used during schema migration
    //     to mark datums that must exist for the core system to function.
    //     Remove after all datums have been audited.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_for_core: Option<bool>,
}

// Re-export ApiProvides from config_types (needed by BootDatum)
use crate::ApiProvides;

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
