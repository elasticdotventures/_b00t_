use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};

pub mod datum_ai;
pub mod datum_apt;
pub mod datum_bash;
pub mod datum_docker;
pub mod datum_k8s;
pub mod datum_vscode;
pub mod k8s;
pub mod traits;
pub mod utils;
pub use traits::*;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
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

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct BootDatum {
    pub name: String,
    #[serde(rename = "type")]
    pub datum_type: Option<DatumType>,
    pub desires: Option<String>,
    pub hint: String,

    pub install: Option<String>,
    pub update: Option<String>,
    pub version: Option<String>,
    pub version_regex: Option<String>,

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

    // Environment variables
    pub env: Option<std::collections::HashMap<String, String>>,

    // Require constraints
    pub require: Option<Vec<String>>,

    // Aliases for CLI commands
    pub aliases: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DatumType {
    Unknown,
    Mcp,
    Bash,
    Vscode,
    Docker,
    K8s,
    Apt,
    Nix,
    Ai,
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

    let (mut cleaned_input, mut first_comment) = (String::new(), None);

    for line in input.lines() {
        if let Some(cap) = comment_re.find(line) {
            if first_comment.is_none() {
                first_comment = Some(cap.as_str().trim_start_matches("//").trim().to_string());
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

    (cleaned_input.trim().to_string(), first_comment)
}

pub fn clean_json_for_dwiw(input: &str) -> String {
    extract_comments_and_clean_json(input).0
}

pub fn normalize_mcp_json(input: &str, dwiw: bool) -> Result<BootDatum> {
    let (cleaned_input, hint) = if dwiw {
        extract_comments_and_clean_json(input)
    } else {
        (input.to_string(), None)
    };

    let json_value: serde_json::Value = serde_json::from_str(&cleaned_input)?;

    // Handle direct format: {"name": "...", "command": "...", "args": [...]}
    if let Some(name) = json_value.get("name") {
        let server = BootDatum {
            name: name.as_str().unwrap_or("unknown").to_string(),
            datum_type: Some(DatumType::Mcp),
            desires: None,
            hint: hint.clone().unwrap_or_else(|| "MCP server".to_string()),
            install: None,
            update: None,
            version: None,
            version_regex: None,
            command: Some(
                json_value
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("npx")
                    .to_string(),
            ),
            args: Some(
                json_value
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_else(|| vec![]),
            ),
            vsix_id: None,
            script: None,
            image: None,
            docker_args: None,
            oci_uri: None,
            resource_path: None,
            chart_path: None,
            namespace: None,
            values_file: None,
            keywords: None,
            package_name: None,
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
            aliases: None,
        };
        return Ok(server);
    }

    // Handle mcpServers wrapper format: {"mcpServers": {"server_name": {...}}}
    if let Some(mcp_servers) = json_value.get("mcpServers") {
        let keys: Vec<_> = mcp_servers
            .as_object()
            .map(|obj| obj.keys().collect())
            .unwrap_or_default();

        if keys.len() == 1 {
            let server_name = &keys[0];
            let server_config = &mcp_servers[server_name];

            let server = BootDatum {
                name: server_name.to_string(),
                datum_type: Some(DatumType::Mcp),
                desires: None,
                hint: hint.clone().unwrap_or_else(|| "MCP server".to_string()),
                install: None,
                update: None,
                version: None,
                version_regex: None,
                command: Some(
                    server_config
                        .get("command")
                        .and_then(|v| v.as_str())
                        .unwrap_or("npx")
                        .to_string(),
                ),
                args: Some(
                    server_config
                        .get("args")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str())
                                .map(|s| s.to_string())
                                .collect()
                        })
                        .unwrap_or_else(|| vec![]),
                ),
                vsix_id: None,
                script: None,
                image: None,
                docker_args: None,
                oci_uri: None,
                resource_path: None,
                chart_path: None,
                namespace: None,
                values_file: None,
                keywords: None,
                package_name: None,
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
                aliases: None,
            };
            return Ok(server);
        }
    }

    // Handle single server format: {"server_name": {...}}
    let keys: Vec<_> = json_value
        .as_object()
        .map(|obj| obj.keys().collect())
        .unwrap_or_default();

    if keys.len() == 1 {
        let server_name = &keys[0];
        let server_config = &json_value[server_name];

        let server = BootDatum {
            name: server_name.to_string(),
            datum_type: Some(DatumType::Mcp),
            desires: None,
            hint: hint.unwrap_or_else(|| "MCP server".to_string()),
            install: None,
            update: None,
            version: None,
            version_regex: None,
            command: Some(
                server_config
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("npx")
                    .to_string(),
            ),
            args: Some(
                server_config
                    .get("args")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(|s| s.to_string())
                            .collect()
                    })
                    .unwrap_or_else(|| vec![]),
            ),
            vsix_id: None,
            script: None,
            image: None,
            docker_args: None,
            oci_uri: None,
            resource_path: None,
            chart_path: None,
            namespace: None,
            values_file: None,
            keywords: None,
            package_name: None,
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
            aliases: None,
        };
        return Ok(server);
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

impl std::fmt::Display for DatumType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DatumType::Unknown => write!(f, "unknown"),
            DatumType::Mcp => write!(f, "MCP"),
            DatumType::Bash => write!(f, "bash"),
            DatumType::Vscode => write!(f, "VSCode"),
            DatumType::Docker => write!(f, "docker"),
            DatumType::K8s => write!(f, "k8s"),
            DatumType::Apt => write!(f, "apt"),
            DatumType::Nix => write!(f, "nix"),
            DatumType::Ai => write!(f, "AI"),
        }
    }
}

impl DatumType {
    pub fn from_filename_extension(filename: &str) -> DatumType {
        if filename.ends_with(".cli.toml") {
            DatumType::Unknown
        } else if filename.ends_with(".mcp.toml") {
            DatumType::Mcp
        } else if filename.ends_with(".bash.toml") {
            DatumType::Bash
        } else if filename.ends_with(".vscode.toml") {
            DatumType::Vscode
        } else if filename.ends_with(".docker.toml") {
            DatumType::Docker
        } else if filename.ends_with(".k8s.toml") {
            DatumType::K8s
        } else if filename.ends_with(".apt.toml") {
            DatumType::Apt
        } else if filename.ends_with(".nix.toml") {
            DatumType::Nix
        } else if filename.ends_with(".ai.toml") {
            DatumType::Ai
        } else {
            DatumType::Unknown // Default fallback for .toml files
        }
    }
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
    Ok(std::path::PathBuf::from(
        shellexpand::tilde(path).to_string(),
    ))
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
    // Try different file extensions in order of preference
    let extensions = [
        ".cli.toml",
        ".mcp.toml",
        ".vscode.toml",
        ".docker.toml",
        ".apt.toml",
        ".nix.toml",
        ".bash.toml",
        ".toml",
    ];

    let mut path_buf = std::path::PathBuf::new();
    path_buf.push(shellexpand::tilde(path).to_string());

    for ext in &extensions {
        let filename = format!("{}{}", command, ext);
        path_buf.set_file_name(&filename);
        if path_buf.exists() {
            let content = std::fs::read_to_string(&path_buf)?;
            let config: UnifiedConfig = toml::from_str(&content)?;
            return Ok((config, filename));
        }
    }

    eprintln!("{} UNDEFINED", command);
    std::process::exit(100);
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
