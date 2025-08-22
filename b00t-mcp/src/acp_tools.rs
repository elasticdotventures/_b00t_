//! MCP Tools for ACP Hive Communication
//! 
//! Provides MCP tools that enable agents to participate in coordinated hive missions.

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn, error};

use crate::acp_hive::{AcpHiveClient, HiveMission};
use b00t_acp::{fetch_jwt_from_website, AcpJwtValidator};

/// Global hive client registry for MCP agents
type HiveRegistry = Arc<Mutex<HashMap<String, AcpHiveClient>>>;

lazy_static::lazy_static! {
    static ref HIVE_CLIENTS: HiveRegistry = Arc::new(Mutex::new(HashMap::new()));
}

/// Parameters for joining a hive mission
#[derive(Debug, Serialize, Deserialize)]
pub struct JoinHiveParams {
    /// Mission identifier
    pub mission_id: String,
    /// Agent role in the mission
    pub role: String,
    /// Agent namespace (defaults to account.username)
    pub namespace: Option<String>,
    /// NATS server URL (defaults to c010.promptexecution.com:4222)
    pub nats_url: Option<String>,
}

/// Parameters for creating a new hive mission
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateHiveParams {
    /// Mission identifier
    pub mission_id: String,
    /// Expected number of agents
    pub expected_agents: usize,
    /// Mission description
    pub description: String,
    /// Agent role in the mission
    pub role: String,
    /// Agent namespace (defaults to account.username)
    pub namespace: Option<String>,
    /// NATS server URL (defaults to c010.promptexecution.com:4222)
    pub nats_url: Option<String>,
}

/// Parameters for sending status to hive
#[derive(Debug, Serialize, Deserialize)]
pub struct HiveStatusParams {
    /// Mission identifier
    pub mission_id: String,
    /// Status description
    pub description: String,
    /// Optional payload data
    pub payload: Option<serde_json::Value>,
}

/// Parameters for proposing actions to hive
#[derive(Debug, Serialize, Deserialize)]
pub struct HiveProposeParams {
    /// Mission identifier
    pub mission_id: String,
    /// Action to propose
    pub action: String,
    /// Optional action payload
    pub payload: Option<serde_json::Value>,
}

/// Parameters for step synchronization
#[derive(Debug, Serialize, Deserialize)]
pub struct HiveStepSyncParams {
    /// Mission identifier
    pub mission_id: String,
    /// Target step to synchronize to
    pub target_step: u64,
    /// Timeout in seconds (default: 60)
    pub timeout_seconds: Option<u64>,
}

/// Parameters for showing hive status
#[derive(Debug, Serialize, Deserialize)]
pub struct HiveShowParams {
    /// Mission identifier (optional - shows all missions if not specified)
    pub mission_id: Option<String>,
}

/// MCP tool: Join an existing hive mission
pub async fn acp_hive_join(params: JoinHiveParams) -> Result<String> {
    let agent_id = format!("mcp_agent_{}", uuid::Uuid::new_v4().to_string()[..8]);
    let namespace = params.namespace.unwrap_or_else(|| {
        get_hive_namespace(&params.role)
    });
    let nats_url = params.nats_url.unwrap_or_else(|| {
        std::env::var("NATS_URL")
            .unwrap_or_else(|_| "nats://c010.promptexecution.com:4222".to_string())
    });

    info!("🐝 MCP agent {} joining hive mission: {}", agent_id, params.mission_id);

    // 🔐 JWT Security: Fetch JWT from b00t-website for namespace authentication
    let jwt_token = fetch_jwt_for_hive_operation(&params.role, &namespace).await?;

    // Create hive client with JWT authentication
    let mut client = AcpHiveClient::join_mission(
        agent_id.clone(),
        params.role.clone(),
        params.mission_id.clone(),
        namespace.clone(),
        nats_url,
    ).await.context("Failed to join hive mission")?;
    
    // Set JWT token for security
    if jwt_token != "development_mode_no_jwt" {
        client.set_jwt_token(jwt_token);
    }

    // Register client globally
    {
        let mut registry = HIVE_CLIENTS.lock().await;
        registry.insert(params.mission_id.clone(), client);
    }

    let response = serde_json::json!({
        "success": true,
        "message": format!("Agent {} joined hive mission {}", agent_id, params.mission_id),
        "agent_id": agent_id,
        "mission_id": params.mission_id,
        "role": params.role,
        "namespace": namespace,
        "security": {
            "jwt_authenticated": jwt_token != "development_mode_no_jwt",
            "namespace_enforced": jwt_token != "development_mode_no_jwt"
        }
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Create a new hive mission
pub async fn acp_hive_create(params: CreateHiveParams) -> Result<String> {
    let agent_id = format!("mcp_leader_{}", uuid::Uuid::new_v4().to_string()[..8]);
    let namespace = params.namespace.unwrap_or_else(|| {
        get_hive_namespace(&params.role)
    });
    let nats_url = params.nats_url.unwrap_or_else(|| {
        std::env::var("NATS_URL")
            .unwrap_or_else(|_| "nats://c010.promptexecution.com:4222".to_string())
    });

    info!("🐝 MCP agent {} creating hive mission: {}", agent_id, params.mission_id);

    // 🔐 JWT Security: Fetch JWT from b00t-website for namespace authentication
    let jwt_token = fetch_jwt_for_hive_operation(&params.role, &namespace).await?;

    // Create mission
    let mission = AcpHiveClient::create_mission(
        params.mission_id.clone(),
        namespace.clone(),
        params.expected_agents,
        params.description.clone(),
    );

    // Create hive client with JWT authentication
    let mut client = AcpHiveClient::new(
        agent_id.clone(),
        params.role.clone(),
        mission,
        nats_url,
    ).await.context("Failed to create hive mission")?;
    
    // Set JWT token for security
    if jwt_token != "development_mode_no_jwt" {
        client.set_jwt_token(jwt_token.clone());
    }

    // Register client globally
    {
        let mut registry = HIVE_CLIENTS.lock().await;
        registry.insert(params.mission_id.clone(), client);
    }

    let response = serde_json::json!({
        "success": true,
        "message": format!("Created hive mission {} with leader {}", params.mission_id, agent_id),
        "agent_id": agent_id,
        "mission_id": params.mission_id,
        "expected_agents": params.expected_agents,
        "role": params.role,
        "namespace": namespace,
        "description": params.description,
        "security": {
            "jwt_authenticated": jwt_token != "development_mode_no_jwt",
            "namespace_enforced": jwt_token != "development_mode_no_jwt"
        }
    });

    Ok(serde_json::to_string_pretty(&response)?)
}

/// MCP tool: Send status update to hive
pub async fn acp_hive_status(params: HiveStatusParams) -> Result<String> {
    let mut registry = HIVE_CLIENTS.lock().await;
    
    if let Some(client) = registry.get_mut(&params.mission_id) {
        client.send_status(&params.description, params.payload)
            .await
            .context("Failed to send status to hive")?;

        let response = serde_json::json!({
            "success": true,
            "message": format!("Status sent to hive mission {}", params.mission_id),
            "mission_id": params.mission_id,
            "description": params.description,
            "agent_id": client.agent_id(),
            "step": client.current_step()
        });

        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        Err(anyhow::anyhow!("No active hive client for mission: {}", params.mission_id))
    }
}

/// MCP tool: Propose action to hive
pub async fn acp_hive_propose(params: HiveProposeParams) -> Result<String> {
    let mut registry = HIVE_CLIENTS.lock().await;
    
    if let Some(client) = registry.get_mut(&params.mission_id) {
        client.propose_action(&params.action, params.payload)
            .await
            .context("Failed to propose action to hive")?;

        let response = serde_json::json!({
            "success": true,
            "message": format!("Action '{}' proposed to hive mission {}", params.action, params.mission_id),
            "mission_id": params.mission_id,
            "action": params.action,
            "agent_id": client.agent_id(),
            "step": client.current_step()
        });

        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        Err(anyhow::anyhow!("No active hive client for mission: {}", params.mission_id))
    }
}

/// MCP tool: Wait for step synchronization
pub async fn acp_hive_sync(params: HiveStepSyncParams) -> Result<String> {
    let mut registry = HIVE_CLIENTS.lock().await;
    
    if let Some(client) = registry.get_mut(&params.mission_id) {
        let timeout = params.timeout_seconds.unwrap_or(60);
        
        info!("🐝 Agent {} waiting for step sync to step {} (timeout: {}s)", 
              client.agent_id(), params.target_step, timeout);

        let hive_status = client.wait_for_step_sync(params.target_step, timeout)
            .await
            .context("Step synchronization failed or timed out")?;

        let response = serde_json::json!({
            "success": true,
            "message": format!("Step {} synchronized for mission {}", params.target_step, params.mission_id),
            "mission_id": params.mission_id,
            "target_step": params.target_step,
            "current_step": hive_status.current_step,
            "agents_ready": hive_status.agents_ready,
            "step_complete": hive_status.step_complete,
            "total_agents": hive_status.agents.len()
        });

        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        Err(anyhow::anyhow!("No active hive client for mission: {}", params.mission_id))
    }
}

/// MCP tool: Signal ready for next step
pub async fn acp_hive_ready(params: HiveStepSyncParams) -> Result<String> {
    let mut registry = HIVE_CLIENTS.lock().await;
    
    if let Some(client) = registry.get_mut(&params.mission_id) {
        client.signal_step_ready(params.target_step)
            .await
            .context("Failed to signal step readiness")?;

        let response = serde_json::json!({
            "success": true,
            "message": format!("Agent {} signaled ready for step {}", client.agent_id(), params.target_step),
            "mission_id": params.mission_id,
            "agent_id": client.agent_id(),
            "step": params.target_step
        });

        Ok(serde_json::to_string_pretty(&response)?)
    } else {
        Err(anyhow::anyhow!("No active hive client for mission: {}", params.mission_id))
    }
}

/// MCP tool: Show hive status and participating agents
pub async fn acp_hive_show(params: HiveShowParams) -> Result<String> {
    let registry = HIVE_CLIENTS.lock().await;
    
    if let Some(mission_id) = params.mission_id {
        // Show specific mission
        if let Some(client) = registry.get(&mission_id) {
            let status = client.get_hive_status().await
                .context("Failed to get hive status")?;

            let response = serde_json::json!({
                "success": true,
                "mission_id": mission_id,
                "hive_status": status,
                "agent_summary": client.get_agent_summary(),
                "hive_summary": client.get_hive_summary()
            });

            Ok(serde_json::to_string_pretty(&response)?)
        } else {
            Err(anyhow::anyhow!("No active hive client for mission: {}", mission_id))
        }
    } else {
        // Show all active missions
        let mut missions = Vec::new();
        
        for (mission_id, client) in registry.iter() {
            missions.push(serde_json::json!({
                "mission_id": mission_id,
                "agent_summary": client.get_agent_summary(),
                "hive_summary": client.get_hive_summary()
            }));
        }

        let response = serde_json::json!({
            "success": true,
            "message": format!("Found {} active hive missions", missions.len()),
            "active_missions": missions
        });

        Ok(serde_json::to_string_pretty(&response)?)
    }
}

/// MCP tool: Leave hive mission
pub async fn acp_hive_leave(params: HiveShowParams) -> Result<String> {
    if let Some(mission_id) = params.mission_id {
        let mut registry = HIVE_CLIENTS.lock().await;
        
        if let Some(client) = registry.remove(&mission_id) {
            // Send leaving status before disconnecting
            if let Err(e) = client.send_status("leaving_mission", None).await {
                warn!("Failed to send leaving status: {}", e);
            }

            let response = serde_json::json!({
                "success": true,
                "message": format!("Left hive mission {}", mission_id),
                "mission_id": mission_id,
                "agent_id": client.agent_id()
            });

            Ok(serde_json::to_string_pretty(&response)?)
        } else {
            Err(anyhow::anyhow!("No active hive client for mission: {}", mission_id))
        }
    } else {
        Err(anyhow::anyhow!("Mission ID is required to leave hive"))
    }
}

/// Helper to get NATS URL from environment or default
pub fn get_nats_url() -> String {
    std::env::var("NATS_URL")
        .unwrap_or_else(|_| "nats://c010.promptexecution.com:4222".to_string())
}

/// Helper to get current hive namespace for a given role
pub fn get_hive_namespace(role: &str) -> String {
    format!("account.{}.{}", whoami::username(), role)
}

/// 🔐 Fetch JWT token for hive operations with namespace enforcement
async fn fetch_jwt_for_hive_operation(role: &str, namespace: &str) -> Result<String> {
    // Check for environment variables first (for testing/development)
    if let Ok(jwt) = std::env::var("B00T_HIVE_JWT") {
        info!("Using JWT from B00T_HIVE_JWT environment variable");
        return Ok(jwt);
    }
    
    if let Ok(session_token) = std::env::var("B00T_SESSION_TOKEN") {
        let website_url = std::env::var("B00T_WEBSITE_URL")
            .unwrap_or_else(|_| "https://b00t.promptexecution.com".to_string());
        
        info!("Fetching JWT from b00t-website for role: {}, namespace: {}", role, namespace);
        
        match fetch_jwt_from_website(&website_url, &session_token, role).await {
            Ok(jwt) => {
                // Validate JWT and ensure namespace matches
                if let Ok(validator) = AcpJwtValidator::from_operator_jwt("placeholder") {
                    if let Ok(security_ctx) = validator.validate_jwt(&jwt) {
                        if security_ctx.namespace == namespace {
                            info!("✅ JWT validated for namespace: {}", namespace);
                            return Ok(jwt);
                        } else {
                            return Err(anyhow::anyhow!(
                                "JWT namespace '{}' does not match requested namespace '{}'",
                                security_ctx.namespace, namespace
                            ));
                        }
                    }
                }
                
                // Return JWT even if validation fails (for development)
                warn!("JWT validation failed, continuing in development mode");
                return Ok(jwt);
            }
            Err(e) => {
                warn!("Failed to fetch JWT from b00t-website: {}", e);
            }
        }
    }
    
    // For development: continue without JWT
    warn!("No JWT available - running in development mode (INSECURE)");
    warn!("Set B00T_HIVE_JWT or B00T_SESSION_TOKEN environment variable for production");
    
    Ok("development_mode_no_jwt".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_param_structures() {
        let join_params = JoinHiveParams {
            mission_id: "test_mission".to_string(),
            role: "worker".to_string(),
            namespace: None,
            nats_url: None,
        };

        assert_eq!(join_params.mission_id, "test_mission");
        assert_eq!(join_params.role, "worker");
    }

    #[test]
    fn test_nats_url_helper() {
        let url = get_nats_url();
        assert!(url.starts_with("nats://"));
    }

    #[test]
    fn test_namespace_helper() {
        let namespace = get_user_namespace();
        assert!(namespace.starts_with("account."));
    }
}