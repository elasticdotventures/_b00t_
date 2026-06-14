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

fn default_max_iterations() -> usize {
    10
}

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
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            match std::fs::read_to_string(path) {
                Ok(content) => format!("file content of {}:\n{}", path, content),
                Err(e) => format!("error reading {}: {}", path, e),
            }
        }
        "write" | "edit" => {
            let path = call.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
            let content = call
                .args
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match std::fs::write(path, content) {
                Ok(_) => format!("wrote {} bytes to {}", content.len(), path),
                Err(e) => format!("error writing {}: {}", path, e),
            }
        }
        "bash" | "shell" | "run" => {
            let cmd = call
                .args
                .get("command")
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
                        format!(
                            "exit {}: {}{}",
                            out.status.code().unwrap_or(-1),
                            stdout,
                            stderr
                        )
                    }
                }
                Err(e) => format!("bash error: {}", e),
            }
        }
        "grep" => {
            let pattern = call
                .args
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let path = call
                .args
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            match Command::new("grep").args(["-rn", pattern, path]).output() {
                Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
                Err(e) => format!("grep error: {}", e),
            }
        }
        "ls" | "find" => {
            let path = call
                .args
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(".");
            match Command::new("ls").arg("-la").arg(path).output() {
                Ok(out) => String::from_utf8_lossy(&out.stdout).into_owned(),
                Err(e) => format!("ls error: {}", e),
            }
        }
        unknown => format!("unsupported tool: {}", unknown),
    }
}

//// Parse pi -p output: returns Some(tool_calls) if tool dispatch needed, None if final answer.
///
/// Handles two formats:
/// 1. JSON: `{"tool_code": [{"name": "bash", "args": {"command": "..."}}]}`
/// 2. Pythonic (gemma4/vLLM): `call:toolname{key:val,key2:val2}` — one call per output line
fn parse_pi_output(raw: &str) -> Option<Vec<ToolCall>> {
    let trimmed = raw.trim();

    // Format 1: JSON tool_code envelope (legacy / other providers)
    if trimmed.starts_with('{') {
        match serde_json::from_str::<PiToolOutput>(trimmed) {
            Ok(parsed) if !parsed.tool_code.is_empty() => return Some(parsed.tool_code),
            Ok(_) => {} // valid JSON but empty tool_code — treat as final answer
            Err(e) => {
                warn!(
                    "parse_pi_output: JSON parse failed (treating as final answer): {}",
                    e
                );
            }
        }
        return None;
    }

    // Format 2: pythonic `call:name{...}` — emitted by gemma4 via vLLM
    // 🤓 gemma4 outputs: call:bash{command:echo hi} or call:write{content:...,path:/foo}
    //    Multiple calls may appear as multiple lines; collect all.
    let calls: Vec<ToolCall> = trimmed
        .lines()
        .filter_map(|line| parse_pythonic_call(line.trim()))
        .collect();

    if calls.is_empty() { None } else { Some(calls) }
}

/// Parse a single `call:toolname{key:val,key2:val2}` line into a ToolCall.
///
/// Splitting strategy: find all `,([a-z_]+):` boundaries using regex-free scan.
/// Values may contain commas (e.g. `content:hello, world,path:/foo`), but key
/// names are always simple lowercase identifiers — scan for `,word:` patterns.
fn parse_pythonic_call(line: &str) -> Option<ToolCall> {
    // Must start with "call:"
    let rest = line.strip_prefix("call:")?;

    // Split on first `{` to get tool name
    let brace_pos = rest.find('{')?;
    let tool_name = rest[..brace_pos].trim().to_string();
    if tool_name.is_empty() {
        return None;
    }

    // Extract interior between `{` and trailing `}`
    let interior = rest[brace_pos + 1..].strip_suffix('}')?.trim();

    // Build args JSON by splitting on `,identifier:` boundaries
    let args = parse_pythonic_args(interior);

    Some(ToolCall {
        name: tool_name,
        args,
    })
}

/// Split `key:val,key2:val2` into a serde_json::Value object.
/// Keys are lowercase ASCII identifiers; values may contain commas.
/// Strategy: scan for `,<ident>:` boundaries (ident = [a-z_][a-z0-9_]*).
fn parse_pythonic_args(s: &str) -> serde_json::Value {
    if s.is_empty() {
        return serde_json::Value::Object(Default::default());
    }

    // Find all split positions: index of the comma before each `key:`
    let mut split_positions: Vec<usize> = vec![0];
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b',' {
            // Check if what follows is `ident:` where ident is [a-z_][a-z0-9_]*
            let start = i + 1;
            let mut j = start;
            while j < len
                && (bytes[j].is_ascii_lowercase()
                    || bytes[j] == b'_'
                    || (j > start && bytes[j].is_ascii_digit()))
            {
                j += 1;
            }
            if j > start && j < len && bytes[j] == b':' {
                // Confirmed: comma at `i` is a key boundary
                split_positions.push(i + 1); // start of next key
            }
        }
        i += 1;
    }

    // Extract key-value pairs using split positions
    let mut map = serde_json::Map::new();
    for (idx, &start) in split_positions.iter().enumerate() {
        let end = if idx + 1 < split_positions.len() {
            split_positions[idx + 1] - 1 // exclude the comma
        } else {
            len
        };
        let segment = &s[start..end];
        if let Some(colon) = segment.find(':') {
            let key = segment[..colon].trim().to_string();
            let val = segment[colon + 1..].to_string();
            if !key.is_empty() {
                map.insert(key, serde_json::Value::String(val));
            }
        }
    }

    serde_json::Value::Object(map)
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
            stdin
                .write_all(context.as_bytes())
                .with_context(|| format!("failed to write stdin to {}", executor.cli_path))?;
        }

        let out = child
            .wait_with_output()
            .with_context(|| format!("agent {} failed", executor.cli_path))?;

        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&out.stderr);

        if !stderr.is_empty() {
            warn!("agent stderr [iter {}]: {}", iteration, stderr.trim());
        }

        // Check for tool calls
        match parse_pi_output(&stdout) {
            Some(calls) => {
                info!(
                    "agent tool call [iter {}]: {} call(s)",
                    iteration,
                    calls.len()
                );
                // Execute each tool, accumulate results
                let mut results = Vec::new();
                for call in &calls {
                    let result = dispatch_tool(call);
                    info!("tool {} → {} bytes", call.name, result.len());
                    results.push(format!("[tool:{}]\n{}", call.name, result));
                }
                // Re-invoke with original prompt + tool results appended
                context = format!(
                    "{}\n\nTool results:\n{}",
                    initial_prompt,
                    results.join("\n---\n")
                );
            }
            None => {
                // Final answer — no more tool calls
                info!("agent complete after {} iteration(s)", iteration + 1);
                return Ok(stdout);
            }
        }
    }

    anyhow::bail!(
        "agent exceeded max_iterations ({})",
        executor.max_iterations
    )
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
    pub socket_path: Option<PathBuf>,
    pub coordinator: AgentCoordinator,
    _listener: Option<UnixListener>,
}

impl AgentHandle {
    /// Get the agent ID.
    pub fn agent_id(&self) -> &str {
        &self.config.b00t.name
    }

    /// Get the socket path.
    pub fn socket_path(&self) -> Option<&Path> {
        self.socket_path.as_deref()
    }

    /// Get coordinator reference.
    pub fn coordinator(&self) -> &AgentCoordinator {
        &self.coordinator
    }
}

impl Drop for AgentHandle {
    fn drop(&mut self) {
        // Clean up socket file on drop
        if let Some(socket_path) = &self.socket_path {
            if !socket_path.exists() {
                return;
            }

            if let Err(e) = std::fs::remove_file(socket_path) {
                error!("Failed to remove socket {}: {}", socket_path.display(), e);
            } else {
                info!("🧹 Cleaned up socket: {}", socket_path.display());
            }
        }
    }
}

fn configured_socket_path(config: &AgentConfig) -> Option<PathBuf> {
    let socket = config.b00t.agent.ipc.socket.trim();
    if socket.is_empty() {
        None
    } else {
        Some(PathBuf::from(socket))
    }
}

fn is_agent_config_file(path: &Path) -> bool {
    let Some(file_name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };
    matches!(
        path.extension().and_then(|s| s.to_str()),
        Some("toml") | Some("tomllm") | Some("tomllmd")
    ) && (file_name.contains(".agent.") || file_name.ends_with(".agent"))
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

        let socket_path = configured_socket_path(&config);
        let listener = if let Some(socket_path) = socket_path.as_ref() {
            if let Some(parent) = socket_path.parent() {
                tokio::fs::create_dir_all(parent).await.context(format!(
                    "Failed to create socket directory: {}",
                    parent.display()
                ))?;
            }

            // Remove stale socket if exists
            if socket_path.exists() {
                tokio::fs::remove_file(socket_path).await.ok();
            }

            // Create Unix socket listener
            let listener = UnixListener::bind(socket_path).with_context(|| {
                let mut message = format!("Failed to bind agent socket: {}", socket_path.display());
                if let Some(hint) = sandbox_root_cause_hint("Unix socket bind") {
                    message.push(' ');
                    message.push_str(&hint);
                }
                message
            })?;

            info!("🔌 Agent socket bound: {}", socket_path.display());
            Some(listener)
        } else {
            info!(
                "🌐 Agent {} uses {} transport without local socket bind",
                config.b00t.name, config.b00t.agent.ipc.protocol
            );
            None
        };

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
            _listener: listener,
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
            if is_agent_config_file(&path) {
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
        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();
        let config = AgentManager::load_config(&config_path).await.unwrap();
        let exec = config.b00t.agent.executor.unwrap();
        assert_eq!(exec.cli_path, "pi");
        assert_eq!(exec.max_iterations, 10);
        assert!(exec.supports_tools.contains(&"bash".to_string()));
    }

    #[test]
    fn test_agent_config_file_accepts_cardinal_subtypes() {
        assert!(is_agent_config_file(Path::new("claude.agent.cli.toml")));
        assert!(is_agent_config_file(Path::new("maf.agent.sdk.tomllm")));
        assert!(is_agent_config_file(Path::new(
            "copilot.agent.ide.vsix.tomllmd"
        )));
        assert!(!is_agent_config_file(Path::new("claude.cli.toml")));
    }

    #[tokio::test]
    async fn test_load_config_with_empty_socket() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("opencode.agent.toml");
        let config_content = r#"
[b00t]
name = "opencode"
type = "agent"
hint = "opencode coding agent"

[b00t.agent]
pid = "opencode-001"
skills = ["code"]
role = "specialist"

[b00t.agent.ipc]
socket = ""
pubsub = false
protocol = "http+acp"

[b00t.agent.crew]
role = "specialist"
captain = false
"#;
        tokio::fs::write(&config_path, config_content)
            .await
            .unwrap();
        let config = AgentManager::load_config(&config_path).await.unwrap();
        assert_eq!(configured_socket_path(&config), None);
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
        assert!(
            result.contains("hello b00t"),
            "read tool must return file content: {}",
            result
        );
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
        assert!(
            result.contains("b00t hive") || result.contains("wrote"),
            "{}",
            result
        );
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

    // ── Pythonic call:name{...} format tests (gemma4/vLLM) ──────────────────

    #[test]
    fn test_parse_pi_output_pythonic_bash() {
        let raw = "call:bash{command:echo hello_world}";
        let calls = parse_pi_output(raw).expect("must parse pythonic bash call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "bash");
        assert_eq!(
            calls[0].args.get("command").and_then(|v| v.as_str()),
            Some("echo hello_world")
        );
    }

    #[test]
    fn test_parse_pi_output_pythonic_write_two_args() {
        // content value contains a comma — must not split on it
        let raw = "call:write{content:hello, world,path:/tmp/b00t_test.txt}";
        let calls = parse_pi_output(raw).expect("must parse pythonic write call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "write");
        assert_eq!(
            calls[0].args.get("content").and_then(|v| v.as_str()),
            Some("hello, world")
        );
        assert_eq!(
            calls[0].args.get("path").and_then(|v| v.as_str()),
            Some("/tmp/b00t_test.txt")
        );
    }

    #[test]
    fn test_parse_pi_output_pythonic_read() {
        let raw = "call:read{path:/tmp/b00t_test_pi.txt}";
        let calls = parse_pi_output(raw).expect("must parse pythonic read call");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read");
        assert_eq!(
            calls[0].args.get("path").and_then(|v| v.as_str()),
            Some("/tmp/b00t_test_pi.txt")
        );
    }

    #[test]
    fn test_parse_pi_output_pythonic_multiple_lines() {
        // pi may emit multiple calls as separate lines
        let raw = "call:bash{command:mkdir -p /tmp/b00t}\ncall:write{content:done,path:/tmp/b00t/out.txt}";
        let calls = parse_pi_output(raw).expect("must parse two pythonic calls");
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "bash");
        assert_eq!(calls[1].name, "write");
    }

    #[test]
    fn test_parse_pythonic_args_single() {
        let args = parse_pythonic_args("command:ls -F /tmp");
        assert_eq!(
            args.get("command").and_then(|v| v.as_str()),
            Some("ls -F /tmp")
        );
    }

    #[test]
    fn test_parse_pythonic_args_comma_in_value() {
        // content:hello, world,path:/foo — comma inside value must not split key
        let args = parse_pythonic_args("content:hello, world,path:/foo");
        assert_eq!(
            args.get("content").and_then(|v| v.as_str()),
            Some("hello, world")
        );
        assert_eq!(args.get("path").and_then(|v| v.as_str()), Some("/foo"));
    }

    #[test]
    fn test_agent_manager_creation() {
        let manager = AgentManager::default();
        assert_eq!(manager.redis_config.host, "localhost");
    }
}
