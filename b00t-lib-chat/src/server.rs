//! Local chat server + inbox queue management.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
};

use tokio::sync::Mutex;

use crate::{error::ChatResult, message::ChatMessage};

// 🤓 `tokio::net::UnixListener`/`UnixStream` are #[cfg(unix)]-gated upstream —
// this crate previously imported them unconditionally, which never actually
// compiled on Windows. Unlike agent_manager.rs's optional side-channel
// listener, this local chat server is a real accept loop with real socket
// I/O, so there's no meaningful unit-type placeholder to hand back on
// non-unix; the accept loop, connection handler, and bind itself are all
// gated `#[cfg(unix)]` and callers get a runtime error instead (see the
// `#[cfg(not(unix))]` twin of `spawn_local_server` below).
// `ChatTransportKind::Nats` remains fully functional on all platforms.
#[cfg(unix)]
use chrono::Utc;
#[cfg(unix)]
use tokio::io::{AsyncBufReadExt, BufReader};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
#[cfg(unix)]
use tracing::{debug, error, info};
#[cfg(unix)]
use crate::transport::default_socket_path;

/// Shared inbox that stores unread chat messages.
#[derive(Debug, Clone)]
pub struct ChatInbox {
    inner: Arc<Mutex<VecDeque<ChatMessage>>>,
}

impl ChatInbox {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub async fn push(&self, message: ChatMessage) {
        let mut guard = self.inner.lock().await;
        guard.push_back(message);
    }

    pub async fn unread_count(&self) -> usize {
        let guard = self.inner.lock().await;
        guard.len()
    }

    /// Remove and return all unread messages.
    pub async fn drain(&self) -> Vec<ChatMessage> {
        let mut guard = self.inner.lock().await;
        guard.drain(..).collect()
    }
}

/// Handle that keeps the local chat server alive.
pub struct LocalChatServer {
    socket_path: PathBuf,
}

impl LocalChatServer {
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Spawn the async listener for the default socket path.
#[cfg(unix)]
pub async fn spawn_local_server(inbox: ChatInbox) -> ChatResult<LocalChatServer> {
    let socket_path = default_socket_path()?;
    if socket_path.exists() {
        if let Err(e) = tokio::fs::remove_file(&socket_path).await {
            error!("failed to remove stale chat socket: {}", e);
        }
    }

    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let listener = UnixListener::bind(&socket_path)?;
    info!("🥾 local chat listener bound to {}", socket_path.display());

    tokio::spawn(run_accept_loop(listener, inbox));

    Ok(LocalChatServer { socket_path })
}

/// Non-unix twin: local Unix-socket chat transport has no equivalent on this
/// platform, so this returns a runtime error rather than binding anything.
/// Callers that need cross-platform chat should use `ChatTransportKind::Nats`.
#[cfg(not(unix))]
pub async fn spawn_local_server(_inbox: ChatInbox) -> ChatResult<LocalChatServer> {
    Err(crate::error::ChatError::Other(
        "local Unix-socket chat transport is not supported on this platform; use NATS transport instead"
            .to_string(),
    ))
}

#[cfg(unix)]
async fn run_accept_loop(listener: UnixListener, inbox: ChatInbox) {
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let inbox_clone = inbox.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, inbox_clone).await {
                        error!("chat socket connection error: {}", e);
                    }
                });
            }
            Err(err) => {
                error!("chat listener accept failure: {}", err);
                break;
            }
        }
    }
}

#[cfg(unix)]
async fn handle_connection(stream: UnixStream, inbox: ChatInbox) -> ChatResult<()> {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<ChatMessage>(&line) {
            Ok(mut message) => {
                // Ensure timestamps exist for older clients.
                if message.timestamp.timestamp() == 0 {
                    message.timestamp = Utc::now();
                }
                debug!(
                    "📥 received chat message (channel={}, sender={})",
                    message.channel, message.sender
                );
                inbox.push(message).await;
            }
            Err(err) => {
                error!("invalid chat payload: {}", err);
            }
        }
    }
    Ok(())
}
