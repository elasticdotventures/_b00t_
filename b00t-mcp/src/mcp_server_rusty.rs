use anyhow::Result;
use rmcp::{
    handler::server::ServerHandler,
    model::{
        Annotated,
        CallToolRequestParams,
        CallToolResult,
        Content,
        ErrorData as McpError,
        Implementation,
        // Add resource support
        ListResourcesResult,
        ListToolsResult,
        PaginatedRequestParams,
        RawResource,
        ReadResourceRequestParams,
        ReadResourceResult,
        ResourceContents,
        ServerCapabilities,
        ServerInfo,
    },
    service::{RequestContext, RoleServer},
};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, error, info};

use crate::clap_reflection::McpCommandRegistry;
use crate::{chat::ChatRuntime, mcp_tools::{create_code_mode_registry, create_mcp_registry}};
use b00t_c0re_lib::{B00tContext, utils};

/// Rusty b00t MCP server with compile-time generated tools
///
/// This replaces the brittle dynamic approach with proper Rust trait-based
/// compile-time tool generation that dtolnay would approve of.
#[derive(Clone)]
pub struct B00tMcpServerRusty {
    working_dir: std::path::PathBuf,
    registry: McpCommandRegistry,
    chat_runtime: ChatRuntime,
    /// Captured from MCP initialize request — identifies the host client
    /// (e.g., "hermes", "claude-code", "opencode") for response customization.
    client_info: std::sync::Arc<std::sync::Mutex<Option<rmcp::model::Implementation>>>,
}

impl B00tMcpServerRusty {
    pub fn new<P: AsRef<Path>>(working_dir: P, _config_path: &str, code_mode: bool) -> Result<Self> {
        let working_dir = working_dir.as_ref().to_path_buf();

        let registry = if code_mode {
            create_code_mode_registry()
        } else {
            create_mcp_registry()
        };

        Ok(Self {
            working_dir,
            registry,
            chat_runtime: ChatRuntime::global(),
            client_info: std::sync::Arc::new(std::sync::Mutex::new(None)),
        })
    }

    /// Convenience constructor for flat mode (backward compatible)
    pub fn new_flat<P: AsRef<Path>>(working_dir: P, config_path: &str) -> Result<Self> {
        Self::new(working_dir, config_path, false)
    }

    /// Convenience constructor for code mode
    pub fn new_code_mode<P: AsRef<Path>>(working_dir: P, config_path: &str) -> Result<Self> {
        Self::new(working_dir, config_path, true)
    }

    /// Get the number of available tools
    pub fn tool_count(&self) -> usize {
        self.registry.get_tools().len()
    }
}

impl ServerHandler for B00tMcpServerRusty {
    async fn ping(&self, _context: RequestContext<RoleServer>) -> Result<(), McpError> {
        debug!("🏓 Ping received - Rusty MCP server is alive and well");

        // Log server health info for debugging
        let tools_count = self.registry.get_tools().len();
        debug!(
            "🦀 Server status: {} compile-time tools available",
            tools_count
        );
        debug!("📁 Working directory: {}", self.working_dir.display());

        // Verify b00t-cli is available
        let b00t_cli_available = std::process::Command::new("b00t-cli")
            .arg("--help")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        debug!(
            "🥾 b00t-cli availability: {}",
            if b00t_cli_available { "✅" } else { "❌" }
        );

        if !b00t_cli_available {
            info!("⚠️  b00t-cli not available - MCP tools may fail to execute properly");
        }

        Ok(())
    }

    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::default();
        info.server_info = Implementation::from_build_env();
        info.instructions = Some(
            "🦀 Rusty MCP server for b00t-cli with compile-time generated tools. \
             Features type-safe command dispatch, zero runtime parsing failures, \
             and full CLAP structure synchronization."
                .into(),
        );
        info.capabilities = ServerCapabilities::builder()
            .enable_tools()
            .enable_resources()
            .build();
        info
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        debug!("🦀 list_tools called - using compile-time generated tools");

        let tools = self.registry.get_tools();

        info!(
            "🦀 Generated {} compile-time tools from b00t-cli CLAP structures",
            tools.len()
        );

        for tool in &tools {
            debug!("🔧 Tool: {}", tool.name);
        }

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let tool_name = request.name.as_ref();

        // Extract client identity for response customization
        let client_name = context.peer.peer_info()
            .map(|p| p.client_info.name.clone())
            .unwrap_or_default();

        // Convert request arguments to HashMap
        let params: HashMap<String, serde_json::Value> =
            request.arguments.unwrap_or_default().into_iter().collect();

        if !client_name.is_empty() {
            debug!("🦀 call_tool: {} via client: {}", tool_name, client_name);
        }

        info!(
            "🦀 Executing compile-time tool: {} with params: {:?}",
            tool_name, params
        );

        let execution_result = self.registry.execute(tool_name, &params);
        let chat_indicator = self.chat_runtime.drain_indicator().await;

        match execution_result {
            Ok(output) => {
                info!("✅ Successfully executed tool: {}", tool_name);
                Ok(self.create_success_result(&output, &chat_indicator))
            }
            Err(e) => {
                error!("❌ Failed to execute tool {}: {}", tool_name, e);
                Ok(self.create_error_result(&e.to_string(), &chat_indicator))
            }
        }
    }

    // 🦀 MCP Resources Support
    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
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
                            format!("b00t_skill_{}", topic_name.replace('.', "_")),
                        );
                        resource.description = Some(format!("B00t skill: {}", topic_name));
                        resource.mime_type = Some("text/markdown".to_string());
                        resources.push(Annotated::new(resource, None));
                    }
                }
            }
        }

        // Add current context as a resource
        let mut context_resource =
            RawResource::new("b00t://context/current", "b00t_current_context");
        context_resource.description =
            Some("Current b00t agent context and environment".to_string());
        context_resource.mime_type = Some("application/json".to_string());
        resources.push(Annotated::new(context_resource, None));

        info!("🦀 Providing {} b00t resources", resources.len());

        Ok(ListResourcesResult {
            resources,
            next_cursor: None,
            meta: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = &request.uri;
        debug!("🦀 read_resource called for URI: {}", uri);

        match uri.as_str() {
            uri if uri.starts_with("b00t://learn/") => {
                let topic = uri.strip_prefix("b00t://learn/").unwrap_or("");
                info!("📚 Reading b00t skill: {}", topic);

                match self.read_b00t_skill(topic).await {
                    Ok(content) => Ok(ReadResourceResult::new(
                        vec![ResourceContents::text(content, uri)],
                    )),
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
                    Ok(content) => Ok(ReadResourceResult::new(
                        vec![ResourceContents::TextResourceContents {
                            uri: uri.clone(),
                            mime_type: Some("application/json".to_string()),
                            text: content,
                            meta: None,
                        }],
                    )),
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
                    Ok(content) => Ok(ReadResourceResult::new(
                        vec![ResourceContents::text(content, uri)],
                    )),
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

    async fn on_initialized(
        &self,
        context: rmcp::service::NotificationContext<rmcp::service::RoleServer>,
    ) {
        // Capture client info from peer metadata (hermes, claude-code, opencode, etc.)
        if let Some(peer_info) = context.peer.peer_info() {
            let client_name = peer_info.client_info.name.clone();
            let client_version = peer_info.client_info.version.clone();
            if let Ok(mut info) = self.client_info.lock() {
                *info = Some(peer_info.client_info.clone());
            }
            info!(
                "🦀 b00t-mcp connected to client: {} v{}",
                client_name, client_version
            );
        }

        info!("🦀 Rusty b00t-mcp server initialized successfully");

        let tools = self.registry.get_tools();
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();

        info!("🦀 Available compile-time tools: {}", tool_names.join(", "));

        // Log some statistics
        info!("📊 Total tools: {}", tools.len());

        let tool_categories: HashMap<&str, usize> =
            tools.iter().fold(HashMap::new(), |mut acc, tool| {
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
    fn create_success_result(&self, output: &str, indicator: &str) -> CallToolResult {
        #[derive(serde::Serialize)]
        struct B00tOutput {
            output: String,
            success: bool,
            server_type: String,
            working_dir: String,
            indicator: String,
        }

        let decorated_output = if output.trim().is_empty() {
            indicator.to_string()
        } else {
            format!("{}\n{}", output, indicator)
        };

        let result = B00tOutput {
            output: decorated_output,
            success: true,
            server_type: "rusty".to_string(),
            working_dir: self.working_dir.display().to_string(),
            indicator: indicator.to_string(),
        };

        let content = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to serialize result".to_string());

        CallToolResult::success(vec![Content::text(content)])
    }

    /// Create error MCP tool result
    fn create_error_result(&self, error: &str, indicator: &str) -> CallToolResult {
        #[derive(serde::Serialize)]
        struct B00tError {
            error: String,
            success: bool,
            server_type: String,
            working_dir: String,
            indicator: String,
        }

        let decorated_error = if error.trim().is_empty() {
            indicator.to_string()
        } else {
            format!("{}\n{}", error, indicator)
        };

        let result = B00tError {
            error: decorated_error,
            success: false,
            server_type: "rusty".to_string(),
            working_dir: self.working_dir.display().to_string(),
            indicator: indicator.to_string(),
        };

        let content = serde_json::to_string_pretty(&result)
            .unwrap_or_else(|_| "Failed to serialize error".to_string());

        CallToolResult::error(vec![Content::text(content)])
    }

    /// Read a b00t skill using the shared library
    async fn read_b00t_skill(&self, topic: &str) -> Result<String> {
        use b00t_c0re_lib::TemplateRenderer;
        use b00t_c0re_lib::learn::get_learn_lesson;
        let path = self.working_dir.to_str().unwrap_or("");
        let lesson = get_learn_lesson(path, topic)?;
        let renderer = TemplateRenderer::with_defaults()?;
        let rendered = renderer.render(&lesson)?;
        Ok(rendered)
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

    fn with_tokio_runtime<T>(f: impl FnOnce() -> T) -> T {
        let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
        let guard = runtime.enter();
        let result = f();
        drop(guard);
        runtime.shutdown_background();
        result
    }

    #[test]
    fn test_server_creation() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();

            assert_eq!(server.working_dir, temp_dir.path());

            // Test that registry has tools
            let tools = server.registry.get_tools();
            assert!(!tools.is_empty());
        });
    }

    #[test]
    fn test_server_info() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();

            let info = server.get_info();
            assert!(info.instructions.unwrap().contains("🦀 Rusty MCP server"));
            assert!(info.capabilities.tools.is_some());
        });
    }

    // 🦨 TODO: Fix RequestContext creation for tests
    // #[tokio::test]
    // async fn test_list_tools() {
    //     let temp_dir = TempDir::new().unwrap();
    //     let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();
    //
    //     // Need to create proper RequestContext - RequestContext::default() doesn't exist
    //     // let result = server.list_tools(None, context).await;
    //     // assert!(result.is_ok());
    // }

    // #[tokio::test]
    // async fn test_ping() {
    //     let temp_dir = TempDir::new().unwrap();
    //     let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();
    //
    //     // Need to create proper RequestContext - RequestContext::default() doesn't exist
    //     // let result = server.ping(context).await;
    //     // assert!(result.is_ok());
    // }

    #[test]
    fn test_result_creation() {
        with_tokio_runtime(|| {
            let temp_dir = TempDir::new().unwrap();
            let server = B00tMcpServerRusty::new_flat(temp_dir.path(), "").unwrap();

            let indicator = "<🥾>{ \"chat\": { \"msgs\": 0 } }</🥾>";
            let success_result = server.create_success_result("Test output", indicator);
            assert!(!success_result.content.is_empty());

            let error_result = server.create_error_result("Test error", indicator);
            assert!(!error_result.content.is_empty());

            // Verify the content can be parsed
            if let Some(_content) = success_result.content.get(0) {
                // Verify we have content
                assert!(!success_result.content.is_empty());
            }
        });
    }
}
