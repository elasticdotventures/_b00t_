use anyhow::{Context, Result};
use rmcp::{
    handler::server::{ServerHandler, router::tool::ToolRouter, tool::Parameters},
    model::{
        CallToolResult, Content, Implementation, ProtocolVersion,
        ServerCapabilities, ServerInfo,
    },
    tool, tool_handler, tool_router,
    schemars::{self, JsonSchema},
};
use serde::{Deserialize, Serialize};
use serde_json;
use std::path::Path;
use std::process::Command;

use crate::acl::AclFilter;

// Parameter structs for tools
#[derive(Debug, Deserialize, JsonSchema)]
pub struct DetectParams {
    pub tool: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DesiresParams {
    pub tool: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LearnParams {
    pub topic: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct McpParams {
    pub action: String,
    pub json_config: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct B00tOutput {
    pub output: String,
    pub success: bool,
}

#[derive(Clone)]
pub struct B00tMcpServer {
    working_dir: std::path::PathBuf,
    acl_filter: AclFilter,
    tool_router: ToolRouter<Self>,
}

impl B00tMcpServer {
    pub fn new<P: AsRef<Path>>(working_dir: P, config_path: &str) -> Result<Self> {
        let acl_filter = AclFilter::load_from_file(config_path)
            .context("Failed to load ACL configuration")?;

        Ok(Self {
            working_dir: working_dir.as_ref().to_path_buf(),
            acl_filter,
            tool_router: Self::tool_router(),
        })
    }

    /// Execute b00t-cli command with arguments
    fn execute_b00t_command(&self, subcommand: &str, args: &[String]) -> Result<String> {
        // Check ACL first
        if !self.acl_filter.is_allowed(subcommand, args) {
            anyhow::bail!("Command '{}' with args {:?} is not allowed by ACL policy", subcommand, args);
        }

        let mut cmd = Command::new("b00t-cli");
        cmd.current_dir(&self.working_dir);
        cmd.arg(subcommand);
        
        for arg in args {
            cmd.arg(arg);
        }

        let output = cmd.output()
            .with_context(|| format!("Failed to execute b00t-cli {} {:?}", subcommand, args))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("b00t-cli command failed: {}", stderr);
        }
    }
}

#[tool_router]
impl B00tMcpServer {
    #[tool(description = "Detect currently installed version of a tool")]
    async fn b00t_detect(
        &self,
        Parameters(params): Parameters<DetectParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        match self.execute_b00t_command("detect", &[params.tool]) {
            Ok(output) => {
                let result = B00tOutput {
                    output,
                    success: true,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::success(vec![Content::text(content)]))
            },
            Err(e) => {
                let result = B00tOutput {
                    output: e.to_string(),
                    success: false,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }

    #[tool(description = "Show desired version of a tool from configuration")]
    async fn b00t_desires(
        &self,
        Parameters(params): Parameters<DesiresParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        match self.execute_b00t_command("desires", &[params.tool]) {
            Ok(output) => {
                let result = B00tOutput {
                    output,
                    success: true,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::success(vec![Content::text(content)]))
            },
            Err(e) => {
                let result = B00tOutput {
                    output: e.to_string(),
                    success: false,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }

    #[tool(description = "Show learning resources for a topic")]
    async fn b00t_learn(
        &self,
        Parameters(params): Parameters<LearnParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let args = if let Some(topic) = params.topic {
            vec![topic]
        } else {
            vec![]
        };

        match self.execute_b00t_command("learn", &args) {
            Ok(output) => {
                let result = B00tOutput {
                    output,
                    success: true,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::success(vec![Content::text(content)]))
            },
            Err(e) => {
                let result = B00tOutput {
                    output: e.to_string(),
                    success: false,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }

    #[tool(description = "Manage MCP servers (list or add only)")]
    async fn b00t_mcp(
        &self,
        Parameters(params): Parameters<McpParams>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        let args = if params.action == "add" {
            if let Some(json_config) = params.json_config {
                vec![params.action, json_config]
            } else {
                return Ok(CallToolResult::error(vec![Content::text(
                    "json_config required for 'add' action".to_string()
                )]));
            }
        } else {
            vec![params.action]
        };

        match self.execute_b00t_command("mcp", &args) {
            Ok(output) => {
                let result = B00tOutput {
                    output,
                    success: true,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::success(vec![Content::text(content)]))
            },
            Err(e) => {
                let result = B00tOutput {
                    output: e.to_string(),
                    success: false,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }

    #[tool(description = "Show status of all tools (equivalent to 'b00t .' command)")]
    async fn b00t_status(
        &self,
        _params: Parameters<()>,
    ) -> Result<CallToolResult, rmcp::model::ErrorData> {
        match self.execute_b00t_command(".", &[]) {
            Ok(output) => {
                let result = B00tOutput {
                    output,
                    success: true,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::success(vec![Content::text(content)]))
            },
            Err(e) => {
                let result = B00tOutput {
                    output: e.to_string(),
                    success: false,
                };
                let content = serde_json::to_string_pretty(&result).unwrap();
                Ok(CallToolResult::error(vec![Content::text(content)]))
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for B00tMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            server_info: Implementation::from_build_env(),
            instructions: Some("MCP server for b00t-cli command proxy with ACL filtering. Provides secure access to b00t-cli commands.".into()),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
        }
    }
}