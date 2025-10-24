//! Generic MCP Tool Proxy Implementation in Rust
//!
//! Provides a dynamic, runtime-configurable MCP tool proxy that can handle
//! any MCP tool without requiring compile-time knowledge of tool signatures.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Generic MCP tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// Input schema for tool parameters
    pub input_schema: ToolInputSchema,
    /// MCP server configuration
    pub server_config: McpServerConfig,
}

/// Tool input schema (JSON Schema)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: Option<Map<String, Value>>,
    pub required: Option<Vec<String>>,
    #[serde(flatten)]
    pub additional: Map<String, Value>,
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server command
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Working directory
    pub cwd: Option<String>,
    /// Environment variables
    pub env: Option<HashMap<String, String>>,
    /// Server initialization timeout (milliseconds)
    pub timeout_ms: Option<u64>,
}

/// MCP tool execution request
#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolRequest {
    /// Tool name to execute
    pub tool: String,
    /// Tool parameters (JSON object)
    pub params: Value,
    /// Optional request ID
    pub request_id: Option<String>,
}

/// MCP tool execution response
#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolResponse {
    /// Success status
    pub success: bool,
    /// Response data (if successful)
    pub data: Option<Value>,
    /// Error message (if failed)
    pub error: Option<String>,
    /// Request ID (if provided)
    pub request_id: Option<String>,
    /// Execution metadata
    pub metadata: McpExecutionMetadata,
}

/// Execution metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct McpExecutionMetadata {
    /// Tool name executed
    pub tool: String,
    /// Execution duration (milliseconds)
    pub duration_ms: u64,
    /// Server that handled the request
    pub server: String,
    /// Timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Generic MCP proxy for dynamic tool execution
pub struct GenericMcpProxy {
    /// Available tool definitions
    tools: HashMap<String, McpToolDefinition>,
    /// Active MCP server connections
    servers: HashMap<String, McpServerConnection>,
}

/// MCP server connection state
struct McpServerConnection {
    config: McpServerConfig,
    /// Last used timestamp for connection reuse
    last_used: chrono::DateTime<chrono::Utc>,
    /// Connection health status
    healthy: bool,
}

impl GenericMcpProxy {
    /// Create new generic MCP proxy
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            servers: HashMap::new(),
        }
    }

    /// Register an MCP tool definition
    pub fn register_tool(&mut self, tool: McpToolDefinition) -> Result<()> {
        let tool_name = tool.name.clone();
        let server_id = format!(
            "{}:{}",
            tool.server_config.command,
            tool.server_config.args.join(" ")
        );

        // Register server if not already registered
        if !self.servers.contains_key(&server_id) {
            let connection = McpServerConnection {
                config: tool.server_config.clone(),
                last_used: chrono::Utc::now(),
                healthy: false, // Will be verified on first use
            };
            self.servers.insert(server_id, connection);
        }

        self.tools.insert(tool_name.clone(), tool);
        info!("🔧 Registered MCP tool: {}", tool_name);
        Ok(())
    }

    /// Register multiple tools from configuration
    pub fn register_tools_from_config(&mut self, tools: Vec<McpToolDefinition>) -> Result<()> {
        for tool in tools {
            self.register_tool(tool)?;
        }
        Ok(())
    }

    /// Auto-discover tools from an MCP server
    pub async fn discover_tools_from_server(
        &mut self,
        server_config: McpServerConfig,
    ) -> Result<Vec<String>> {
        let tools = self.query_server_tools(&server_config).await?;
        let mut registered_tools = Vec::new();

        for tool_info in tools {
            let tool_def = McpToolDefinition {
                name: tool_info["name"].as_str().unwrap_or("unknown").to_string(),
                description: tool_info["description"].as_str().unwrap_or("").to_string(),
                input_schema: serde_json::from_value(tool_info["inputSchema"].clone())
                    .unwrap_or_else(|_| ToolInputSchema {
                        schema_type: "object".to_string(),
                        properties: None,
                        required: None,
                        additional: Map::new(),
                    }),
                server_config: server_config.clone(),
            };

            self.register_tool(tool_def)?;
            registered_tools.push(tool_info["name"].as_str().unwrap_or("unknown").to_string());
        }

        info!(
            "🔍 Auto-discovered {} tools from MCP server",
            registered_tools.len()
        );
        Ok(registered_tools)
    }

    /// Query MCP server for available tools
    async fn query_server_tools(&self, server_config: &McpServerConfig) -> Result<Vec<Value>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/list",
            "params": {}
        });

        let response = self.execute_server_request(server_config, request).await?;

        if let Some(result) = response.get("result") {
            if let Some(tools) = result.get("tools") {
                if let Some(tools_array) = tools.as_array() {
                    return Ok(tools_array.clone());
                }
            }
        }

        Ok(Vec::new())
    }

    /// Execute tool by name with parameters
    pub async fn execute_tool(&mut self, request: McpToolRequest) -> Result<McpToolResponse> {
        let start_time = std::time::Instant::now();
        let timestamp = chrono::Utc::now();

        // Get tool definition
        let tool_def = self
            .tools
            .get(&request.tool)
            .ok_or_else(|| anyhow::anyhow!("Tool '{}' not found", request.tool))?
            .clone();

        // Validate parameters against schema (basic validation)
        self.validate_parameters(&tool_def.input_schema, &request.params)?;

        // Prepare JSON-RPC request
        let request_id = request
            .request_id
            .clone()
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let jsonrpc_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {
                "name": request.tool,
                "arguments": request.params
            }
        });

        info!(
            "🚀 Executing MCP tool '{}' with params: {}",
            request.tool, request.params
        );

        // Execute via MCP server
        match self
            .execute_server_request(&tool_def.server_config, jsonrpc_request)
            .await
        {
            Ok(response) => {
                let duration = start_time.elapsed().as_millis() as u64;

                if let Some(result) = response.get("result") {
                    Ok(McpToolResponse {
                        success: true,
                        data: Some(result.clone()),
                        error: None,
                        request_id: request.request_id,
                        metadata: McpExecutionMetadata {
                            tool: request.tool,
                            duration_ms: duration,
                            server: format!(
                                "{}:{}",
                                tool_def.server_config.command,
                                tool_def.server_config.args.join(" ")
                            ),
                            timestamp,
                        },
                    })
                } else if let Some(error) = response.get("error") {
                    Ok(McpToolResponse {
                        success: false,
                        data: None,
                        error: Some(error.to_string()),
                        request_id: request.request_id,
                        metadata: McpExecutionMetadata {
                            tool: request.tool,
                            duration_ms: duration,
                            server: format!(
                                "{}:{}",
                                tool_def.server_config.command,
                                tool_def.server_config.args.join(" ")
                            ),
                            timestamp,
                        },
                    })
                } else {
                    Err(anyhow::anyhow!("Invalid MCP response format"))
                }
            }
            Err(e) => {
                let duration = start_time.elapsed().as_millis() as u64;
                Ok(McpToolResponse {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                    request_id: request.request_id,
                    metadata: McpExecutionMetadata {
                        tool: request.tool,
                        duration_ms: duration,
                        server: format!(
                            "{}:{}",
                            tool_def.server_config.command,
                            tool_def.server_config.args.join(" ")
                        ),
                        timestamp,
                    },
                })
            }
        }
    }

    /// Execute JSON-RPC request on MCP server
    async fn execute_server_request(
        &self,
        server_config: &McpServerConfig,
        request: Value,
    ) -> Result<Value> {
        let mut cmd = Command::new(&server_config.command);
        cmd.args(&server_config.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(cwd) = &server_config.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(env) = &server_config.env {
            for (key, value) in env {
                cmd.env(key, value);
            }
        }

        debug!(
            "Starting MCP server: {} {}",
            server_config.command,
            server_config.args.join(" ")
        );

        let mut child = cmd.spawn().context("Failed to start MCP server process")?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdin handle"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdout handle"))?;

        // Send request
        let mut stdin_writer = stdin;
        let request_str = serde_json::to_string(&request)?;
        debug!("Sending MCP request: {}", request_str);

        stdin_writer.write_all(request_str.as_bytes()).await?;
        stdin_writer.write_all(b"\n").await?;
        stdin_writer.shutdown().await?;

        // Read response
        let mut stdout_reader = BufReader::new(stdout);
        let mut response_line = String::new();
        stdout_reader.read_line(&mut response_line).await?;

        // Clean up process
        let _ = child.kill().await;

        debug!("Received MCP response: {}", response_line.trim());

        // Parse JSON response
        let response: Value = serde_json::from_str(response_line.trim())
            .context("Failed to parse MCP response as JSON")?;

        Ok(response)
    }

    /// Basic parameter validation against JSON schema
    fn validate_parameters(&self, schema: &ToolInputSchema, params: &Value) -> Result<()> {
        // Basic validation - check required fields
        if let Some(required) = &schema.required {
            if let Some(params_obj) = params.as_object() {
                for required_field in required {
                    if !params_obj.contains_key(required_field) {
                        return Err(anyhow::anyhow!(
                            "Required parameter '{}' missing",
                            required_field
                        ));
                    }
                }
            } else if !required.is_empty() {
                return Err(anyhow::anyhow!(
                    "Parameters must be an object when required fields are specified"
                ));
            }
        }

        Ok(())
    }

    /// List all registered tools
    pub fn list_tools(&self) -> Vec<&McpToolDefinition> {
        self.tools.values().collect()
    }

    /// Get tool definition by name
    pub fn get_tool(&self, name: &str) -> Option<&McpToolDefinition> {
        self.tools.get(name)
    }

    /// Health check for all registered servers
    pub async fn health_check(&mut self) -> HashMap<String, bool> {
        let mut health_status = HashMap::new();

        // Collect server configs to avoid borrowing issues
        let server_configs: Vec<(String, McpServerConfig)> = self
            .servers
            .iter()
            .map(|(id, conn)| (id.clone(), conn.config.clone()))
            .collect();

        for (server_id, config) in server_configs {
            let health = self.check_server_health(&config).await;
            if let Some(connection) = self.servers.get_mut(&server_id) {
                connection.healthy = health;
                connection.last_used = chrono::Utc::now();
            }
            health_status.insert(server_id, health);
        }

        health_status
    }

    /// Check individual server health
    async fn check_server_health(&self, server_config: &McpServerConfig) -> bool {
        let ping_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "health_check",
            "method": "ping",
            "params": {}
        });

        match self
            .execute_server_request(server_config, ping_request)
            .await
        {
            Ok(_) => true,
            Err(_) => false,
        }
    }
}

impl Default for GenericMcpProxy {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to create RAGLight MCP tool definitions
pub fn create_raglight_tools() -> Vec<McpToolDefinition> {
    vec![
        McpToolDefinition {
            name: "rag-add-document".to_string(),
            description: "Add document to RAG system for indexing".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: Some([
                    ("source".to_string(), serde_json::json!({"type": "string", "description": "Document source (URL, file path, git repo)"})),
                    ("topic".to_string(), serde_json::json!({"type": "string", "description": "Target topic/datum for indexing"})),
                    ("loader_type".to_string(), serde_json::json!({"type": "string", "enum": ["url", "git", "pdf", "text", "markdown", "auto"], "description": "Loader type (auto-detected if not specified)"})),
                    ("metadata".to_string(), serde_json::json!({"type": "object", "description": "Optional metadata for the document"})),
                ].into_iter().collect()),
                required: Some(vec!["source".to_string(), "topic".to_string()]),
                additional: Map::new(),
            },
            server_config: McpServerConfig {
                command: "python3".to_string(),
                args: vec!["-m".to_string(), "raglight.mcp_server".to_string()],
                cwd: None,
                env: None,
                timeout_ms: Some(30000),
            },
        },
        McpToolDefinition {
            name: "rag-query".to_string(),
            description: "Query RAG system for information on a topic".to_string(),
            input_schema: ToolInputSchema {
                schema_type: "object".to_string(),
                properties: Some([
                    ("topic".to_string(), serde_json::json!({"type": "string", "description": "Topic to query"})),
                    ("query".to_string(), serde_json::json!({"type": "string", "description": "Query text"})),
                    ("max_results".to_string(), serde_json::json!({"type": "integer", "description": "Maximum number of results", "default": 10})),
                ].into_iter().collect()),
                required: Some(vec!["topic".to_string(), "query".to_string()]),
                additional: Map::new(),
            },
            server_config: McpServerConfig {
                command: "python3".to_string(),
                args: vec!["-m".to_string(), "raglight.mcp_server".to_string()],
                cwd: None,
                env: None,
                timeout_ms: Some(30000),
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_registration() {
        let mut proxy = GenericMcpProxy::new();
        let tools = create_raglight_tools();

        assert!(proxy.register_tools_from_config(tools).is_ok());
        assert_eq!(proxy.list_tools().len(), 2);
        assert!(proxy.get_tool("rag-add-document").is_some());
        assert!(proxy.get_tool("rag-query").is_some());
    }

    #[test]
    fn test_parameter_validation() {
        let proxy = GenericMcpProxy::new();
        let schema = ToolInputSchema {
            schema_type: "object".to_string(),
            properties: None,
            required: Some(vec!["source".to_string(), "topic".to_string()]),
            additional: Map::new(),
        };

        // Valid parameters
        let valid_params = serde_json::json!({
            "source": "https://example.com",
            "topic": "rust"
        });
        assert!(proxy.validate_parameters(&schema, &valid_params).is_ok());

        // Missing required parameter
        let invalid_params = serde_json::json!({
            "source": "https://example.com"
        });
        assert!(proxy.validate_parameters(&schema, &invalid_params).is_err());
    }

    #[tokio::test]
    async fn test_tool_execution_request_format() {
        let request = McpToolRequest {
            tool: "rag-query".to_string(),
            params: serde_json::json!({
                "topic": "rust",
                "query": "How to handle errors?"
            }),
            request_id: Some("test-123".to_string()),
        };

        assert_eq!(request.tool, "rag-query");
        assert_eq!(request.request_id, Some("test-123".to_string()));
    }
}
