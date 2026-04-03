//! Agent lifecycle management with TOML configuration loading.
//!
//! Provides utilities for spawning, managing, and coordinating b00t agents
//! from `.agent.toml` configuration files.

use crate::B00tResult;
use crate::agent_coordination::{AgentCoordinator, AgentMetadata};
use crate::redis::{AgentStatus, RedisComms, RedisConfig};
use crate::runtime_env::sandbox_root_cause_hint;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UnixListener;
use tracing::{error, info, warn};

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
    pub executor: Option<ExecutorConfig>,
}

/// Executor config from [b00t.agent.executor] — drives the deterministic tool-call loop.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExecutorConfig {
    /// CLI binary to invoke (e.g. "pi")
    pub cli_path: String,
    /// Args passed before the prompt (e.g. ["-p", "--provider", "llama-cpp", "--model", "ch0nky"])
    #[serde(default)]
    pub cli_args: Vec<String>,
    /// Tool names this executor can handle (deterministic dispatch, no LLM)
    #[serde(default)]
    pub supports_tools: Vec<String>,
    /// Max tool-call iterations before returning (default: 10)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: usize,
    /// systemd units required before invoking
    #[serde(default)]
    pub requires: Vec<String>,
    /// b00t hive profile to activate
    pub hive_profile: Option<String>,
}

fn default_max_iterations() -> usize { 10 }

// ── Tool call protocol (pi JSON output) ──────────────────────────────────────

/// A single tool call emitted by pi in -p mode.
#[derive(Debug, Deserialize)]
struct ToolCall {
    name: String,
    #[serde(default)]
    args: serde_json::Value,
}

/// Top-level pi -p output when it wants to call a tool.
#[derive(Debug, Deserialize)]
struct PiToolOutput {
    tool_code: Vec<ToolCall>,
}

// ── Deterministic tool dispatcher ────────────────────────────────────────────

/// Execute a single tool call. Pure OS operations — no LLM involved.
/// Returns (tool_name, result_string) for feeding back to the agent.
pub fn dispatch_tool(call: &ToolCall) -> String {
    match call.name.as_str() {
        "read" => {
            let path = call.args.get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match std::fs::read_to_string(path) {
                Ok(content) => format!("file content of {}:\n{}", path, content),
                Err(e) => format!("error reading {}: {}", path, e),
            }
        }
        "write" | "edit" => {
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = call.args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            match std::fs::write(path, content) {
                Ok(_) => format!("wrote {} bytes to {}", content.len(), path),
                Err(e) => format!("error writing {}: {}", path, e),
            }
        }
        "bash" | "shell" | "run" => {
            let cmd = call.args.get("command")
                .or_else(|| call.args.get("cmd"))
                .or_else(|| call.args.get("shell"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // 🤓 deterministic: fixed shell, no PATH injection risk from agent output
            match Command::new("bash").arg("-c").arg(cmd).output() {
                Ok(out) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    if out.status.success() {
                        stdout.into_owned()
                    } else {
                        format!("exit {}: {}{}", out.status.code().unwrap_or(-1), stdout, stderr)
                    }
                }
                Err(e) => format!("bash error: {}", e),
            }
        }
        "grep" => {
            let pattern = call.args.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            match Command::new("grep").args(["-rn", pattern, path]).output() {
                Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
                Err(e) => format!("grep error: {}", e),
            }
        }
        "ls" | "find" => {
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            match Command::new("ls").arg("-la").arg(path).output() {
                Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
                Err(e) => format!("ls error: {}", e),
            }
        }
        unknown => format!("unsupported tool: {}", unknown),
    }
}

/// Parse pi -p output: returns Some(tool_calls) if tool dispatch needed, None if final answer.
fn parse_pi_output(raw: &str) -> Option<Vec<ToolCall>> {
    // pi emits JSON with tool_code key when it wants tool execution
    if let Ok(parsed) = serde_json::from_str::<PiToolOutput>(raw.trim()) {
        if !parsed.tool_code.is_empty() {
            return Some(parsed.tool_code);
        }
    }
    None
}

/// Invoke an agent executor in a deterministic tool-call loop.
/// Feeds pi a prompt, executes any tool calls it requests, returns final response.
pub fn invoke_agent_executor(
    executor: &ExecutorConfig,
    env: &HashMap<String, String>,
    initial_prompt: &str,
) -> anyhow::Result<String> {
    let mut context = initial_prompt.to_string();

    for iteration in 0..executor.max_iterations {
        // Spawn agent with current context as stdin
        let mut child = Command::new(&executor.cli_path)
            .args(&executor.cli_args)
            .envs(env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn {}", executor.cli_path))?;

        // Write context to stdin
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(context.as_bytes());
        }

        let out = child.wait_with_output()
            .with_context(|| format!("agent {} failed", executor.cli_path))?;

        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr);

        if !stderr.is_empty() {
            warn!("agent stderr [iter {}]: {}", iteration, stderr.trim());
        }

        // Check for tool calls
        match parse_pi_output(&stdout) {
            Some(calls) => {
                info!("agent tool call [iter {}]: {} call(s)", iteration, calls.len());
                // Execute each tool, accumulate results
                let mut results = Vec::new();
                for call in &calls {
                    let result = dispatch_tool(call);
                    info!("tool {} → {} bytes", call.name, result.len());
                    results.push(format!("[tool:{}]\n{}", call.name, result));
                }
                // Re-invoke with original prompt + tool results appended
                context = format!("{}\n\nTool results:\n{}", initial_prompt, results.join("\n---\n"));
            }
            None => {
                // Final answer — no more tool calls
                info!("agent complete after {} iteration(s)", iteration + 1);
                return Ok(stdout);
            }
        }
    }

    anyhow::bail!("agent exceeded max_iterations ({})", executor.max_iterations)
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
                error!(
                    "Failed to remove socket {}: {}",
                    self.socket_path.display(),
                    e
                );
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
        let listener = UnixListener::bind(&socket_path).with_context(|| {
            let mut message = format!("Failed to bind agent socket: {}", socket_path.display());
            if let Some(hint) = sandbox_root_cause_hint("Unix socket bind") {
                message.push(' ');
                message.push_str(&hint);
            }
            message
        })?;

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
                && path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.ends_with(".agent.toml"))
                    .unwrap_or(false)
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

        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();

        let config = AgentManager::load_config(&config_path).await.unwrap();
        assert_eq!(config.b00t.name, "test-agent");
        assert_eq!(config.b00t.agent.skills, vec!["rust", "testing"]);
    }

    #[tokio::test]
    async fn test_load_config_with_executor() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("pi.agent.toml");
        let config_content = r#"
[b00t]
name = "pi"
type = "agent"
hint = "pi coding agent"

[b00t.agent]
pid = "pi-001"
skills = ["code"]
role = "specialist"

[b00t.agent.ipc]
socket = "/tmp/pi.sock"
pubsub = false
protocol = "json"

[b00t.agent.crew]
role = "specialist"
captain = false

[b00t.agent.executor]
cli_path = "pi"
cli_args = ["-p", "--provider", "llama-cpp", "--model", "ch0nky"]
supports_tools = ["read", "bash", "write"]
max_iterations = 10
"#;
        tokio::fs::write(&config_path, config_content).await.unwrap();
        let config = AgentManager::load_config(&config_path).await.unwrap();
        let exec = config.b00t.agent.executor.unwrap();
        assert_eq!(exec.cli_path, "pi");
        assert_eq!(exec.max_iterations, 10);
        assert!(exec.supports_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn test_dispatch_tool_read() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "hello b00t").unwrap();
        let call = ToolCall {
            name: "read".to_string(),
            args: serde_json::json!({"path": tmp.path().to_str().unwrap()}),
        };
        let result = dispatch_tool(&call);
        assert!(result.contains("hello b00t"), "read tool must return file content: {}", result);
    }

    #[test]
    fn test_dispatch_tool_bash() {
        let call = ToolCall {
            name: "bash".to_string(),
            args: serde_json::json!({"command": "echo deterministic"}),
        };
        let result = dispatch_tool(&call);
        assert_eq!(result.trim(), "deterministic");
    }

    #[test]
    fn test_dispatch_tool_write() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let call = ToolCall {
            name: "write".to_string(),
            args: serde_json::json!({"path": tmp.path().to_str().unwrap(), "content": "b00t hive"}),
        };
        let result = dispatch_tool(&call);
        assert!(result.contains("b00t hive") || result.contains("wrote"), "{}", result);
        assert_eq!(std::fs::read_to_string(tmp.path()).unwrap(), "b00t hive");
    }

    #[test]
    fn test_parse_pi_output_tool_call() {
        let raw = r#"{"tool_code": [{"name": "read", "args": {"path": "/tmp/x"}}]}"#;
        let calls = parse_pi_output(raw).unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
    }

    #[test]
    fn test_parse_pi_output_final_answer() {
        // Plain text → no tool calls → final answer
        let raw = "fn add(a: i32, b: i32) -> i32 { a + b }";
        assert!(parse_pi_output(raw).is_none());
    }

    #[test]
    fn test_agent_manager_creation() {
        let manager = AgentManager::default();
        assert_eq!(manager.redis_config.host, "localhost");
    }
}
