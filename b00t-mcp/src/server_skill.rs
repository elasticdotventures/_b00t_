// 🤓 b00t SkillExecutor — lazy MCP server lifecycle manager.
//    Reads [b00t.mcp.lifecycle] blocks from .mcp.toml datums, spawns servers
//    on first tool call, and reaps them after idle_timeout_secs of inactivity.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::{Child, Command};
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
use tracing::{info, warn, error, debug};

// ── Configuration types ──

#[derive(Debug, Clone, Deserialize)]
pub struct HealthCheck {
    #[serde(rename = "type")]
    pub check_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LifecycleServer {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub health_check: Option<HealthCheck>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpLifecycle {
    pub idle_timeout_secs: u64,
    #[serde(default)]
    pub servers: Vec<LifecycleServer>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpTomlConfig {
    #[serde(default)]
    pub b00t: Option<B00tSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct B00tSection {
    pub name: Option<String>,
    #[serde(default)]
    pub mcp: Option<McpSection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct McpSection {
    #[serde(default)]
    pub lifecycle: Option<McpLifecycle>,
}

// ── Runtime types ──

pub struct ManagedServer {
    pub name: String,
    pub config: LifecycleServer,
    pub idle_timeout: Duration,
    last_activity: Instant,
    child: Option<Child>,
    status: ServerStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed(String),
}

pub struct SkillExecutor {
    servers: HashMap<String, ManagedServer>,
    #[allow(dead_code)]
    reap_interval: Duration,
}

lazy_static::lazy_static! {
    static ref EXECUTOR: Arc<Mutex<SkillExecutor>> = Arc::new(Mutex::new(SkillExecutor::new()));
}

impl SkillExecutor {
    pub fn new() -> Self {
        SkillExecutor {
            servers: HashMap::new(),
            reap_interval: Duration::from_secs(30),
        }
    }

    pub async fn global() -> Arc<Mutex<SkillExecutor>> {
        EXECUTOR.clone()
    }

    /// Load lifecycle configs from .mcp.toml files in a directory.
    pub async fn load_from_dir(&mut self, dir: &Path) -> Result<usize> {
        let mut count = 0;

        let entries = match std::fs::read_dir(dir) {
            Ok(rd) => rd,
            Err(e) => {
                warn!("SkillExecutor: cannot read dir {}: {}", dir.display(), e);
                return Ok(0);
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("toml")
                && path.file_name().and_then(|s| s.to_str()).map_or(false, |n| n.ends_with(".mcp.toml"))
            {
                match self.load_one(&path) {
                    Ok(true) => count += 1,
                    Ok(false) => {} // no lifecycle block — skip
                    Err(e) => warn!("SkillExecutor: failed to load {}: {}", path.display(), e),
                }
            }
        }

        if count > 0 {
            info!("SkillExecutor: loaded {} skill lifecycle configs", count);
        }
        Ok(count)
    }

    fn load_one(&mut self, path: &Path) -> Result<bool> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let config: McpTomlConfig = toml::from_str(&content)
            .with_context(|| format!("Failed to parse {}", path.display()))?;

        let b00t = match config.b00t {
            Some(b) => b,
            None => return Ok(false),
        };

        let lifecycle = match b00t.mcp.and_then(|m| m.lifecycle) {
            Some(l) => l,
            None => return Ok(false),
        };

        let skill_name = b00t.name.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        for server_config in &lifecycle.servers {
            let managed = ManagedServer {
                name: skill_name.clone(),
                config: server_config.clone(),
                idle_timeout: Duration::from_secs(lifecycle.idle_timeout_secs),
                last_activity: Instant::now(),
                child: None,
                status: ServerStatus::Stopped,
            };
            self.servers.insert(skill_name.clone(), managed);
            debug!(
                "SkillExecutor: registered {} (cmd={}, idle_timeout={}s)",
                skill_name, server_config.command, lifecycle.idle_timeout_secs
            );
        }

        Ok(true)
    }

    /// Ensure a server is running — spawn it if dead. Returns true if newly spawned.
    pub async fn ensure_running(&mut self, name: &str) -> Result<bool> {
        let server = self.servers.get_mut(name)
            .ok_or_else(|| anyhow::anyhow!("SkillExecutor: unknown server '{}'", name))?;

        server.last_activity = Instant::now();

        match server.status {
            ServerStatus::Running => {
                debug!("SkillExecutor: {} already running", name);
                return Ok(false);
            }
            ServerStatus::Starting => {
                debug!("SkillExecutor: {} already starting", name);
                return Ok(false);
            }
            ServerStatus::Stopping => {
                warn!("SkillExecutor: {} is stopping — waiting before spawn", name);
                return Ok(false);
            }
            ServerStatus::Failed(ref err) => {
                warn!("SkillExecutor: {} was failed ({}), retrying spawn", name, err);
            }
            ServerStatus::Stopped => {}
        }

        self.spawn_server(name).await
    }

    async fn spawn_server(&mut self, name: &str) -> Result<bool> {
        let server = match self.servers.get_mut(name) {
            Some(s) => s,
            None => return Err(anyhow::anyhow!("SkillExecutor: server '{}' vanished", name)),
        };

        server.status = ServerStatus::Starting;

        let cfg = &server.config;
        info!(
            "SkillExecutor: spawning {} via `{} {}`",
            name,
            cfg.command,
            cfg.args.join(" ")
        );

        let mut cmd = Command::new(&cfg.command);
        cmd.args(&cfg.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        for (key, value) in &cfg.env {
            cmd.env(key, value);
        }

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let err_msg = format!("spawn failed: {}", e);
                error!("SkillExecutor: {} {}", name, err_msg);
                server.status = ServerStatus::Failed(err_msg);
                return Err(anyhow::anyhow!("Failed to spawn {}: {}", name, e));
            }
        };

        // Run health check if configured, otherwise just wait a brief moment
        if let Some(ref hc) = cfg.health_check {
            match hc.check_type.as_str() {
                "process_alive" => {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    match child.id() {
                        Some(pid) => info!("SkillExecutor: {} spawned (pid={})", name, pid),
                        None => {
                            let err = "process exited immediately".to_string();
                            error!("SkillExecutor: {} {}", name, err);
                            server.status = ServerStatus::Failed(err);
                            return Err(anyhow::anyhow!("{} exited immediately", name));
                        }
                    }
                }
                "tcp_port" | "http_get" => {
                    warn!(
                        "SkillExecutor: {} health_check type '{}' not yet implemented",
                        name, hc.check_type
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                other => {
                    warn!(
                        "SkillExecutor: {} unknown health_check type '{}', waiting 500ms",
                        name, other
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
            }
        } else {
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        server.child = Some(child);
        server.status = ServerStatus::Running;
        server.last_activity = Instant::now();
        Ok(true)
    }

    /// Stop a server gracefully (SIGKILL with timeout).
    pub async fn stop_server(&mut self, name: &str) -> Result<()> {
        if let Some(server) = self.servers.get_mut(name) {
            server.status = ServerStatus::Stopping;
            if let Some(mut child) = server.child.take() {
                info!("SkillExecutor: stopping {} (pid={:?})", name, child.id());
                let _ = child.start_kill();
                match tokio::time::timeout(Duration::from_secs(5), child.wait()).await {
                    Ok(Ok(status)) => debug!("SkillExecutor: {} exited with {:?}", name, status.code()),
                    Ok(Err(e)) => error!("SkillExecutor: {} wait error: {}", name, e),
                    Err(_) => warn!("SkillExecutor: {} kill timeout, orphaned?", name),
                }
            }
            server.status = ServerStatus::Stopped;
        }
        Ok(())
    }

    /// Stop all managed servers. Call on shutdown.
    pub async fn shutdown_all(&mut self) {
        let names: Vec<String> = self.servers.keys().cloned().collect();
        for name in names {
            let _ = self.stop_server(&name).await;
        }
        info!("SkillExecutor: all servers stopped");
    }

    /// Reap idle servers. Called by a background tick.
    pub async fn reap_idle(&mut self) {
        let now = Instant::now();
        let mut to_stop = Vec::new();

        for (name, server) in self.servers.iter() {
            if server.status == ServerStatus::Running {
                let idle = now.duration_since(server.last_activity);
                if idle >= server.idle_timeout {
                    warn!(
                        "SkillExecutor: {} idle for {:.0}s (timeout={}s) — reaping",
                        name,
                        idle.as_secs(),
                        server.idle_timeout.as_secs()
                    );
                    to_stop.push(name.clone());
                }
            }
        }

        for name in to_stop {
            let _ = self.stop_server(&name).await;
        }
    }

    pub fn is_running(&self, name: &str) -> bool {
        self.servers
            .get(name)
            .map(|s| s.status == ServerStatus::Running)
            .unwrap_or(false)
    }

    pub fn status(&self, name: &str) -> Option<ServerStatus> {
        self.servers.get(name).map(|s| s.status.clone())
    }

    pub fn server_names(&self) -> Vec<String> {
        self.servers.keys().cloned().collect()
    }
}

/// Start the background reap loop. Call once at application startup.
pub async fn start_reap_loop() {
    let executor = SkillExecutor::global().await;
    let mut tick = interval(Duration::from_secs(30));

    tokio::spawn(async move {
        loop {
            tick.tick().await;
            executor.lock().await.reap_idle().await;
        }
    });
}

/// Initialise the executor: load all .mcp.toml files from the b00t datums dir.
pub async fn init_executor() -> Result<usize> {
    let home = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    let datums_dir = home.join(".dotfiles").join("_b00t_");

    let arc = SkillExecutor::global().await;
    let mut executor = arc.lock().await;
    executor.load_from_dir(&datums_dir).await
}
