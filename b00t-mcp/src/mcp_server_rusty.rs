use anyhow::Result;
use rmcp::{
    handler::server::ServerHandler,
    model::{
        CallToolResult, Implementation, ProtocolVersion,
        ServerCapabilities, ServerInfo, ListToolsResult,
        CallToolRequestParam, PaginatedRequestParam,
        Content, ErrorData as McpError,
    },
    service::{RequestContext, RoleServer},
};
use std::path::Path;
use std::collections::HashMap;
use tracing::{info, error, debug};

use crate::mcp_tools::create_mcp_registry;
use crate::clap_reflection::McpCommandRegistry;

/// Rusty b00t MCP server with compile-time generated tools
/// 
/// This replaces the brittle dynamic approach with proper Rust trait-based
/// compile-time tool generation that dtolnay would approve of.
#[derive(Clone)]
pub struct B00tMcpServerRusty {
    working_dir: std::path::PathBuf,
    registry: McpCommandRegistry,
}

impl B00tMcpServerRusty {
    pub fn new<P: AsRef<Path>>(working_dir: P, _config_path: &str) -> Result<Self> {
        let working_dir = working_dir.as_ref().to_path_buf();
        
        Ok(Self {
            working_dir,
            registry: create_mcp_registry(),
        })
    }
    
    /// Get the number of available tools
    pub fn tool_count(&self) -> usize {
        self.registry.get_tools().len()
    }
}

impl ServerHandler for B00tMcpServerRusty {
    async fn ping(
        &self,
        _context: RequestContext<RoleServer>,
    ) -> Result<(), McpError> {
        debug!("🏓 Ping received - Rusty MCP server is alive and well");
        
        // Log server health info for debugging
        let tools_count = self.registry.get_tools().len();
        debug!("🦀 Server status: {} compile-time tools available", tools_count);
        debug!("📁 Working directory: {}", self.working_dir.display());
        
        // Verify b00t-cli is available
        let b00t_cli_available = std::process::Command::new("b00t-cli")
            .arg("--help")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        
        debug!("🥾 b00t-cli availability: {}", if b00t_cli_available { "✅" } else { "❌" });
        
        if !b00t_cli_available {
            info!("⚠️  b00t-cli not available - MCP tools may fail to execute properly");
        }
        
        Ok(())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(), // Uses LATEST (2025-03-26)
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "🦀 Rusty MCP server for b00t-cli with compile-time generated tools. \
                 Features type-safe command dispatch, zero runtime parsing failures, \
                 and full CLAP structure synchronization.".into()
            ),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        debug!("🦀 list_tools called - using compile-time generated tools");
        
        let tools = self.registry.get_tools();
        
        info!("🦀 Generated {} compile-time tools from b00t-cli CLAP structures", tools.len());
        
        for tool in &tools {
            debug!("🔧 Tool: {}", tool.name);
        }
        
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_name = request.name.as_ref();
        
        // Convert request arguments to HashMap
        let params: HashMap<String, serde_json::Value> = request.arguments
            .unwrap_or_default()
            .into_iter()
            .collect();

        info!("🦀 Executing compile-time tool: {} with params: {:?}", tool_name, params);

        // Execute the command using the registry
        match self.registry.execute(tool_name, &params) {
            Ok(output) => {
                info!("✅ Successfully executed tool: {}", tool_name);
                Ok(self.create_success_result(&output))
            }
            Err(e) => {
                error!("❌ Failed to execute tool {}: {}", tool_name, e);
                Ok(self.create_error_result(&e.to_string()))
            }
        }
    }

    async fn on_initialized(&self, _context: rmcp::service::NotificationContext<rmcp::service::RoleServer>) {
        info!("🦀 Rusty b00t-mcp server initialized successfully");
        
        let tools = self.registry.get_tools();
        let tool_names: Vec<&str> = tools.iter()
            .map(|t| t.name.as_ref())
            .collect();
            
        info!("🦀 Available compile-time tools: {}", tool_names.join(", "));
        
        // Log some statistics
        info!("📊 Total tools: {}", tools.len());
        
        let tool_categories: HashMap<&str, usize> = tools.iter()
            .fold(HashMap::new(), |mut acc, tool| {
                let prefix = tool.name.as_ref().split('_').nth(1).unwrap_or("unknown");
                *acc.entry(prefix).or_insert(0) += 1;
                acc
            });
            
        for (category, count) in tool_categories {
            info!("📋 {} tools: {}", category, count);
        }
    }
}

impl B00tMcpServerRusty {
    /// Create successful MCP tool result
    fn create_success_result(&self, output: &str) -> CallToolResult {
        #[derive(serde::Serialize)]
        struct B00tOutput {
            output: String,
            success: bool,
            server_type: String,
            working_dir: String,
        }

        let result = B00tOutput {
            output: output.to_string(),
            success: true,
            server_type: "rusty".to_string(),
            working_dir: self.working_dir.display().to_string(),
        };
        
        let content = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to serialize result".to_string());
            
        CallToolResult::success(vec![Content::text(content)])
    }

    /// Create error MCP tool result
    fn create_error_result(&self, error: &str) -> CallToolResult {
        #[derive(serde::Serialize)]
        struct B00tError {
            error: String,
            success: bool,
            server_type: String,
            working_dir: String,
        }

        let result = B00tError {
            error: error.to_string(),
            success: false,
            server_type: "rusty".to_string(),
            working_dir: self.working_dir.display().to_string(),
        };
        
        let content = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to serialize error".to_string());
            
        CallToolResult::error(vec![Content::text(content)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_server_creation() {
        let temp_dir = TempDir::new().unwrap();
        let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
        
        assert_eq!(server.working_dir, temp_dir.path());
        
        // Test that registry has tools
        let tools = server.registry.get_tools();
        assert!(!tools.is_empty());
    }
    
    #[test]
    fn test_server_info() {
        let temp_dir = TempDir::new().unwrap();
        let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
        
        let info = server.get_info();
        assert!(info.instructions.unwrap().contains("🦀 Rusty MCP server"));
        assert!(info.capabilities.tools.is_some());
    }
    
    #[tokio::test]
    async fn test_list_tools() {
        let temp_dir = TempDir::new().unwrap();
        let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
        
        let result = server.list_tools(None, RequestContext::default()).await;
        assert!(result.is_ok());
        
        let tools_result = result.unwrap();
        assert!(!tools_result.tools.is_empty());
        
        // Check for specific expected tools
        let tool_names: Vec<&str> = tools_result.tools.iter()
            .map(|t| t.name.as_ref())
            .collect();
            
        assert!(tool_names.contains(&"b00t_mcp_list"));
        assert!(tool_names.contains(&"b00t_whoami"));
        assert!(tool_names.contains(&"b00t_status"));
    }
    
    #[tokio::test]
    async fn test_ping() {
        let temp_dir = TempDir::new().unwrap();
        let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
        
        // Test that ping returns Ok(())
        let result = server.ping(RequestContext::default()).await;
        assert!(result.is_ok());
    }
    
    #[test]
    fn test_result_creation() {
        let temp_dir = TempDir::new().unwrap();
        let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
        
        let success_result = server.create_success_result("Test output");
        assert!(success_result.content.len() > 0);
        
        let error_result = server.create_error_result("Test error");
        assert!(error_result.content.len() > 0);
        
        // Verify the content can be parsed
        if let Content::Text { text } = &success_result.content[0] {
            let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
            assert_eq!(parsed["success"], true);
            assert_eq!(parsed["server_type"], "rusty");
        }
    }
}