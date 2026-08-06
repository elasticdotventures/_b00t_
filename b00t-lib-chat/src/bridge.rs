use crate::{
    error::{ChatError, ChatResult},
    message::NotificationMessage,
    transport::ChatTransport,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone)]
pub struct McpServerSpec {
    pub id: String,
    pub label: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Option<HashMap<String, String>>,
    pub cwd: Option<String>,
}

#[derive(Debug)]
pub struct McpBridge {
    spec: McpServerSpec,
    child: Option<Child>,
}

#[derive(Debug, Deserialize)]
struct McpJsonRpcNotification {
    #[serde(default)]
    method: String,
    #[serde(default)]
    params: Option<serde_json::Value>,
}

impl McpBridge {
    pub fn new(spec: McpServerSpec) -> Self {
        Self { spec, child: None }
    }

    pub async fn start(&mut self, transport: &ChatTransport) -> ChatResult<()> {
        let mut cmd = Command::new(&self.spec.command);
        cmd.args(&self.spec.args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(cwd) = &self.spec.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(env) = &self.spec.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        cmd.kill_on_drop(false);

        let mut child = cmd.spawn().map_err(|e| {
            ChatError::Other(format!(
                "MCP bridge spawn failed ({}): {}",
                self.spec.command, e
            ))
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ChatError::Other("MCP bridge: no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ChatError::Other("MCP bridge: no stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ChatError::Other("MCP bridge: no stderr".into()))?;

        self.child = Some(child);

        let mut stdin_writer = stdin;
        let handshake = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "b00t-mcp-bridge", "version": "0.1.0" }
            }
        });

        let handshake_str = serde_json::to_string(&handshake).unwrap();
        stdin_writer
            .write_all(handshake_str.as_bytes())
            .await
            .map_err(|e| ChatError::Other(format!("MCP bridge: handshake write failed: {}", e)))?;
        stdin_writer.write_all(b"\n").await.map_err(|e| {
            ChatError::Other(format!("MCP bridge: handshake newline failed: {}", e))
        })?;
        stdin_writer
            .flush()
            .await
            .map_err(|e| ChatError::Other(format!("MCP bridge: handshake flush failed: {}", e)))?;

        let mut stdout_reader = BufReader::new(stdout);
        let mut response_line = String::new();
        stdout_reader
            .read_line(&mut response_line)
            .await
            .map_err(|e| ChatError::Other(format!("MCP bridge: handshake read failed: {}", e)))?;

        let init_response: serde_json::Value = serde_json::from_str(&response_line)
            .map_err(|e| ChatError::Other(format!("MCP bridge: bad handshake: {}", e)))?;

        if init_response.get("error").is_some() {
            return Err(ChatError::Other(format!(
                "MCP bridge: init failed for {}: {}",
                self.spec.id,
                response_line.trim()
            )));
        }

        let initialized = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        });
        let init_str = serde_json::to_string(&initialized).unwrap();
        stdin_writer
            .write_all(init_str.as_bytes())
            .await
            .map_err(|e| {
                ChatError::Other(format!("MCP bridge: initialized notify failed: {}", e))
            })?;
        stdin_writer.write_all(b"\n").await.map_err(|e| {
            ChatError::Other(format!("MCP bridge: initialized newline failed: {}", e))
        })?;
        stdin_writer.flush().await.map_err(|e| {
            ChatError::Other(format!("MCP bridge: initialized flush failed: {}", e))
        })?;

        info!("MCP bridge {} connected successfully", self.spec.id);

        let source = self.spec.label.clone();
        let spec_id = self.spec.id.clone();
        let transport_clone = transport.clone();

        let stderr_id = spec_id.clone();

        tokio::spawn(async move {
            Self::notification_loop(spec_id, source, stdout_reader, transport_clone).await;
        });

        tokio::spawn(async move {
            let mut stderr_reader = BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                match stderr_reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) if line.trim().is_empty() => continue,
                    Ok(_) => debug!("MCP bridge [{}] stderr: {}", stderr_id, line.trim()),
                    Err(_) => break,
                }
            }
        });

        info!("MCP bridge {} notification loop started", self.spec.id);
        Ok(())
    }

    async fn notification_loop(
        id: String,
        source: String,
        mut reader: BufReader<tokio::process::ChildStdout>,
        transport: ChatTransport,
    ) {
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    warn!("MCP bridge {} stdout closed", id);
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    match serde_json::from_str::<McpJsonRpcNotification>(trimmed) {
                        Ok(notif) => {
                            let method = notif.method;
                            if method.starts_with("notifications/") {
                                let event_type = method
                                    .strip_prefix("notifications/")
                                    .unwrap_or(&method)
                                    .replace('/', ".");
                                let payload = notif.params.unwrap_or(serde_json::Value::Null);
                                let notification =
                                    NotificationMessage::new(&source, &event_type, payload);
                                if let Err(e) = transport.publish_notification(&notification).await
                                {
                                    error!("MCP bridge {} publish failed: {}", id, e);
                                } else {
                                    debug!(
                                        "MCP bridge {} → b00t.notify.{}.{}",
                                        id, source, event_type
                                    );
                                }
                            } else if method.contains("error") {
                                warn!("MCP bridge {} received error: {}", id, trimmed);
                            }
                        }
                        Err(e) => {
                            warn!("MCP bridge {} bad JSON: {} ({})", id, e, trimmed);
                        }
                    }
                }
                Err(e) => {
                    error!("MCP bridge {} read error: {}", id, e);
                    break;
                }
            }
        }
        warn!("MCP bridge {} notification loop exited", id);
    }

    pub async fn stop(&mut self) {
        if let Some(ref mut child) = self.child {
            let _ = child.kill().await;
            let _ = child.wait().await;
            self.child = None;
            info!("MCP bridge {} stopped", self.spec.id);
        }
    }
}

#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[test]
    fn test_mcp_bridge_spec() {
        let spec = McpServerSpec {
            id: "test-gmail".to_string(),
            label: "gmail".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            env: None,
            cwd: None,
        };
        assert_eq!(spec.id, "test-gmail");
        assert_eq!(spec.label, "gmail");
    }

    #[test]
    fn test_notification_method_parsing() {
        let json = r#"{"jsonrpc":"2.0","method":"notifications/resources/updated","params":{"uri":"file:///test.txt"}}"#;
        let parsed: McpJsonRpcNotification = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.method, "notifications/resources/updated");
        assert!(parsed.params.is_some());
    }

    #[test]
    fn test_notification_event_type_convention() {
        let method = "notifications/resources/updated";
        let event_type = method
            .strip_prefix("notifications/")
            .unwrap()
            .replace('/', ".");
        assert_eq!(event_type, "resources.updated");

        let method2 = "notifications/tools/list_changed";
        let event_type2 = method2
            .strip_prefix("notifications/")
            .unwrap()
            .replace('/', ".");
        assert_eq!(event_type2, "tools.list_changed");
    }
}
