//! Agent lifecycle management with TOML configuration loading.
//!
//! Provides utilities for spawning, managing, and coordinating b00t agents
//! from `.agent.toml` configuration files.

use crate::agent_coordination::{AgentCoordinator, AgentMetadata};
use crate::redis::{AgentStatus, RedisComms, RedisConfig};
use crate::B00tResult;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UnixListener;
use tracing::{error, info};

/// Agent configuration loaded from TOML files.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub b00t: B00tConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct B00tConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub agent_type: String,
    pub hint: Option<String>,
    pub agent: AgentDef,
    pub env: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentDef {
    pub pid: String,
    pub branch: Option<String>,
    pub model: Option<String>,
    pub skills: Vec<String>,
    pub personality: Option<String>,
    pub humor: Option<String>,
    pub role: String,
    pub ipc: IpcConfig,
    pub crew: CrewConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IpcConfig {
    pub socket: String,
    pub pubsub: bool,
    pub protocol: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CrewConfig {
    pub role: String,
    pub captain: bool,
}

/// Handle to a running agent.
pub struct AgentHandle {
    pub config: AgentConfig,
    pub socket_path: PathBuf,
    pub coordinator: AgentCoordinator,
    _listener: Option<UnixListener>,
}

impl AgentHandle {
    /// Get the agent ID.
    pub fn agent_id(&self) -> &str {
        &self.config.b00t.name
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Get coordinator reference.
    pub fn coordinator(&self) -> &AgentCoordinator {
        &self.coordinator
    }
}

impl Drop for AgentHandle {
    fn drop(&mut self) {
        // Clean up socket file on drop
        if self.socket_path.exists() {
            if let Err(e) = std::fs::remove_file(&self.socket_path) {
                error!("Failed to remove socket {}: {}", self.socket_path.display(), e);
            } else {
                info!("🧹 Cleaned up socket: {}", self.socket_path.display());
            }
        }
    }
}

/// Agent manager for spawning and coordinating agents.
pub struct AgentManager {
    redis_config: RedisConfig,
}

impl AgentManager {
    /// Create a new agent manager.
    pub fn new(redis_config: RedisConfig) -> Self {
        Self { redis_config }
    }

    /// Load agent configuration from a TOML file.
    pub async fn load_config(path: &Path) -> B00tResult<AgentConfig> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context(format!("Failed to read agent config: {}", path.display()))?;

        let config: AgentConfig = toml::from_str(&content)
            .context(format!("Failed to parse agent config: {}", path.display()))?;

        Ok(config)
    }

    /// Spawn an agent from a configuration file.
    pub async fn spawn_agent(&self, config_path: &Path) -> B00tResult<AgentHandle> {
        let config = Self::load_config(config_path).await?;

        info!("🚀 Spawning agent: {}", config.b00t.name);

        // Create agent socket
        let socket_path = PathBuf::from(&config.b00t.agent.ipc.socket);
        if let Some(parent) = socket_path.parent() {
            tokio::fs::create_dir_all(parent).await.context(format!(
                "Failed to create socket directory: {}",
                parent.display()
            ))?;
        }

        // Remove stale socket if exists
        if socket_path.exists() {
            tokio::fs::remove_file(&socket_path).await.ok();
        }

        // Create Unix socket listener
        let listener = UnixListener::bind(&socket_path).context(format!(
            "Failed to bind agent socket: {}",
            socket_path.display()
        ))?;

        info!("🔌 Agent socket bound: {}", socket_path.display());

        // Create Redis connection
        let redis = RedisComms::new(self.redis_config.clone(), config.b00t.agent.pid.clone())?;

        // Build agent metadata
        let metadata = AgentMetadata {
            agent_id: config.b00t.name.clone(),
            agent_role: config.b00t.agent.role.clone(),
            capabilities: config.b00t.agent.skills.clone(),
            crew: Some(config.b00t.agent.crew.role.clone()),
            status: AgentStatus::Online,
            last_seen: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
            load: 0.0,
            specializations: HashMap::new(),
        };

        // Create coordinator
        let mut coordinator = AgentCoordinator::new(redis, metadata);
        coordinator.start().await?;

        info!("✅ Agent {} started successfully", config.b00t.name);

        Ok(AgentHandle {
            config,
            socket_path,
            coordinator,
            _listener: Some(listener),
        })
    }

    /// Spawn multiple agents from a directory of config files.
    pub async fn spawn_from_directory(&self, dir: &Path) -> B00tResult<Vec<AgentHandle>> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .context(format!("Failed to read directory: {}", dir.display()))?;

        let mut handles = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml")
                && path.file_name().and_then(|s| s.to_str()).map(|s| s.ends_with(".agent.toml")).unwrap_or(false)
            {
                match self.spawn_agent(&path).await {
                    Ok(handle) => handles.push(handle),
                    Err(e) => {
                        error!("Failed to spawn agent from {}: {}", path.display(), e);
                    }
                }
            }
        }

        info!("🎉 Spawned {} agents from {}", handles.len(), dir.display());

        Ok(handles)
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        Self::new(RedisConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.agent.toml");

        let config_content = r#"
[b00t]
name = "test-agent"
type = "agent"
hint = "Test agent"

[b00t.agent]
pid = "test-pid-123"
skills = ["rust", "testing"]
role = "specialist"

[b00t.agent.ipc]
socket = "/tmp/test.sock"
pubsub = true
protocol = "msgpack"

[b00t.agent.crew]
role = "test-crew"
captain = false
"#;

        tokio::fs::write(&config_path, config_content).await.unwrap();

        let config = AgentManager::load_config(&config_path).await.unwrap();
        assert_eq!(config.b00t.name, "test-agent");
        assert_eq!(config.b00t.agent.skills, vec!["rust", "testing"]);
    }

    #[test]
    fn test_agent_manager_creation() {
        let manager = AgentManager::default();
        assert_eq!(manager.redis_config.host, "localhost");
    }
}
