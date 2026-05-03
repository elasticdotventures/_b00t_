use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::utils::get_workspace_root;
use crate::{BootDatum, get_mcp_config};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GeminiExtension {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<HashMap<String, McpServerConfig>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tools: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tools: Option<Vec<String>>,
}

impl From<BootDatum> for McpServerConfig {
    fn from(datum: BootDatum) -> Self {
        McpServerConfig {
            command: datum.command.unwrap_or_default(),
            args: datum.args,
            env: None,           // Not yet supported in b00t-cli BootDatum
            cwd: None,           // Not yet supported in b00t-cli BootDatum
            timeout: None,       // Not yet supported in b00t-cli BootDatum
            trust: None,         // Not yet supported in b00t-cli BootDatum
            include_tools: None, // Not yet supported in b00t-cli BootDatum
            exclude_tools: None, // Not yet supported in b00t-cli BootDatum
        }
    }
}

fn get_gemini_extensions_dir(is_repo: bool) -> Result<PathBuf> {
    let path = if is_repo {
        let workspace_root = get_workspace_root();
        PathBuf::from(workspace_root).join(".gemini/extensions")
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".gemini/extensions")
    };
    Ok(path)
}

pub fn gemini_install_mcp(name: &str, b00t_path: &str, is_repo: bool) -> Result<()> {
    let mcp_datum = get_mcp_config(name, b00t_path)?;
    let extensions_dir = get_gemini_extensions_dir(is_repo)?;
    let extension_dir = extensions_dir.join("b00t");
    fs::create_dir_all(&extension_dir)?;

    let extension_file_path = extension_dir.join("gemini-extension.json");

    let mut extension = if extension_file_path.exists() {
        let content = fs::read_to_string(&extension_file_path)?;
        serde_json::from_str::<GeminiExtension>(&content)?
    } else {
        GeminiExtension {
            name: "b00t".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            mcp_servers: Some(HashMap::new()),
            context_file_name: Some("GEMINI.md".to_string()),
            exclude_tools: None,
        }
    };

    let mcp_servers = extension.mcp_servers.get_or_insert_with(HashMap::new);
    mcp_servers.insert(name.to_string(), mcp_datum.into());

    let json_str = serde_json::to_string_pretty(&extension)?;
    fs::write(&extension_file_path, json_str)?;

    println!(
        "Successfully installed MCP server '{}' to Gemini CLI extension at {}",
        name,
        extension_file_path.display()
    );

    Ok(())
}
