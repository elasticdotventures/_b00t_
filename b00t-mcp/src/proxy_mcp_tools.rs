//! Proxy MCP Tools using Generic MCP Proxy
//! 
//! Provides MCP tools that use the generic proxy system for dynamic tool execution.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use crate::generic_mcp_proxy::{GenericMcpProxy, McpToolRequest, McpToolDefinition, create_raglight_tools};

/// Global MCP proxy instance
type McpProxyRegistry = Arc<Mutex<GenericMcpProxy>>;

lazy_static::lazy_static! {
    static ref MCP_PROXY: McpProxyRegistry = Arc::new(Mutex::new(GenericMcpProxy::new()));
}

/// Parameters for executing any MCP tool via proxy
#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyExecuteParams {
    /// Tool name to execute
    pub tool: String,
    /// Tool parameters (JSON object)
    pub params: serde_json::Value,
    /// Optional request ID
    pub request_id: Option<String>,
}

/// Parameters for registering MCP tool
#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyRegisterToolParams {
    /// Tool definition to register
    pub tool_definition: serde_json::Value,
}

/// Parameters for discovering tools from MCP server
#[derive(Debug, Serialize, Deserialize)]
pub struct ProxyDiscoverParams {
    /// Server command
    pub command: String,
    /// Server arguments
    pub args: Vec<String>,
    /// Working directory (optional)
    pub cwd: Option<String>,
    /// Environment variables (optional)
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// MCP tool: Execute any registered tool via proxy
pub async fn proxy_execute(params: ProxyExecuteParams) -> Result<String> {
    let mut proxy = MCP_PROXY.lock().await;
    
    let request = McpToolRequest {
        tool: params.tool.clone(),
        params: params.params,
        request_id: params.request_id,
    };
    
    info!("🔄 Proxying execution of tool: {}", params.tool);
    
    let response = proxy.execute_tool(request).await
        .context("Failed to execute tool via proxy")?;
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: List all registered tools in proxy
pub async fn proxy_list_tools() -> Result<String> {
    let proxy = MCP_PROXY.lock().await;
    let tools = proxy.list_tools();
    
    let tool_summaries: Vec<serde_json::Value> = tools.iter().map(|tool| {
        serde_json::json!({
            "name": tool.name,
            "description": tool.description,
            "server": format!("{}:{}", tool.server_config.command, tool.server_config.args.join(" ")),
            "input_schema": tool.input_schema
        })
    }).collect();
    
    let response = serde_json::json!({
        "success": true,
        "tools": tool_summaries,
        "total_tools": tools.len()
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Register a new tool definition
pub async fn proxy_register_tool(params: ProxyRegisterToolParams) -> Result<String> {
    let mut proxy = MCP_PROXY.lock().await;
    
    let tool_def: McpToolDefinition = serde_json::from_value(params.tool_definition)
        .context("Invalid tool definition format")?;
    
    let tool_name = tool_def.name.clone();
    proxy.register_tool(tool_def)
        .context("Failed to register tool")?;
    
    let response = serde_json::json!({
        "success": true,
        "message": format!("Tool '{}' registered successfully", tool_name),
        "tool_name": tool_name
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Discover tools from MCP server
pub async fn proxy_discover_tools(params: ProxyDiscoverParams) -> Result<String> {
    let mut proxy = MCP_PROXY.lock().await;
    
    let server_config = crate::generic_mcp_proxy::McpServerConfig {
        command: params.command.clone(),
        args: params.args,
        cwd: params.cwd,
        env: params.env,
        timeout_ms: Some(30000),
    };
    
    info!("🔍 Discovering tools from MCP server: {}", params.command);
    
    let discovered_tools = proxy.discover_tools_from_server(server_config).await
        .context("Failed to discover tools from MCP server")?;
    
    let response = serde_json::json!({
        "success": true,
        "message": format!("Discovered {} tools from server '{}'", discovered_tools.len(), params.command),
        "server": params.command,
        "discovered_tools": discovered_tools,
        "total_discovered": discovered_tools.len()
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Health check all registered servers
pub async fn proxy_health_check() -> Result<String> {
    let mut proxy = MCP_PROXY.lock().await;
    
    info!("🏥 Performing health check on all MCP servers");
    
    let health_status = proxy.health_check().await;
    
    let healthy_servers = health_status.values().filter(|&&status| status).count();
    let total_servers = health_status.len();
    
    let response = serde_json::json!({
        "success": true,
        "health_status": health_status,
        "healthy_servers": healthy_servers,
        "total_servers": total_servers,
        "overall_health": if healthy_servers == total_servers { "healthy" } else { "degraded" }
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Get detailed information about a specific tool
pub async fn proxy_get_tool_info(tool_name: String) -> Result<String> {
    let proxy = MCP_PROXY.lock().await;
    
    if let Some(tool) = proxy.get_tool(&tool_name) {
        let response = serde_json::json!({
            "success": true,
            "tool": {
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
                "server_config": {
                    "command": tool.server_config.command,
                    "args": tool.server_config.args,
                    "cwd": tool.server_config.cwd,
                    "timeout_ms": tool.server_config.timeout_ms
                }
            }
        });
        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        Err(anyhow::anyhow!("Tool '{}' not found", tool_name))
    }
}

/// Initialize proxy with default RAGLight tools
pub async fn initialize_proxy_with_defaults() -> Result<()> {
    let mut proxy = MCP_PROXY.lock().await;
    
    // Register RAGLight tools
    let raglight_tools = create_raglight_tools();
    proxy.register_tools_from_config(raglight_tools)
        .context("Failed to register RAGLight tools")?;
    
    info!("🚀 Initialized MCP proxy with default RAGLight tools");
    Ok(())
}

/// MCP tool: Initialize proxy with RAGLight tools (can be called via MCP)
pub async fn proxy_init() -> Result<String> {
    initialize_proxy_with_defaults().await?;
    
    let response = serde_json::json!({
        "success": true,
        "message": "MCP proxy initialized with default tools",
        "default_tools": ["rag-add-document", "rag-query"]
    });
    
    Ok(serde_json::to_string_pretty(&response)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proxy_initialization() {
        let result = proxy_init().await;
        assert!(result.is_ok());
        
        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(response["success"], true);
    }

    #[tokio::test]
    async fn test_proxy_list_tools() {
        // Initialize first
        let _ = proxy_init().await;
        
        let result = proxy_list_tools().await;
        assert!(result.is_ok());
        
        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(response["success"], true);
        assert!(response["total_tools"].as_u64().unwrap() > 0);
    }

    #[tokio::test]
    async fn test_proxy_tool_info() {
        // Initialize first
        let _ = proxy_init().await;
        
        let result = proxy_get_tool_info("rag-query".to_string()).await;
        assert!(result.is_ok());
        
        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["tool"]["name"], "rag-query");
    }
}