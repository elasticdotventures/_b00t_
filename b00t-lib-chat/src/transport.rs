//! Chat transport backends (local socket + NATS stub).

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::{fs, io::AsyncWriteExt, net::UnixStream, time::timeout};
use tracing::{debug, info, warn};

use crate::{
    error::{ChatError, ChatResult},
    message::ChatMessage,
};

/// Supported transport backends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatTransportKind {
    /// Local named pipe located at `~/.b00t/chat.channel.socket`
    LocalSocket,
    /// NATS message bus (stubbed)
    Nats,
}

impl ChatTransportKind {
    /// Parse from user provided string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "local" | "local-socket" | "socket" | "pipe" => Some(Self::LocalSocket),
            "nats" => Some(Self::Nats),
            _ => None,
        }
    }
}

/// Transport configuration.
#[derive(Debug, Clone)]
pub struct ChatTransportConfig {
    pub kind: ChatTransportKind,
    pub socket_path: Option<PathBuf>,
    pub nats_url: Option<String>,
}

impl Default for ChatTransportConfig {
    fn default() -> Self {
        Self {
            kind: ChatTransportKind::LocalSocket,
            socket_path: None,
            nats_url: None,
        }
    }
}

/// Unified transport wrapper.
#[derive(Debug, Clone)]
pub enum ChatTransport {
    Local(LocalSocketTransport),
    Nats(NatsTransport),
}

impl ChatTransport {
    /// Construct a new transport from config.
    pub fn from_config(config: ChatTransportConfig) -> ChatResult<Self> {
        match config.kind {
            ChatTransportKind::LocalSocket => {
                let transport = LocalSocketTransport::new(config.socket_path)?;
                Ok(Self::Local(transport))
            }
            ChatTransportKind::Nats => {
                let url = config
                    .nats_url
                    .unwrap_or_else(|| "nats://c010.promptexecution.com:4222".to_string());
                Ok(Self::Nats(NatsTransport { url }))
            }
        }
    }

    /// Send a message through the transport.
    pub async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        match self {
            ChatTransport::Local(transport) => transport.send(message).await,
            ChatTransport::Nats(transport) => transport.send(message).await,
        }
    }
}

/// Local socket transport implementation.
#[derive(Debug, Clone)]
pub struct LocalSocketTransport {
    socket_path: PathBuf,
}

impl LocalSocketTransport {
    pub fn new(path_override: Option<PathBuf>) -> ChatResult<Self> {
        let socket_path = if let Some(path) = path_override {
            path
        } else {
            default_socket_path()?
        };
        Ok(Self { socket_path })
    }

    async fn ensure_parent_dir(path: &Path) -> ChatResult<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).await?;
            }
        }
        Ok(())
    }

    pub async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        Self::ensure_parent_dir(&self.socket_path).await?;

        let payload = serde_json::to_vec(message)?;

        let connect_future = UnixStream::connect(&self.socket_path);
        let stream = match timeout(Duration::from_secs(1), connect_future).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                warn!("local chat socket unavailable: {}", e);
                return Err(ChatError::NotConnected);
            }
            Err(_) => {
                warn!("local chat socket connection timed out");
                return Err(ChatError::NotConnected);
            }
        };

        let mut stream = stream;
        stream.write_all(&payload).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;
        Ok(())
    }

    /// Expose the socket path used by the transport.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Stubbed NATS transport implementation.
#[derive(Debug, Clone)]
pub struct NatsTransport {
    url: String,
}

impl NatsTransport {
    async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        info!(
            "📡 NATS stub: would publish to {} (channel={}, sender={})",
            self.url, message.channel, message.sender
        );
        debug!("payload: {}", message.body);
        Ok(())
    }

    /// Accessor for subject prefix (mirrors legacy ACP subject structure).
    #[allow(dead_code)]
    pub fn subject_for(&self, channel: &str) -> String {
        format!("chat.{}", channel)
    }
}

/// Resolve the default socket path (`~/.b00t/chat.channel.socket`).
pub fn default_socket_path() -> ChatResult<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| ChatError::InvalidSocketPath("unable to resolve home directory".into()))?;
    Ok(home.join(".b00t/chat.channel.socket"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::ChatMessage;
    use std::path::PathBuf;

    /// Sending to a non-existent socket path returns ChatError::NotConnected, not a panic.
    #[tokio::test]
    async fn test_local_send_stale_socket_returns_not_connected() {
        let transport = LocalSocketTransport::new(Some(PathBuf::from("/tmp/b00t_no_such_socket_xyz.sock"))).unwrap();
        let msg = ChatMessage::new("test.channel", "tester", "hello");
        let result = transport.send(&msg).await;
        assert!(
            matches!(result, Err(ChatError::NotConnected) | Err(ChatError::Io(_))),
            "expected NotConnected or Io, got: {:?}",
            result
        );
    }

    /// Send to stale socket path that exists as a file (not a socket listener)
    /// also yields a connection error, not a hard panic.
    #[tokio::test]
    async fn test_local_send_existing_file_not_socket_returns_error() {
        let tmp = std::env::temp_dir().join("b00t_chat_test_not_a_socket.txt");
        std::fs::write(&tmp, b"not a socket").unwrap();
        let transport = LocalSocketTransport::new(Some(tmp.clone())).unwrap();
        let msg = ChatMessage::new("test.channel", "tester", "hello");
        let result = transport.send(&msg).await;
        assert!(result.is_err(), "expected error connecting to a regular file");
        let _ = std::fs::remove_file(tmp);
    }
}
