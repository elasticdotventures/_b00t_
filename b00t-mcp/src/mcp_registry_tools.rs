//! MCP Tools for b00t MCP Registry
//!
//! Provides MCP tools to interact with the b00t MCP registry

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error};

use b00t_c0re_lib::mcp_registry::{McpRegistry, McpServerRegistration, McpServerConfig, HealthStatus, create_registration_from_datum, ServerTransport, RegistrationMetadata, RegistrationSource, InstallationStatus};
use b00t_c0re_lib::mcp_proxy::GenericMcpProxy;

/// Global MCP registry instance
type RegistryHandle = Arc<Mutex<McpRegistry>>;

lazy_static::lazy_static! {
    static ref REGISTRY: RegistryHandle = Arc::new(Mutex::new(McpRegistry::default()));
    static ref MCP_PROXY: Arc<Mutex<GenericMcpProxy>> = Arc::new(Mutex::new(GenericMcpProxy::new()));
}

/// Parameters for registering an MCP server
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryRegisterParams {
    /// Server ID (e.g., "io.b00t/server-name")
    pub id: String,
    /// Server name
    pub name: String,
    /// Server description
    pub description: Option<String>,
    /// Server command
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Environment variables
    pub env: Option<std::collections::HashMap<String, String>>,
    /// Tags for categorization
    pub tags: Option<Vec<String>>,
}

/// Parameters for unregistering a server
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryUnregisterParams {
    /// Server ID to unregister
    pub id: String,
}

/// Parameters for getting server info
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistryGetParams {
    /// Server ID
    pub id: String,
}

/// Parameters for searching registry
#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrySearchParams {
    /// Search keyword
    pub keyword: Option<String>,
    /// Search by tag
    pub tag: Option<String>,
}

/// MCP tool: Register MCP server in b00t registry
pub async fn registry_register(params: RegistryRegisterParams) -> Result<String> {
    let mut registry = REGISTRY.lock().await;

    // Clone for proxy before moving into registration
    let command_for_proxy = params.command.clone();
    let args_for_proxy = params.args.clone();
    let env_for_proxy = params.env.clone();
    let id_for_response = params.id.clone();
    let name_for_response = params.name.clone();

    let registration = McpServerRegistration {
        id: params.id.clone(),
        name: params.name.clone(),
        description: params.description.unwrap_or_else(|| format!("MCP server: {}", params.name)),
        version: "0.1.0".to_string(),
        homepage: None,
        documentation: None,
        license: None,
        tags: params.tags.unwrap_or_else(|| vec!["b00t".to_string()]),
        config: McpServerConfig {
            command: params.command,
            args: params.args,
            env: params.env,
            cwd: None,
            transport: ServerTransport::Stdio,
        },
        metadata: RegistrationMetadata {
            registered_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            source: RegistrationSource::Local,
            health_status: HealthStatus::Unknown,
            last_health_check: None,
            dependencies: Vec::new(),
            installation_status: InstallationStatus::NotInstalled,
        },
    };

    registry.register(registration)
        .context("Failed to register server")?;

    // Also register with generic proxy for dynamic tool execution
    let mut proxy = MCP_PROXY.lock().await;
    let server_config = b00t_c0re_lib::mcp_proxy::McpServerConfig {
        command: command_for_proxy,
        args: args_for_proxy,
        cwd: None,
        env: env_for_proxy,
        timeout_ms: Some(30000),
    };

    // Auto-discover tools from the registered server
    match proxy.discover_tools_from_server(server_config).await {
        Ok(tools) => {
            info!("🔍 Discovered {} tools from {}", tools.len(), id_for_response);
        }
        Err(e) => {
            error!("⚠️  Failed to discover tools from {}: {}", id_for_response, e);
        }
    }

    let response = serde_json::json!({
        "success": true,
        "message": format!("Registered MCP server: {}", id_for_response),
        "server_id": id_for_response,
        "server_name": name_for_response
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Unregister MCP server from b00t registry
pub async fn registry_unregister(params: RegistryUnregisterParams) -> Result<String> {
    let mut registry = REGISTRY.lock().await;

    registry.unregister(&params.id)
        .context("Failed to unregister server")?;

    let response = serde_json::json!({
        "success": true,
        "message": format!("Unregistered MCP server: {}", params.id),
        "server_id": params.id
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: List all registered MCP servers
pub async fn registry_list() -> Result<String> {
    let registry = REGISTRY.lock().await;
    let servers = registry.list();

    let server_summaries: Vec<serde_json::Value> = servers.iter().map(|s| {
        serde_json::json!({
            "id": s.id,
            "name": s.name,
            "description": s.description,
            "version": s.version,
            "command": s.config.command,
            "tags": s.tags,
            "health": format!("{:?}", s.metadata.health_status),
            "source": format!("{:?}", s.metadata.source),
        })
    }).collect();

    let response = serde_json::json!({
        "success": true,
        "servers": server_summaries,
        "total_servers": servers.len()
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Get specific server registration
pub async fn registry_get(params: RegistryGetParams) -> Result<String> {
    let registry = REGISTRY.lock().await;

    if let Some(server) = registry.get(&params.id) {
        let response = serde_json::json!({
            "success": true,
            "server": server
        });
        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        Err(anyhow::anyhow!("Server '{}' not found in registry", params.id))
    }
}

/// MCP tool: Search MCP servers in registry
pub async fn registry_search(params: RegistrySearchParams) -> Result<String> {
    let registry = REGISTRY.lock().await;

    let results = if let Some(tag) = params.tag {
        registry.search_by_tag(&tag)
    } else if let Some(keyword) = params.keyword {
        registry.search(&keyword)
    } else {
        registry.list()
    };

    let server_summaries: Vec<serde_json::Value> = results.iter().map(|s| {
        serde_json::json!({
            "id": s.id,
            "name": s.name,
            "description": s.description,
            "tags": s.tags,
        })
    }).collect();

    let response = serde_json::json!({
        "success": true,
        "results": server_summaries,
        "total_results": results.len()
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Sync with official MCP registry
pub async fn registry_sync_official() -> Result<String> {
    let mut registry = REGISTRY.lock().await;

    let synced_count = registry.sync_official_registry().await
        .context("Failed to sync with official registry")?;

    let response = serde_json::json!({
        "success": true,
        "message": format!("Synced {} servers from official MCP registry", synced_count),
        "synced_count": synced_count
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Auto-discover MCP servers from system
pub async fn registry_discover() -> Result<String> {
    let mut registry = REGISTRY.lock().await;

    let discovered_count = registry.auto_discover().await
        .context("Failed to auto-discover servers")?;

    let response = serde_json::json!({
        "success": true,
        "message": format!("Discovered {} MCP servers", discovered_count),
        "discovered_count": discovered_count
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Export registry to MCP format
pub async fn registry_export() -> Result<String> {
    let registry = REGISTRY.lock().await;

    let export_json = registry.export_to_mcp_format()
        .context("Failed to export registry")?;

    Ok(export_json)
}

/// MCP tool: Import from MCP registry format
pub async fn registry_import(json: String) -> Result<String> {
    let mut registry = REGISTRY.lock().await;

    let imported_count = registry.import_from_mcp_format(&json)
        .context("Failed to import registry")?;

    let response = serde_json::json!({
        "success": true,
        "message": format!("Imported {} servers", imported_count),
        "imported_count": imported_count
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// Initialize registry with b00t-mcp itself
pub async fn init_registry_with_b00t() -> Result<()> {
    let mut registry = REGISTRY.lock().await;

    // Register b00t-mcp itself
    let b00t_mcp_path = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("b00t-mcp"));

    let registration = create_registration_from_datum(
        "io.b00t/b00t-mcp".to_string(),
        "b00t MCP Server".to_string(),
        b00t_mcp_path.display().to_string(),
        vec!["--stdio".to_string()],
    );

    registry.register(registration)?;

    info!("🥾 Registered b00t-mcp itself in registry");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_register() {
        let params = RegistryRegisterParams {
            id: "io.test/test-server".to_string(),
            name: "Test Server".to_string(),
            description: Some("Test description".to_string()),
            command: "test-cmd".to_string(),
            args: vec!["--stdio".to_string()],
            env: None,
            tags: Some(vec!["test".to_string()]),
        };

        let result = registry_register(params).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_list() {
        let result = registry_list().await;
        assert!(result.is_ok());

        let response: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(response["success"], true);
    }

    #[tokio::test]
    async fn test_registry_search() {
        let params = RegistrySearchParams {
            keyword: Some("test".to_string()),
            tag: None,
        };

        let result = registry_search(params).await;
        assert!(result.is_ok());
    }
}