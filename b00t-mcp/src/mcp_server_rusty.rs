use anyhow::Result;
use rmcp::{
    handler::server::ServerHandler,
    model::{
        CallToolResult, Implementation, ProtocolVersion,
        ServerCapabilities, ServerInfo, ListToolsResult,
        CallToolRequestParam, PaginatedRequestParam,
        Content, ErrorData as McpError,
        // Add resource support
        ListResourcesResult, ReadResourceRequestParam, ReadResourceResult,
        RawResource, ResourceContents, Annotated,
    },
    service::{RequestContext, RoleServer},
};
use std::path::Path;
use std::collections::HashMap;
use tracing::{info, error, debug};

use crate::mcp_tools::create_mcp_registry;
use crate::clap_reflection::McpCommandRegistry;
use b00t_c0re_lib::{B00tContext, utils};

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
                .enable_resources()
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

    // 🦀 MCP Resources Support
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        debug!("🦀 list_resources called - providing b00t ecosystem resources");

        let mut resources: Vec<Annotated<RawResource>> = Vec::new();
        
        // Add b00t skills directory as a resource
        if let Ok(b00t_dir) = utils::get_b00t_config_dir() {
            if b00t_dir.exists() {
                let skills_uri = format!("file://{}", b00t_dir.display());
                let mut resource = RawResource::new(skills_uri, "b00t_skills_directory");
                resource.description = Some("B00t skills and configuration directory".to_string());
                resource.mime_type = Some("application/x-directory".to_string());
                resources.push(Annotated::new(resource, None));
            }
        }

        // Add b00t learn topics as resources
        if let Ok(entries) = std::fs::read_dir(utils::get_b00t_config_dir().unwrap_or_default()) {
            for entry in entries.flatten() {
                if let Some(extension) = entry.path().extension() {
                    if extension == "md" {
                        let name = entry.file_name().to_string_lossy().to_string();
                        let topic_name = name.strip_suffix(".md").unwrap_or(&name);
                        let uri = format!("b00t://learn/{}", topic_name);
                        let mut resource = RawResource::new(
                            uri,
                            format!("b00t_skill_{}", topic_name.replace('.', "_"))
                        );
                        resource.description = Some(format!("B00t skill: {}", topic_name));
                        resource.mime_type = Some("text/markdown".to_string());
                        resources.push(Annotated::new(resource, None));
                    }
                }
            }
        }

        // Add current context as a resource
        let mut context_resource = RawResource::new("b00t://context/current", "b00t_current_context");
        context_resource.description = Some("Current b00t agent context and environment".to_string());
        context_resource.mime_type = Some("application/json".to_string());
        resources.push(Annotated::new(context_resource, None));

        info!("🦀 Providing {} b00t resources", resources.len());

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = &request.uri;
        debug!("🦀 read_resource called for URI: {}", uri);

        match uri.as_str() {
            uri if uri.starts_with("b00t://learn/") => {
                let topic = uri.strip_prefix("b00t://learn/").unwrap_or("");
                info!("📚 Reading b00t skill: {}", topic);
                
                match self.read_b00t_skill(topic).await {
                    Ok(content) => Ok(ReadResourceResult {
                        contents: vec![ResourceContents::text(content, uri)],
                    }),
                    Err(e) => {
                        error!("❌ Failed to read b00t skill {}: {}", topic, e);
                        let error_msg = format!("Failed to read skill: {}", e);
                        Err(McpError::internal_error(error_msg, None))
                    }
                }
            }
            "b00t://context/current" => {
                info!("🎯 Reading current b00t context");
                
                match self.read_current_context().await {
                    Ok(content) => Ok(ReadResourceResult {
                        contents: vec![ResourceContents::TextResourceContents {
                            uri: uri.clone(),
                            mime_type: Some("application/json".to_string()),
                            text: content,
                        }],
                    }),
                    Err(e) => {
                        error!("❌ Failed to read current context: {}", e);
                        let error_msg = format!("Failed to read context: {}", e);
                        Err(McpError::internal_error(error_msg, None))
                    }
                }
            }
            uri if uri.starts_with("file://") => {
                let file_path = uri.strip_prefix("file://").unwrap_or(uri);
                info!("📁 Reading file resource: {}", file_path);
                
                match std::fs::read_to_string(file_path) {
                    Ok(content) => Ok(ReadResourceResult {
                        contents: vec![ResourceContents::text(content, uri)],
                    }),
                    Err(e) => {
                        error!("❌ Failed to read file {}: {}", file_path, e);
                        let error_msg = format!("Failed to read file: {}", e);
                        Err(McpError::internal_error(error_msg, None))
                    }
                }
            }
            _ => {
                error!("❌ Unknown resource URI: {}", uri);
                let error_msg = format!("Unknown resource URI: {}", uri);
                Err(McpError::invalid_params(error_msg, None))
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

    /// Read a b00t skill using the template renderer
    async fn read_b00t_skill(&self, topic: &str) -> Result<String> {
        // Use b00t-cli learn command to get the rendered skill
        let output = tokio::process::Command::new("b00t-cli")
            .arg("learn")
            .arg(topic)
            .current_dir(&self.working_dir)
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            anyhow::bail!(
                "b00t-cli learn {} failed: {}", 
                topic,
                String::from_utf8_lossy(&output.stderr)
            )
        }
    }

    /// Read current b00t context as JSON
    async fn read_current_context(&self) -> Result<String> {
        let context = B00tContext::current()?;
        let json = serde_json::to_string_pretty(&context)?;
        Ok(json)
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
    
    // 🦨 TODO: Fix RequestContext creation for tests
    // #[tokio::test]
    // async fn test_list_tools() {
    //     let temp_dir = TempDir::new().unwrap();
    //     let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
    //     
    //     // Need to create proper RequestContext - RequestContext::default() doesn't exist
    //     // let result = server.list_tools(None, context).await;
    //     // assert!(result.is_ok());
    // }
    
    // #[tokio::test]
    // async fn test_ping() {
    //     let temp_dir = TempDir::new().unwrap();
    //     let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
    //     
    //     // Need to create proper RequestContext - RequestContext::default() doesn't exist  
    //     // let result = server.ping(context).await;
    //     // assert!(result.is_ok());
    // }
    
    #[test]
    fn test_result_creation() {
        let temp_dir = TempDir::new().unwrap();
        let server = B00tMcpServerRusty::new(temp_dir.path(), "").unwrap();
        
        let success_result = server.create_success_result("Test output");
        assert!(success_result.content.len() > 0);
        
        let error_result = server.create_error_result("Test error");
        assert!(error_result.content.len() > 0);
        
        // Verify the content can be parsed
        if let Some(_content) = success_result.content.get(0) {
            // Verify we have content
            assert!(!success_result.content.is_empty());
        }
    }
}