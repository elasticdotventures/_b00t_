// sandbox/mod.rs
// Sandbox lifecycle management: startup, MCP reloading, shutdown
// Handles dynamic reconfiguration of sandbox environments

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Sandbox state in the orchestration system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SandboxState {
    Initializing,
    Ready,
    ReconfigurationPending,
    Reloading,
    Degraded,
    Shutdown,
}

impl std::fmt::Display for SandboxState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SandboxState::Initializing => write!(f, "Initializing"),
            SandboxState::Ready => write!(f, "Ready"),
            SandboxState::ReconfigurationPending => write!(f, "ReconfigurationPending"),
            SandboxState::Reloading => write!(f, "Reloading"),
            SandboxState::Degraded => write!(f, "Degraded"),
            SandboxState::Shutdown => write!(f, "Shutdown"),
        }
    }
}

/// MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MCPServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub environment: BTreeMap<String, String>,
    #[serde(default)]
    pub enabled: bool,
}

/// Sandbox configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SandboxConfig {
    pub name: String,
    pub agent_role: String,
    #[serde(default)]
    pub mcp_servers: Vec<MCPServerConfig>,
    #[serde(default)]
    pub max_token_budget: u32,
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default)]
    pub restricted_paths: Vec<String>,
}

/// Sandbox instance with lifecycle state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxInstance {
    pub id: String,
    pub config: SandboxConfig,
    pub state: SandboxState,
    pub loaded_mcps: Vec<String>,
    pub active_since: String, // ISO8601 timestamp
    pub last_reload: Option<String>,
}

/// Sandbox lifecycle management
pub struct SandboxManager {
    instances: BTreeMap<String, SandboxInstance>,
}

impl SandboxManager {
    /// Create a new sandbox manager
    pub fn new() -> Self {
        SandboxManager {
            instances: BTreeMap::new(),
        }
    }

    /// Initialize a new sandbox from configuration
    pub fn initialize(&mut self, config: SandboxConfig) -> Result<SandboxInstance, String> {
        let id = format!("sandbox:{}", config.name);

        if self.instances.contains_key(&id) {
            return Err(format!("Sandbox '{}' already exists", id));
        }

        let instance = SandboxInstance {
            id: id.clone(),
            config: config.clone(),
            state: SandboxState::Initializing,
            loaded_mcps: vec![],
            active_since: chrono::Utc::now().to_rfc3339(),
            last_reload: None,
        };

        self.instances.insert(id, instance.clone());
        Ok(instance)
    }

    /// Load MCP servers into a sandbox
    pub fn load_mcps(&mut self, sandbox_id: &str, servers: Vec<String>) -> Result<(), String> {
        let instance = self
            .instances
            .get_mut(sandbox_id)
            .ok_or_else(|| format!("Sandbox '{}' not found", sandbox_id))?;

        // Validate MCPs
        for server_name in &servers {
            if let Some(config) = instance
                .config
                .mcp_servers
                .iter()
                .find(|m| &m.name == server_name)
            {
                if !config.enabled {
                    return Err(format!("MCP server '{}' is disabled", server_name));
                }
            } else {
                return Err(format!("MCP server '{}' not configured", server_name));
            }
        }

        instance.loaded_mcps = servers;
        instance.state = SandboxState::Ready;
        instance.last_reload = Some(chrono::Utc::now().to_rfc3339());

        Ok(())
    }

    /// Request reconfiguration with new MCPs
    pub fn request_reload(
        &mut self,
        sandbox_id: &str,
        new_servers: Vec<String>,
    ) -> Result<(), String> {
        let instance = self
            .instances
            .get_mut(sandbox_id)
            .ok_or_else(|| format!("Sandbox '{}' not found", sandbox_id))?;

        if instance.state == SandboxState::Shutdown {
            return Err("Cannot reload shutdown sandbox".to_string());
        }

        // Validate new servers
        for server_name in &new_servers {
            if !instance
                .config
                .mcp_servers
                .iter()
                .any(|m| &m.name == server_name && m.enabled)
            {
                return Err(format!("Cannot load disabled/missing MCP: {}", server_name));
            }
        }

        instance.state = SandboxState::ReconfigurationPending;
        Ok(())
    }

    /// Perform the actual reload operation
    pub fn perform_reload(
        &mut self,
        sandbox_id: &str,
        new_servers: Vec<String>,
    ) -> Result<(), String> {
        let instance = self
            .instances
            .get_mut(sandbox_id)
            .ok_or_else(|| format!("Sandbox '{}' not found", sandbox_id))?;

        if instance.state != SandboxState::ReconfigurationPending {
            return Err("Reload not requested".to_string());
        }

        instance.state = SandboxState::Reloading;

        // 🤓 TODO: Actually kill old MCP processes and start new ones
        // This would involve:
        // - Send SIGTERM to old MCP processes
        // - Wait for graceful shutdown
        // - Start new MCP processes from config
        // - Verify connectivity

        instance.loaded_mcps = new_servers;
        instance.last_reload = Some(chrono::Utc::now().to_rfc3339());
        instance.state = SandboxState::Ready;

        Ok(())
    }

    /// Shut down a sandbox
    pub fn shutdown(&mut self, sandbox_id: &str) -> Result<(), String> {
        let instance = self
            .instances
            .get_mut(sandbox_id)
            .ok_or_else(|| format!("Sandbox '{}' not found", sandbox_id))?;

        // 🤓 TODO: Kill all MCP processes
        instance.state = SandboxState::Shutdown;
        instance.loaded_mcps.clear();

        Ok(())
    }

    /// Get sandbox instance
    pub fn get(&self, sandbox_id: &str) -> Option<&SandboxInstance> {
        self.instances.get(sandbox_id)
    }

    /// Get mutable sandbox instance
    pub fn get_mut(&mut self, sandbox_id: &str) -> Option<&mut SandboxInstance> {
        self.instances.get_mut(sandbox_id)
    }

    /// List all sandboxes
    pub fn list(&self) -> Vec<&SandboxInstance> {
        self.instances.values().collect()
    }

    /// Get sandboxes in a specific state
    pub fn list_by_state(&self, state: SandboxState) -> Vec<&SandboxInstance> {
        self.instances
            .values()
            .filter(|s| s.state == state)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_sandbox() {
        let mut manager = SandboxManager::new();

        let config = SandboxConfig {
            name: "test-sandbox".to_string(),
            agent_role: "executor".to_string(),
            mcp_servers: vec![MCPServerConfig {
                name: "context7".to_string(),
                command: "context7".to_string(),
                args: vec!["--mode=server".to_string()],
                environment: BTreeMap::new(),
                enabled: true,
            }],
            max_token_budget: 10000,
            allowed_commands: vec!["b00t".to_string()],
            restricted_paths: vec!["/etc".to_string(), "/root".to_string()],
        };

        let instance = manager.initialize(config).expect("Should initialize");
        assert_eq!(instance.state, SandboxState::Initializing);
        assert!(instance.loaded_mcps.is_empty());
    }

    #[test]
    fn test_load_mcps() {
        let mut manager = SandboxManager::new();

        let config = SandboxConfig {
            name: "test-sandbox".to_string(),
            agent_role: "executor".to_string(),
            mcp_servers: vec![MCPServerConfig {
                name: "context7".to_string(),
                command: "context7".to_string(),
                args: vec![],
                environment: BTreeMap::new(),
                enabled: true,
            }],
            max_token_budget: 10000,
            allowed_commands: vec![],
            restricted_paths: vec![],
        };

        let instance = manager.initialize(config).expect("Should initialize");
        let sandbox_id = instance.id.clone();

        manager
            .load_mcps(&sandbox_id, vec!["context7".to_string()])
            .expect("Should load MCPs");

        let updated = manager.get(&sandbox_id).unwrap();
        assert_eq!(updated.state, SandboxState::Ready);
        assert!(updated.loaded_mcps.contains(&"context7".to_string()));
    }

    #[test]
    fn test_cannot_load_disabled_mcp() {
        let mut manager = SandboxManager::new();

        let config = SandboxConfig {
            name: "test-sandbox".to_string(),
            agent_role: "executor".to_string(),
            mcp_servers: vec![MCPServerConfig {
                name: "disabled-mcp".to_string(),
                command: "disabled".to_string(),
                args: vec![],
                environment: BTreeMap::new(),
                enabled: false,
            }],
            max_token_budget: 10000,
            allowed_commands: vec![],
            restricted_paths: vec![],
        };

        let instance = manager.initialize(config).expect("Should initialize");
        let sandbox_id = instance.id.clone();

        let result = manager.load_mcps(&sandbox_id, vec!["disabled-mcp".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_request_reload_transitions_state() {
        let mut manager = SandboxManager::new();

        let config = SandboxConfig {
            name: "test-sandbox".to_string(),
            agent_role: "executor".to_string(),
            mcp_servers: vec![
                MCPServerConfig {
                    name: "old-mcp".to_string(),
                    command: "old".to_string(),
                    args: vec![],
                    environment: BTreeMap::new(),
                    enabled: true,
                },
                MCPServerConfig {
                    name: "new-mcp".to_string(),
                    command: "new".to_string(),
                    args: vec![],
                    environment: BTreeMap::new(),
                    enabled: true,
                },
            ],
            max_token_budget: 10000,
            allowed_commands: vec![],
            restricted_paths: vec![],
        };

        let instance = manager.initialize(config).expect("Should initialize");
        let sandbox_id = instance.id.clone();

        manager
            .load_mcps(&sandbox_id, vec!["old-mcp".to_string()])
            .expect("Should load initial MCPs");

        manager
            .request_reload(&sandbox_id, vec!["new-mcp".to_string()])
            .expect("Should request reload");

        let updated = manager.get(&sandbox_id).unwrap();
        assert_eq!(updated.state, SandboxState::ReconfigurationPending);
    }

    #[test]
    fn test_perform_reload_after_request() {
        let mut manager = SandboxManager::new();

        let config = SandboxConfig {
            name: "test-sandbox".to_string(),
            agent_role: "executor".to_string(),
            mcp_servers: vec![
                MCPServerConfig {
                    name: "mcp1".to_string(),
                    command: "cmd1".to_string(),
                    args: vec![],
                    environment: BTreeMap::new(),
                    enabled: true,
                },
                MCPServerConfig {
                    name: "mcp2".to_string(),
                    command: "cmd2".to_string(),
                    args: vec![],
                    environment: BTreeMap::new(),
                    enabled: true,
                },
            ],
            max_token_budget: 10000,
            allowed_commands: vec![],
            restricted_paths: vec![],
        };

        let instance = manager.initialize(config).expect("Should initialize");
        let sandbox_id = instance.id.clone();

        manager
            .load_mcps(&sandbox_id, vec!["mcp1".to_string()])
            .expect("Should load initial");

        manager
            .request_reload(&sandbox_id, vec!["mcp2".to_string()])
            .expect("Should request reload");

        manager
            .perform_reload(&sandbox_id, vec!["mcp2".to_string()])
            .expect("Should perform reload");

        let updated = manager.get(&sandbox_id).unwrap();
        assert_eq!(updated.state, SandboxState::Ready);
        assert_eq!(updated.loaded_mcps, vec!["mcp2".to_string()]);
    }

    #[test]
    fn test_shutdown_sandbox() {
        let mut manager = SandboxManager::new();

        let config = SandboxConfig {
            name: "test-sandbox".to_string(),
            agent_role: "executor".to_string(),
            mcp_servers: vec![],
            max_token_budget: 10000,
            allowed_commands: vec![],
            restricted_paths: vec![],
        };

        let instance = manager.initialize(config).expect("Should initialize");
        let sandbox_id = instance.id.clone();

        manager.shutdown(&sandbox_id).expect("Should shutdown");

        let updated = manager.get(&sandbox_id).unwrap();
        assert_eq!(updated.state, SandboxState::Shutdown);
        assert!(updated.loaded_mcps.is_empty());
    }

    #[test]
    fn test_list_sandboxes_by_state() {
        let mut manager = SandboxManager::new();

        for i in 0..3 {
            let config = SandboxConfig {
                name: format!("sandbox-{}", i),
                agent_role: "test".to_string(),
                mcp_servers: vec![],
                max_token_budget: 10000,
                allowed_commands: vec![],
                restricted_paths: vec![],
            };

            manager.initialize(config).expect("Should initialize");
        }

        let initializing = manager.list_by_state(SandboxState::Initializing);
        assert_eq!(initializing.len(), 3);
    }
}
