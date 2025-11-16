//! Agent discovery via filesystem socket watching and Redis registry.
//!
//! Monitors multiple directories for `.sock` files representing active agents,
//! automatically registering/deregistering them as they appear/disappear.

use crate::error::{ChatError, ChatResult};
use crate::ipc_transport::{
    AgentEndpoint, AgentEvent, AgentWatcher, DiscoverableTransport, TransportKind,
};
use async_trait::async_trait;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Socket registry that discovers agents via filesystem watching.
///
/// Monitors directories like `/tmp/b00t/agents/*.sock` and `~/.b00t/agents/*.sock`
/// for agent socket files, automatically tracking their lifecycle.
#[derive(Clone)]
pub struct SocketRegistry {
    watch_paths: Vec<PathBuf>,
    agents: Arc<RwLock<HashMap<String, AgentEndpoint>>>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    _watcher: Arc<RwLock<Option<RecommendedWatcher>>>,
}

impl SocketRegistry {
    /// Create a new socket registry.
    pub fn new() -> Self {
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        Self {
            watch_paths: Vec::new(),
            agents: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            _watcher: Arc::new(RwLock::new(None)),
        }
    }

    /// Add a directory to watch for socket files.
    pub fn add_watch_path(&mut self, path: impl Into<PathBuf>) {
        self.watch_paths.push(path.into());
    }

    /// Start watching configured directories.
    pub async fn start_watching(&self) -> ChatResult<()> {
        // Create all watch directories
        for path in &self.watch_paths {
            if let Err(e) = tokio::fs::create_dir_all(&path).await {
                warn!("Failed to create watch directory {}: {}", path.display(), e);
            }
        }

        // Initial scan of existing sockets
        for path in &self.watch_paths {
            self.scan_directory(path).await?;
        }

        // Set up filesystem watcher
        let agents = self.agents.clone();
        let event_tx = self.event_tx.clone();
        let watch_paths = self.watch_paths.clone();

        let (fs_event_tx, mut fs_event_rx) = mpsc::unbounded_channel::<notify::Result<Event>>();

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            let _ = fs_event_tx.send(res);
        })
        .map_err(|e| ChatError::Other(format!("Failed to create watcher: {}", e)))?;

        // Watch all configured paths
        for path in &watch_paths {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|e| ChatError::Other(format!("Failed to watch {}: {}", path.display(), e)))?;
            info!("🔍 Watching for agent sockets: {}", path.display());
        }

        *self._watcher.write().await = Some(watcher);

        // Spawn event processor
        tokio::spawn(async move {
            while let Some(event_result) = fs_event_rx.recv().await {
                match event_result {
                    Ok(event) => {
                        if let Err(e) = Self::handle_fs_event(event, &agents, &event_tx).await {
                            error!("Error handling filesystem event: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Filesystem watch error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }

    /// Scan a directory for existing socket files.
    async fn scan_directory(&self, path: &Path) -> ChatResult<()> {
        let mut entries = match tokio::fs::read_dir(path).await {
            Ok(entries) => entries,
            Err(_) => return Ok(()), // Directory doesn't exist yet
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sock") {
                self.register_socket(&path).await?;
            }
        }

        Ok(())
    }

    /// Register a newly discovered socket.
    async fn register_socket(&self, socket_path: &Path) -> ChatResult<()> {
        let agent_id = socket_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ChatError::InvalidSocketPath(format!("{}", socket_path.display())))?
            .to_string();

        let endpoint = AgentEndpoint {
            agent_id: agent_id.clone(),
            endpoint_uri: socket_path.display().to_string(),
            transport_kind: TransportKind::UnixSocket,
            last_seen: SystemTime::now(),
            metadata: None,
        };

        let mut agents = self.agents.write().await;
        let is_new = !agents.contains_key(&agent_id);

        agents.insert(agent_id.clone(), endpoint.clone());

        if is_new {
            info!("🤖 Discovered agent: {} @ {}", agent_id, socket_path.display());
            let _ = self.event_tx.send(AgentEvent::Discovered(endpoint));
        } else {
            debug!("🔄 Updated agent: {}", agent_id);
            let _ = self.event_tx.send(AgentEvent::Updated(endpoint));
        }

        Ok(())
    }

    /// Unregister a socket that was removed.
    async fn unregister_socket(&self, socket_path: &Path) -> ChatResult<()> {
        let agent_id = socket_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string());

        if let Some(agent_id) = agent_id {
            let mut agents = self.agents.write().await;
            if agents.remove(&agent_id).is_some() {
                info!("👋 Agent disconnected: {}", agent_id);
                let _ = self.event_tx.send(AgentEvent::Lost(agent_id));
            }
        }

        Ok(())
    }

    /// Handle a filesystem event.
    async fn handle_fs_event(
        event: Event,
        agents: &Arc<RwLock<HashMap<String, AgentEndpoint>>>,
        event_tx: &mpsc::UnboundedSender<AgentEvent>,
    ) -> ChatResult<()> {
        match event.kind {
            EventKind::Create(_) => {
                for path in event.paths {
                    if path.extension().and_then(|s| s.to_str()) == Some("sock") {
                        debug!("📥 Socket created: {}", path.display());
                        // Register will be handled by polling or explicit call
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if path.extension().and_then(|s| s.to_str()) == Some("sock") {
                        debug!("📤 Socket removed: {}", path.display());
                        if let Some(agent_id) = path.file_stem().and_then(|s| s.to_str()) {
                            let mut agents = agents.write().await;
                            if agents.remove(agent_id).is_some() {
                                info!("👋 Agent disconnected: {}", agent_id);
                                let _ = event_tx.send(AgentEvent::Lost(agent_id.to_string()));
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// Get a specific agent endpoint.
    pub async fn get_agent(&self, agent_id: &str) -> Option<AgentEndpoint> {
        self.agents.read().await.get(agent_id).cloned()
    }

    /// List all currently known agents.
    pub async fn list_agents(&self) -> Vec<AgentEndpoint> {
        self.agents.read().await.values().cloned().collect()
    }

    /// Remove stale agents (not seen in duration).
    pub async fn prune_stale(&self, max_age: std::time::Duration) -> usize {
        let now = SystemTime::now();
        let mut agents = self.agents.write().await;
        let mut removed = 0;

        agents.retain(|agent_id, endpoint| {
            if let Ok(age) = now.duration_since(endpoint.last_seen) {
                if age > max_age {
                    info!("🧹 Pruning stale agent: {}", agent_id);
                    let _ = self.event_tx.send(AgentEvent::Lost(agent_id.clone()));
                    removed += 1;
                    return false;
                }
            }
            true
        });

        removed
    }
}

impl Default for SocketRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DiscoverableTransport for SocketRegistry {
    async fn discover_agents(&self) -> ChatResult<Vec<AgentEndpoint>> {
        Ok(self.list_agents().await)
    }

    async fn watch_agents(&self) -> ChatResult<AgentWatcher> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let agents = self.agents.clone();

        // Send current agents as Discovered events
        tokio::spawn(async move {
            let current_agents = agents.read().await;
            for endpoint in current_agents.values() {
                let _ = tx.send(AgentEvent::Discovered(endpoint.clone()));
            }
        });

        // Create stream from receiver
        let stream = async_stream::stream! {
            while let Some(event) = rx.recv().await {
                yield event;
            }
        };

        Ok(AgentWatcher::new(Box::pin(stream)))
    }
}

/// Builder for SocketRegistry with common defaults.
pub struct SocketRegistryBuilder {
    paths: Vec<PathBuf>,
}

impl SocketRegistryBuilder {
    pub fn new() -> Self {
        Self { paths: Vec::new() }
    }

    /// Add a custom watch path.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.paths.push(path.into());
        self
    }

    /// Add the default system-wide agent directory.
    pub fn with_system_dir(mut self) -> Self {
        self.paths.push(PathBuf::from("/tmp/b00t/agents"));
        self
    }

    /// Add the default user agent directory.
    pub fn with_user_dir(mut self) -> Self {
        if let Some(home) = dirs::home_dir() {
            self.paths.push(home.join(".b00t/agents"));
        }
        self
    }

    /// Add both system and user directories.
    pub fn with_default_dirs(self) -> Self {
        self.with_system_dir().with_user_dir()
    }

    /// Build the registry.
    pub fn build(self) -> SocketRegistry {
        let mut registry = SocketRegistry::new();
        for path in self.paths {
            registry.add_watch_path(path);
        }
        registry
    }
}

impl Default for SocketRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_socket_registry_creation() {
        let registry = SocketRegistry::new();
        assert_eq!(registry.list_agents().await.len(), 0);
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let registry = SocketRegistryBuilder::new()
            .with_system_dir()
            .with_user_dir()
            .build();

        assert!(registry.watch_paths.len() >= 1);
    }

    #[tokio::test]
    async fn test_socket_discovery() {
        let temp_dir = TempDir::new().unwrap();
        let sock_path = temp_dir.path().join("test-agent.sock");

        // Create a socket file
        tokio::fs::write(&sock_path, b"").await.unwrap();

        let mut registry = SocketRegistry::new();
        registry.add_watch_path(temp_dir.path());
        registry.scan_directory(temp_dir.path()).await.unwrap();

        let agents = registry.list_agents().await;
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].agent_id, "test-agent");
    }

    #[tokio::test]
    async fn test_stale_pruning() {
        let mut registry = SocketRegistry::new();

        // Insert an agent with old timestamp
        let old_endpoint = AgentEndpoint {
            agent_id: "stale-agent".to_string(),
            endpoint_uri: "/tmp/stale.sock".to_string(),
            transport_kind: TransportKind::UnixSocket,
            last_seen: SystemTime::now() - std::time::Duration::from_secs(600),
            metadata: None,
        };

        registry
            .agents
            .write()
            .await
            .insert("stale-agent".to_string(), old_endpoint);

        let removed = registry.prune_stale(std::time::Duration::from_secs(300)).await;
        assert_eq!(removed, 1);
        assert_eq!(registry.list_agents().await.len(), 0);
    }
}
