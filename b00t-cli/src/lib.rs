use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

pub mod ansible;
pub mod bootstrap;
pub mod budget_controller;
pub mod cloud_sync;
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
pub mod datum_gemini;
pub mod datum_job;
pub mod datum_k8s;
pub mod datum_mcp;
pub mod datum_repo;
pub mod datum_skill;
pub mod datum_stack;
pub mod datum_utils;
pub mod datum_vscode;
pub mod dependency_resolver;
pub mod entanglement;
pub mod erp;
pub mod hive;
pub mod hook_engine;
pub mod job_executor;
pub mod job_ipc;
pub mod job_state;
pub mod k8s;
pub mod memory_provider;
pub mod model_manager;
pub mod orchestrator;
pub mod session_memory;
pub mod traits;
pub mod utils;
pub mod whoami;
pub use traits::*;

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

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct BootDatum {
    pub name: String,
    #[serde(rename = "type")]
    pub datum_type: Option<DatumType>,
    pub desires: Option<String>,
    #[serde(default)]
    pub auto_install: Option<bool>,
    pub hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance: Option<Vec<String>>,

    pub install: Option<String>,
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

    // MCP-specific multi-method support - these will be handled by datum_mcp module
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp: Option<McpMethods>,

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

    // Dependency graph
    pub depends_on: Option<Vec<String>>,
    pub members: Option<Vec<String>>,

    // Orchestration / stack / job / skill metadata
    pub orchestration: Option<OrchestrationConfig>,
    pub stack: Option<serde_json::Value>,
    pub job: Option<serde_json::Value>,
    pub skill: Option<serde_json::Value>,

    // Database connection
    pub dsn: Option<String>,

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
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct McpMethods {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdio: Option<Vec<std::collections::HashMap<String, serde_json::Value>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub httpstream: Option<std::collections::HashMap<String, serde_json::Value>>,
}

/// DatumType — b00t's typed datum registry.
/// 🤓 Generated by `tomllm::define_typed_registry!` — add new types here ONLY.
///    Provides: enum, Display, serde, base_suffix(), all_base_suffixes(), from_filename(), TomllmRegistry impl.
tomllm::define_typed_registry! {
    pub enum DatumType {
        //  Variant      base suffix     serde name         display
        Database    => ".database"  [ serde="database",     display="database"    ],
        HiveProfile => ".hive"      [ serde="hive_profile", display="hive_profile" ],
        Agent       => ".agent"     [ serde="agent",        display="agent"       ],
        Config      => ".config"    [ serde="config",       display="config"      ],
        Docker      => ".docker"    [ serde="docker",       display="docker"      ],
        Skill       => ".skill"     [ serde="skill",        display="skill"       ],
        Stack       => ".stack"     [ serde="stack",        display="stack"       ],
        Repo        => ".repo"      [ serde="repo",         display="repo"        ],
        Role        => ".role"      [ serde="role",         display="role"        ],
        Bash        => ".bash"      [ serde="bash",         display="bash"        ],
        Vscode      => ".vscode"    [ serde="vscode",       display="VSCode"      ],
        K8s         => ".k8s"       [ serde="k8s",          display="k8s"         ],
        Apt         => ".apt"       [ serde="apt",          display="apt"         ],
        Nix         => ".nix"       [ serde="nix",          display="nix"         ],
        Mcp         => ".mcp"       [ serde="mcp",          display="MCP"         ],
        Cli         => ".cli"       [ serde="cli",          display="CLI"         ],
        Api         => ".api"       [ serde="api",          display="api"         ],
        Job         => ".job"       [ serde="job",          display="job"         ],
        AiModel     => ".ai_model"  [ serde="ai_model",     display="ai_model"    ],
        Ai          => ".ai"        [ serde="ai",           display="AI"          ],
    }
}

impl DatumType {
    /// Compat alias — prefer `from_filename()` from `TomllmRegistry` trait.
    #[inline]
    pub fn from_filename_extension(filename: &str) -> DatumType {
        DatumType::from_filename(filename)
    }
}

#[derive(Serialize, Debug)]
pub struct McpListOutput {
    pub servers: Vec<McpListItem>,
    pub path: String,
}

#[derive(Serialize, Debug)]
pub struct McpListItem {
    pub name: String,
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub hint: Option<String>,
    pub error: Option<String>,
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

    BootDatum {
        name,
        datum_type: Some(DatumType::Mcp),
        hint: hint.unwrap_or_else(|| "MCP server".to_string()),
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

            return Ok(BootDatum {
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
    };

    let toml_content = toml::to_string(&config).context("Failed to serialize config to TOML")?;

    // Use explicit datum_type or default to Unknown
    let datum_type = datum.datum_type.clone().unwrap_or(DatumType::Unknown);
    let suffix = match datum_type {
        DatumType::Mcp => ".mcp.toml",
        DatumType::Bash => ".bash.toml",
        DatumType::Vscode => ".vscode.toml",
        DatumType::Docker => ".docker.toml",
        DatumType::K8s => ".k8s.toml",
        DatumType::Apt => ".apt.toml",
        DatumType::Nix => ".nix.toml",
        DatumType::Ai => ".ai.toml",
        DatumType::AiModel => ".ai.toml",
        DatumType::Cli => ".cli.toml",
        DatumType::Api => ".api.toml",
        DatumType::Stack => ".stack.toml",
        DatumType::Job => ".job.toml",
        DatumType::Agent => ".agent.toml",
        DatumType::Config => ".config.toml",
        DatumType::Database => ".database.toml",
        DatumType::Repo => ".repo.toml",
        DatumType::Role => ".toml",
        DatumType::Skill => ".skill.toml",
        DatumType::HiveProfile => ".hive.toml",
        // 🤓 .tomllm = valid TOML + # comment annotations for tribal knowledge
        // tomllm crate strips comments for data pipelines; tail-map enables fast executive scanning
        DatumType::Unknown => ".toml",
    };

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
                .map(DatumType::from_filename_extension)
                .unwrap_or(DatumType::Unknown)
        })
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

    // Fallback to legacy ~/.dotfiles/_b00t_ if primary missing
    let legacy = PathBuf::from(shellexpand::tilde("~/.dotfiles/_b00t_").to_string());
    if legacy.exists() {
        if !WARNED_LEGACY.swap(true, Ordering::SeqCst) {
            eprintln!("⚠️ Using legacy b00t path at {}", legacy.display());
        }
        return Ok(legacy);
    }

    // Return the primary even if it doesn't exist to preserve prior behavior
    Ok(primary)
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
    use tomllm::loader::resolve_path;
    let expanded = shellexpand::tilde(path);
    let dir = std::path::Path::new(expanded.as_ref());

    // 🤓 DatumType::all_base_suffixes() is the single source of truth — generated by macro.
    //    resolve_path tries <base>.tomllm first, then <base>.toml.
    for base in DatumType::all_base_suffixes() {
        if let Some(resolved) = resolve_path(dir, command, base) {
            let filename = resolved
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let content = std::fs::read_to_string(&resolved)?;
            let config: UnifiedConfig = toml::from_str(&content)?;
            return Ok((config, filename));
        }
    }
    // fallback: plain .tomllm then .toml (Unknown type — no typed suffix)
    for ext in [".tomllm", ".toml"] {
        let plain = dir.join(format!("{}{}", command, ext));
        if plain.exists() {
            let filename = format!("{}{}", command, ext);
            let content = std::fs::read_to_string(&plain)?;
            let config: UnifiedConfig = toml::from_str(&content)?;
            return Ok((config, filename));
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

    let config: UnifiedConfig =
        toml::from_str(&content).context("Failed to parse MCP config TOML")?;

    Ok(config.b00t)
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

pub fn mcp_list(path: &str, json_output: bool) -> Result<()> {
    use anyhow::Context;

    let mcp_files = get_mcp_toml_files(path)?;
    let mut mcp_items = Vec::new();

    for server_name in mcp_files {
        match get_mcp_config(&server_name, path) {
            Ok(datum) => {
                // Extract command and args from MCP structure (prioritizing stdio methods)
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
                        // Fallback to legacy fields for backwards compatibility
                        (datum.command.clone(), datum.args.clone())
                    };

                mcp_items.push(McpListItem {
                    name: server_name,
                    command,
                    args,
                    hint: Some(datum.hint.clone()),
                    error: None,
                });
            }
            Err(e) => {
                mcp_items.push(McpListItem {
                    name: server_name,
                    command: None,
                    args: None,
                    hint: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    if json_output {
        let expanded_path = get_expanded_path(path)?;
        let output = McpListOutput {
            servers: mcp_items,
            path: expanded_path.display().to_string(),
        };
        let json_str = serde_json::to_string_pretty(&output)
            .context("Failed to serialize MCP list to JSON")?;
        println!("{}", json_str);
    } else {
        let expanded_path = get_expanded_path(path)?;
        if mcp_items.is_empty() {
            println!(
                "No MCP server configurations found in {}",
                expanded_path.display()
            );
            println!("Use 'b00t-cli mcp add <json>' to add MCP server configurations.");
        } else {
            println!("Available MCP servers in {}:", expanded_path.display());
            println!();
            for item in mcp_items {
                match (&item.command, &item.args) {
                    (Some(command), Some(args)) => {
                        println!("📋 {} ({})", item.name, command);
                        if !args.is_empty() {
                            println!("   args: {}", args.join(" "));
                        }
                    }
                    _ => {
                        println!("❌ {} (error reading config)", item.name);
                    }
                }
            }
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
    use_repo: bool,
    stdio_command: Option<&str>,
    use_httpstream: bool,
) -> Result<()> {
    use duct::cmd;

    let datum = get_mcp_config(name, path)?;
    let (command, args, env, method_type) =
        select_mcp_method(&datum, stdio_command, use_httpstream)?;

    let mut codex_config = if method_type == "httpstream" {
        serde_json::json!({ "url": command })
    } else {
        serde_json::json!({
            "command": command,
            "args": args
        })
    };

    if let Some(env_map) = env {
        if let Some(obj) = codex_config.as_object_mut() {
            obj.insert("env".to_string(), serde_json::to_value(env_map)?);
        }
    }

    let json_str =
        serde_json::to_string(&codex_config).context("Failed to serialize JSON for Codex")?;
    let location_flag = if use_repo { "--repo" } else { "--user" };
    let result = cmd!("codex", "mcp", "add-json", location_flag, name, &json_str).run();

    match result {
        Ok(_) => {
            let location = if use_repo {
                "repository"
            } else {
                "user global"
            };
            println!(
                "Successfully installed MCP server '{}' to Codex ({})",
                datum.name, location
            );
            println!(
                "Codex command: codex mcp add-json {} {} '{}'",
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
                "Failed to install MCP server to Codex ({}): {}",
                location, e
            );
            eprintln!(
                "Manual command: codex mcp add-json {} {} '{}'",
                location_flag, datum.name, json_str
            );
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

/// Push all repo .mcp.json servers into Codex CLI config via `codex mcp add-json`.
pub fn codex_sync_dotmcpjson(path: &str, use_repo: bool) -> Result<()> {
    use crate::utils::get_workspace_root;
    use duct::cmd;
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

    let location_flag = if use_repo { "--repo" } else { "--user" };
    let mut failures = Vec::new();

    for (name, config) in servers {
        let json_str = serde_json::to_string(config)
            .with_context(|| format!("Failed to serialize MCP server '{}'", name))?;
        if let Err(e) = cmd!("codex", "mcp", "add-json", location_flag, name, &json_str).run() {
            failures.push((name.clone(), e.to_string()));
        } else {
            println!("Codex synced '{}'", name);
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
    let expanded_path = get_expanded_path(path)?;

    if let Ok(entries) = std::fs::read_dir(&expanded_path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if let Some(file_name) = entry_path.file_name().and_then(|s| s.to_str()) {
                if file_name.ends_with(extension) {
                    if let Some(tool_name) = file_name.strip_suffix(extension) {
                        if let Ok(datum) = T::try_from((tool_name, path)) {
                            tools.push(Box::new(datum));
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
