//! Codebase-memory MCP client — b00t integration shim
//!
//! Wraps the codebase-memory-mcp server (C binary) as a grok backend.
//! The MCP server is launched as a subprocess with stdio transport.
//!
//! 🦨 TODO: The Go binary requires GLIBC 2.38 (Ubuntu 22.04 has 2.35).
//! The pure C build (`make -f Makefile.cbm cbm`) must be compiled from source.
//! Until then, this module provides the interface with stubbed execution.
//!
//! # Integration with DualGrokClient
//! Add `CodebaseMemory` as a variant to `GrokBackend` in dual_grok.rs.
//! The client is initialized lazily — failure to start MCP is non-fatal (warning only).

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::{debug, error, info, warn};

// ── Result types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbmSearchResult {
    pub name: String,
    pub file_path: String,
    pub line: Option<usize>,
    pub signature: Option<String>,
    pub content_snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbmSearchResponse {
    pub results: Vec<CbmSearchResult>,
    pub total: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbmIngestResult {
    pub project: String,
    pub files_indexed: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbmClientInfo {
    /// Path to the codebase-memory-mcp binary
    pub binary_path: String,
    /// Working directory for project indexing (default: current dir)
    pub workdir: PathBuf,
    /// Whether the MCP subprocess is currently running
    pub is_running: bool,
}

// ── MCP subprocess ────────────────────────────────────────────────────────────

/// JSON-RPC request to codebase-memory-mcp
struct McpRequest {
    id: i64,
    method: String,
    params: serde_json::Value,
}

impl McpRequest {
    fn new(id: i64, method: &str, params: serde_json::Value) -> Self {
        Self { id, method: method.to_string(), params }
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.id,
            "method": self.method,
            "params": self.params
        }))?)
    }
}

// ── Client ────────────────────────────────────────────────────────────────────

/// codebase-memory-mcp subprocess wrapper
///
/// Manages the lifecycle of an MCP server subprocess with stdin/stdout transport.
/// The subprocess is started lazily on first tool call and persists across calls.
pub struct CodebaseMemoryClient {
    workdir: PathBuf,
    binary_path: PathBuf,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    request_id: i64,
}

impl CodebaseMemoryClient {
    /// Create a new client for the given project directory.
    ///
    /// `binary_path` should point to the compiled codebase-memory-mcp C binary.
    pub fn new<P: AsRef<std::path::Path>>(workdir: P, binary_path: Option<PathBuf>) -> Self {
        let default_binary = PathBuf::from("/usr/local/bin/codebase-memory-mcp");
        Self {
            workdir: workdir.as_ref().to_path_buf(),
            binary_path: binary_path.unwrap_or(default_binary),
            child: None,
            stdin: None,
            stdout: None,
            request_id: 0,
        }
    }

    /// Get client info without starting the subprocess
    pub fn info(&self) -> CbmClientInfo {
        CbmClientInfo {
            binary_path: self.binary_path.display().to_string(),
            workdir: self.workdir.clone(),
            is_running: self.child.is_some(),
        }
    }

    /// Check if the MCP subprocess is alive
    pub fn is_running(&self) -> bool {
        // 🤓 try_wait requires &mut — use is_some() as proxy since
        // child is only taken on start and None after drop
        self.child.is_some()
    }

    /// Start the MCP subprocess (lazily called before first tool invocation)
    pub async fn start(&mut self) -> Result<()> {
        if self.is_running() {
            debug!("🧠 codebase-memory-mcp already running");
            return Ok(());
        }

        if !self.binary_path.exists() {
            return Err(anyhow::anyhow!(
                "codebase-memory-mcp binary not found at {}. 🦨 Run: make -f Makefile.cbm cbm",
                self.binary_path.display()
            ));
        }

        debug!(
            "🧠 Starting codebase-memory-mcp: {} (workdir: {})",
            self.binary_path.display(),
            self.workdir.display()
        );

        let mut cmd = Command::new(&self.binary_path)
            .current_dir(&self.workdir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn codebase-memory-mcp: {}", e))?;

        let stdin = cmd.stdin.take().ok_or_else(|| anyhow::anyhow!("Failed to take stdin"))?;
        let stdout = cmd
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to take stdout"))?;

        // Log stderr in background
        if let Some(stderr) = cmd.stderr.take() {
            tokio::spawn(Self::log_stderr(stderr));
        }

        self.child = Some(cmd);
        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));

        // Send initialize handshake
        self.initialize().await?;

        info!("🧠 codebase-memory-mcp MCP server initialized");
        Ok(())
    }

    /// MCP initialize handshake
    async fn initialize(&mut self) -> Result<()> {
        let response = self
            .send_request("initialize", serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "b00t-hermes",
                    "version": "0.7.48"
                }
            }))
            .await?;

        debug!("🧠 MCP initialize response: {:?}", response);

        // Send initialized notification (fire and forget)
        let _ = self.send_notification("notifications/initialized", serde_json::json!({})).await;

        Ok(())
    }

    /// Search the codebase knowledge graph
    ///
    /// Returns matching functions, classes, routes by query pattern.
    pub async fn search(&mut self, query: &str, project: &str) -> Result<CbmSearchResponse> {
        self.ensure_started().await?;

        let result = self
            .send_request("tools/call", serde_json::json!({
                "name": "search_graph",
                "arguments": {
                    "query": query,
                    "project": project
                }
            }))
            .await?;

        // Parse the MCP tool result
        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("{}");

        let parsed: CbmSearchResponse = serde_json::from_str(content)
            .unwrap_or_else(|_| CbmSearchResponse { results: vec![], total: 0, truncated: false });

        Ok(parsed)
    }

    /// Index a project directory
    pub async fn index_project(&mut self, project_path: &str) -> Result<CbmIngestResult> {
        self.ensure_started().await?;

        let result = self
            .send_request("tools/call", serde_json::json!({
                "name": "index_project",
                "arguments": {
                    "path": project_path,
                    "language": "all"
                }
            }))
            .await?;

        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("{}");

        let parsed: CbmIngestResult = serde_json::from_str(content)
            .unwrap_or_else(|_| CbmIngestResult {
                project: project_path.to_string(),
                files_indexed: 0,
                duration_ms: 0,
            });

        Ok(parsed)
    }

    /// Get architecture overview of the project
    pub async fn architecture(&mut self, project: &str) -> Result<String> {
        self.ensure_started().await?;
        let result = self
            .send_request("tools/call", serde_json::json!({
                "name": "get_architecture",
                "arguments": {
                    "project": project
                }
            }))
            .await?;
        Ok(self.extract_tool_text(&result))
    }

    /// Trace callers/callees for a function
    pub async fn trace(&mut self, function: &str, project: &str, direction: &str) -> Result<String> {
        self.ensure_started().await?;
        let result = self
            .send_request("tools/call", serde_json::json!({
                "name": "trace_path",
                "arguments": {
                    "function_name": function,
                    "project": project,
                    "direction": direction
                }
            }))
            .await?;
        Ok(self.extract_tool_text(&result))
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    async fn ensure_started(&mut self) -> Result<()> {
        if !self.is_running() {
            self.start().await?;
        }
        Ok(())
    }

    async fn send_request(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        self.request_id += 1;
        let req = McpRequest::new(self.request_id, method, params);
        let json = req.to_json()?;

        if let Some(stdin) = &mut self.stdin {
            stdin
                .write_all(format!("{}\n", json).as_bytes())
                .await
                .map_err(|e| anyhow::anyhow!("Failed to write to MCP stdin: {}", e))?;
            stdin
                .flush()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to flush MCP stdin: {}", e))?;
        }

        // Read response line
        if let Some(stdout) = &mut self.stdout {
            let mut line = String::new();
            stdout
                .read_line(&mut line)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to read MCP stdout: {}", e))?;

            let response: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| anyhow::anyhow!("Failed to parse MCP response: {} | raw: {}", e, line))?;

            // Check for error in response
            if let Some(error) = response.get("error") {
                let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown");
                return Err(anyhow::anyhow!("MCP error [{}]: {}", code, message));
            }

            Ok(response.get("result").cloned().unwrap_or(serde_json::Value::Null))
        } else {
            Err(anyhow::anyhow!("MCP stdout not available"))
        }
    }

    async fn send_notification(&mut self, method: &str, params: serde_json::Value) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });
        let json = serde_json::to_string(&notification)?;

        if let Some(stdin) = &mut self.stdin {
            stdin.write_all(format!("{}\n", json).as_bytes()).await?;
            stdin.flush().await?;
        }

        Ok(())
    }

    async fn log_stderr(mut stderr: tokio::process::ChildStderr) {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        while let Ok(n) = reader.read_line(&mut line).await {
            if n == 0 { break; }
            debug!("🧠 codebase-memory-mcp stderr: {}", line.trim());
            line.clear();
        }
    }

    fn extract_tool_text(&self, result: &serde_json::Value) -> String {
        result
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string()
    }
}

impl Drop for CodebaseMemoryClient {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.start_kill();
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_info_without_start() {
        let client = CodebaseMemoryClient::new(".", None);
        let info = client.info();
        assert_eq!(info.workdir, PathBuf::from("."));
        assert!(info.binary_path.ends_with("codebase-memory-mcp"));
        assert!(!info.is_running);
    }

    #[test]
    fn test_custom_binary_path() {
        let custom = PathBuf::from("/opt/codebase-memory-mcp");
        let client = CodebaseMemoryClient::new(".", Some(custom.clone()));
        assert_eq!(client.info().binary_path, "/opt/codebase-memory-mcp");
    }

    #[test]
    fn test_mcp_request_serialization() {
        let req = McpRequest::new(1, "tools/call", serde_json::json!({"name": "search"}));
        let json = req.to_json().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["jsonrpc"], "2.0");
        assert_eq!(parsed["id"], 1);
        assert_eq!(parsed["method"], "tools/call");
        assert_eq!(parsed["params"]["name"], "search");
    }

    #[test]
    fn test_empty_result_parsing() {
        let result = serde_json::json!({
            "content": []
        });
        let client = CodebaseMemoryClient::new(".", None);
        let text = client.extract_tool_text(&result);
        assert_eq!(text, "");
    }

    #[test]
    fn test_result_text_extraction() {
        let result = serde_json::json!({
            "content": [
                { "type": "text", "text": "architectural overview" }
            ]
        });
        let client = CodebaseMemoryClient::new(".", None);
        let text = client.extract_tool_text(&result);
        assert_eq!(text, "architectural overview");
    }

    #[test]
    fn test_cbm_search_response_deserialization() {
        let json = serde_json::json!({
            "results": [
                {
                    "name": "handle_auth",
                    "file_path": "src/handlers/auth.rs",
                    "line": 42,
                    "signature": "pub async fn handle_auth(req: Request) -> Result<Response>",
                    "content_snippet": "pub async fn handle_auth..."
                }
            ],
            "total": 1,
            "truncated": false
        });
        let response: CbmSearchResponse = serde_json::from_value(json).unwrap();
        assert_eq!(response.total, 1);
        assert_eq!(response.results[0].name, "handle_auth");
        assert_eq!(response.results[0].line, Some(42));
    }

    #[test]
    fn test_cbm_ingest_result_deserialization() {
        let json = serde_json::json!({
            "project": "/home/brianh/.b00t",
            "files_indexed": 1449,
            "duration_ms": 175000
        });
        let result: CbmIngestResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.files_indexed, 1449);
        assert_eq!(result.duration_ms, 175000);
    }
}
