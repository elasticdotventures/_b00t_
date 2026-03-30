// b00t-cli/src/inventory/mod.rs
// System state scanning: tools, MCPs, auth, hive profile

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Inventory {
    pub timestamp: String,
    pub hive: HiveState,
    pub tools: BTreeMap<String, Tool>,
    pub mcp_servers: BTreeMap<String, MCPServer>,
    pub auth: BTreeMap<String, AuthToken>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HiveState {
    pub profile: String,
    pub resources: Resources,
    pub services: Vec<Service>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resources {
    pub ram_total_gb: f64,
    pub ram_free_gb: f64,
    pub gpu_free_mb: f64,
    pub cpu_load: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tool {
    pub present: bool,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MCPStatus {
    Ready,
    Down,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MCPServer {
    pub present: bool,
    pub status: MCPStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configured: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthToken {
    pub present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Service {
    pub name: String,
    pub status: String, // "active" | "inactive" | "failed"
}

impl Default for Inventory {
    fn default() -> Self {
        Inventory {
            timestamp: Utc::now().to_rfc3339(),
            hive: HiveState {
                profile: String::new(),
                resources: Resources {
                    ram_total_gb: 0.0,
                    ram_free_gb: 0.0,
                    gpu_free_mb: 0.0,
                    cpu_load: 0.0,
                },
                services: vec![],
            },
            tools: BTreeMap::new(),
            mcp_servers: BTreeMap::new(),
            auth: BTreeMap::new(),
        }
    }
}

impl Inventory {
    /// Scan current system state
    pub fn scan() -> Result<Self, Box<dyn std::error::Error>> {
        let hive = Self::scan_hive()?;
        let tools = Self::detect_tools()?;
        let mcp_servers = Self::detect_mcp_servers()?;
        let auth = Self::detect_auth()?;

        Ok(Inventory {
            timestamp: Utc::now().to_rfc3339(),
            hive,
            tools,
            mcp_servers,
            auth,
        })
    }

    /// Scan hive status via b00t CLI
    fn scan_hive() -> Result<HiveState, Box<dyn std::error::Error>> {
        use std::process::Command;

        let output = Command::new("b00t")
            .args(&["hive", "status", "--json"])
            .output()?;

        if !output.status.success() {
            return Err("b00t hive status failed".into());
        }

        let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;

        Ok(HiveState {
            profile: json["profile"].as_str().unwrap_or("unknown").to_string(),
            resources: Resources {
                ram_total_gb: json["resources"]["ram_total_gb"].as_f64().unwrap_or(0.0),
                ram_free_gb: json["resources"]["ram_free_gb"].as_f64().unwrap_or(0.0),
                gpu_free_mb: json["resources"]["gpu_free_mb"].as_f64().unwrap_or(0.0),
                cpu_load: json["resources"]["cpu_load"].as_f64().unwrap_or(0.0),
            },
            services: json["services"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|s| Service {
                    name: s["name"].as_str().unwrap_or("").to_string(),
                    status: s["status"].as_str().unwrap_or("unknown").to_string(),
                })
                .collect(),
        })
    }

    /// Detect installed CLI tools
    fn detect_tools() -> Result<BTreeMap<String, Tool>, Box<dyn std::error::Error>> {
        use std::process::Command;

        let mut tools = BTreeMap::new();
        let tool_names = vec!["bash", "rust", "python", "git", "b00t-cli"];

        for name in tool_names {
            let output = Command::new("which").arg(name).output();

            if let Ok(out) = output {
                if out.status.success() {
                    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    let version = Self::get_version(name).unwrap_or_default();

                    tools.insert(
                        name.to_string(),
                        Tool {
                            present: true,
                            version,
                            path: Some(path),
                        },
                    );
                } else {
                    tools.insert(
                        name.to_string(),
                        Tool {
                            present: false,
                            version: String::new(),
                            path: None,
                        },
                    );
                }
            }
        }

        Ok(tools)
    }

    /// Get tool version string
    fn get_version(tool: &str) -> Result<String, Box<dyn std::error::Error>> {
        use std::process::Command;

        let output = match tool {
            "bash" => Command::new("bash").arg("--version").output()?,
            "rust" => Command::new("rustc").arg("--version").output()?,
            "python" => Command::new("python3").arg("--version").output()?,
            "git" => Command::new("git").arg("--version").output()?,
            "b00t-cli" => Command::new("b00t-cli").arg("--version").output()?,
            _ => return Ok(String::new()),
        };

        let version = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        Ok(version)
    }

    /// Detect MCP servers
    fn detect_mcp_servers() -> Result<BTreeMap<String, MCPServer>, Box<dyn std::error::Error>> {
        use std::process::Command;

        let mut servers = BTreeMap::new();
        let mcp_names = vec!["context7", "b00t-mcp"];

        for name in mcp_names {
            let output = Command::new("which").arg(name).output();

            let present = output.is_ok() && output.as_ref().unwrap().status.success();

            servers.insert(
                name.to_string(),
                MCPServer {
                    present,
                    status: if present {
                        MCPStatus::Ready
                    } else {
                        MCPStatus::Down
                    },
                    configured: None,
                },
            );
        }

        Ok(servers)
    }

    /// Detect authentication tokens (check presence, not values)
    fn detect_auth() -> Result<BTreeMap<String, AuthToken>, Box<dyn std::error::Error>> {
        let mut auth = BTreeMap::new();

        // Check for anthropic token
        auth.insert(
            "anthropic".to_string(),
            AuthToken {
                present: std::env::var("ANTHROPIC_API_KEY").is_ok(),
            },
        );

        // Check for github token
        let github_present = std::env::var("GITHUB_TOKEN").is_ok()
            || std::path::Path::new(&format!(
                "{}/.ssh/id_rsa",
                std::env::var("HOME").unwrap_or_default()
            ))
            .exists();

        auth.insert("github".to_string(), AuthToken { present: github_present });

        Ok(auth)
    }

    /// Report missing blessings based on what's not in inventory
    pub fn missing_blessings(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if self.mcp_servers.is_empty() {
            missing.push("blessing:mcp-ecosystem".to_string());
        }

        if !self.auth.get("anthropic").map(|a| a.present).unwrap_or(false) {
            missing.push("blessing:anthropic-auth".to_string());
        }

        missing
    }
}

/// Point-in-time snapshot of system inventory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySnapshot {
    pub timestamp: String,
    pub inventory: Inventory,
}

impl InventorySnapshot {
    pub fn capture() -> Result<Self, Box<dyn std::error::Error>> {
        let inventory = Inventory::scan()?;
        Ok(InventorySnapshot {
            timestamp: Utc::now().to_rfc3339(),
            inventory,
        })
    }
}

#[cfg(test)]
mod tests;
