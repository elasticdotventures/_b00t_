use std::collections::HashSet;
use std::sync::OnceLock;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    ComposeConfig, DatumType, GateSpec, InstallSpec, JustfileConfig, K0mmand3rDatumConfig,
    KnowledgeConfig, LearnMeta, MaintenanceConfig, McpMethods, OrchestrationConfig, PipelineConfig,
    PolysemeConfig, RuntimeConfig, UsageExample,
};

// warn-once registry — one warning per unknown datum type string per process
static DATUM_TYPE_WARNED: OnceLock<std::sync::Mutex<HashSet<String>>> = OnceLock::new();

/// Returns true if the value is a well-known content tag (not a typed datum).
fn is_known_content_tag(s: &str) -> bool {
    matches!(
        s,
        "okr"
            | "prd"
            | "pattern"
            | "datum"
            | "reference"
            | "learn"
            | "hardware"
            | "tomllmd"
            | "specification"
            | "topic"
            | "soul"
            | "install"
            | "github_org"
            | "ai_provider"
            | "pyinfra"
            | "wow"
    )
}

/// Load incubating datum types from a runtime‑defined datum.
fn get_incubating_set() -> &'static HashSet<String> {
    static SET: OnceLock<HashSet<String>> = OnceLock::new();
    SET.get_or_init(|| {
        let base_path =
            std::env::var("_B00T_Path").unwrap_or_else(|_| "~/.b00t/_b00t_".to_string());
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
            let resolved =
                DatumType::from_type_token(&value).or_else(|| handle_incubating_type(&value));

            if resolved.is_none()
                && !is_known_content_tag(&value)
                && !get_incubating_set().contains(&value)
                && std::env::var("B00T_DATUM_WARN")
                    .map(|v| v != "0")
                    .unwrap_or(true)
            {
                let warned =
                    DATUM_TYPE_WARNED.get_or_init(|| std::sync::Mutex::new(HashSet::new()));
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

    // Competency cross-reference (issue #710): when set, prove_by_type() also
    // requires evidence::prove_skill(skill) to return at least one record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requires_competency: Option<String>,

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
    /// Type identity string: `{type_prefix}_{name}`.
    ///
    /// Deterministic — same name + same DatumType always produces the same ID.
    /// Known types get `skill_`, `mcp_`, `cli_`, etc.  Unknown (None) gets `dat_`.
    pub fn type_id(&self) -> String {
        let prefix = self
            .datum_type
            .as_ref()
            .map(|dt| dt.type_prefix())
            .unwrap_or("dat");
        format!("{}_{}", prefix, self.name)
    }

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

/// Scans `dir` (non-recursive) for datum files and loads them into a
/// `{name}.{type_prefix}` -> `BootDatum` map. The single shared
/// implementation of the scan itself for `install`/`uninstall`/`stack`/
/// `cli` — previously each of those four commands hand-rolled its own
/// near-identical copy, which had silently drifted: three copies derived
/// the type-name key via `Debug`+lowercase (wrong for multi-word variants —
/// `HiveProfile` became `"hiveprofile"`, not matching any real file's
/// `.hive` suffix convention) while a fourth used an ad-hoc serde-snake_case
/// guess (also wrong: `"hive_profile"`, still not `.hive`).
/// `DatumType::type_prefix()` is the actual documented single source of
/// truth (auto-derived from the same `datum_type_table!` macro that defines
/// each type's real file suffix), so it's the only correct choice here.
///
/// Recognizes `.toml`, `.tomllm`, and `.tomllmd` as datum file extensions —
/// all three are real, currently-used extensions in this repo (e.g.
/// `_b00t_/datums/*.tomllmd`); a plain `.ends_with(".toml")` check silently
/// excludes the latter two, since `"foo.tomllmd"` does not end with the
/// literal substring `".toml"`.
///
/// Deliberately takes an already-resolved `&Path`, not a raw path string —
/// the four callers resolved their base directory differently before this
/// consolidation (three used plain tilde expansion; `stack.rs` used
/// `lifecycle::get_expanded_path`'s legacy-directory fallback), and that
/// divergence predates this function and is each caller's own concern, not
/// this scan's. Centralizing path resolution too would have silently
/// changed behavior for whichever callers didn't already use the
/// fallback-aware version.
pub fn load_all_datums_from_dir(
    dir: &std::path::Path,
) -> anyhow::Result<std::collections::HashMap<String, BootDatum>> {
    let mut datums = std::collections::HashMap::new();

    if !dir.exists() {
        return Ok(datums);
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let entry_path = entry.path();

        if !entry_path.is_file() {
            continue;
        }
        let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if file_name.ends_with(".stack.toml") {
            continue;
        }
        if !(file_name.ends_with(".toml")
            || file_name.ends_with(".tomllm")
            || file_name.ends_with(".tomllmd"))
        {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&entry_path) else {
            continue;
        };
        let Ok(config) = toml::from_str::<crate::UnifiedConfig>(&content) else {
            continue;
        };
        let datum = config.b00t;
        let type_prefix = datum
            .datum_type
            .as_ref()
            .map(|t| t.type_prefix().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let key = format!("{}.{}", datum.name, type_prefix);
        datums.insert(key, datum);
    }

    Ok(datums)
}

/// Convenience wrapper matching the original (pre-consolidation) simple
/// tilde-expansion path resolution used by `install`/`uninstall`/`cli` —
/// same behavior those three callers already had. `stack.rs` does NOT use
/// this; it resolves via `lifecycle::get_expanded_path` (legacy-fallback
/// aware) and calls `load_all_datums_from_dir` directly, preserving its own
/// pre-existing behavior exactly.
pub fn load_all_datums(path: &str) -> anyhow::Result<std::collections::HashMap<String, BootDatum>> {
    let dir = std::path::PathBuf::from(shellexpand::tilde(path).to_string());
    load_all_datums_from_dir(&dir)
}

#[cfg(test)]
mod load_all_datums_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn finds_toml_tomllm_and_tomllmd_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        fs::write(
            temp_dir.path().join("alpha.cli.toml"),
            "[b00t]\nname = \"alpha\"\ntype = \"cli\"\nhint = \"t\"\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("bravo.cli.tomllm"),
            "[b00t]\nname = \"bravo\"\ntype = \"cli\"\nhint = \"t\"\n",
        )
        .unwrap();
        fs::write(
            temp_dir.path().join("charlie.cli.tomllmd"),
            "[b00t]\nname = \"charlie\"\ntype = \"cli\"\nhint = \"t\"\n",
        )
        .unwrap();

        let datums = load_all_datums(path).unwrap();
        assert_eq!(
            datums.len(),
            3,
            "expected all three extensions to be found, got: {:?}",
            datums.keys().collect::<Vec<_>>()
        );
        assert!(datums.contains_key("alpha.cli"));
        assert!(datums.contains_key("bravo.cli"));
        assert!(datums.contains_key("charlie.cli"));
    }

    #[test]
    fn uses_type_prefix_not_debug_lowercase_for_multiword_types() {
        // Regression test for the pre-consolidation divergence: HiveProfile's
        // real file-suffix convention is `.hive` (type_prefix()), not
        // Debug+lowercase's "hiveprofile" or an ad-hoc "hive_profile" guess.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        fs::write(
            temp_dir.path().join("mesh3d-batch.hive.toml"),
            "[b00t]\nname = \"mesh3d-batch\"\ntype = \"hive_profile\"\nhint = \"t\"\n",
        )
        .unwrap();

        let datums = load_all_datums(path).unwrap();
        assert!(
            datums.contains_key("mesh3d-batch.hive"),
            "expected key using type_prefix() ('hive'), got: {:?}",
            datums.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn skips_stack_toml_files() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();

        fs::write(
            temp_dir.path().join("mystack.stack.toml"),
            "[b00t]\nname = \"mystack\"\ntype = \"stack\"\nhint = \"t\"\n",
        )
        .unwrap();

        let datums = load_all_datums(path).unwrap();
        assert_eq!(datums.len(), 0);
    }

    #[test]
    fn empty_directory_returns_empty_map() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().to_str().unwrap();
        let datums = load_all_datums(path).unwrap();
        assert_eq!(datums.len(), 0);
    }
}
