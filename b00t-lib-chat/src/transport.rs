//! Chat transport backends — local Unix socket + real NATS via async-nats.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use async_nats::ConnectOptions;
use futures::StreamExt;
use tokio::{fs, io::AsyncWriteExt, net::UnixStream, time::timeout};
use tracing::{debug, info, warn};

use crate::{
    error::{ChatError, ChatResult},
    message::{ChatMessage, TaskMessage},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatTransportKind {
    LocalSocket,
    Nats,
}

impl ChatTransportKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "local" | "local-socket" | "socket" | "pipe" => Some(Self::LocalSocket),
            "nats" => Some(Self::Nats),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatTransportConfig {
    pub kind: ChatTransportKind,
    pub socket_path: Option<PathBuf>,
    pub nats_url: Option<String>,
    pub nats_user: Option<String>,
    pub nats_password: Option<String>,
}

impl Default for ChatTransportConfig {
    fn default() -> Self {
        Self { kind: ChatTransportKind::LocalSocket, socket_path: None, nats_url: None, nats_user: None, nats_password: None }
    }
}

impl ChatTransportConfig {
    pub fn nats(url: impl Into<String>, user: impl Into<String>, password: impl Into<String>) -> Self {
        Self { kind: ChatTransportKind::Nats, socket_path: None, nats_url: Some(url.into()), nats_user: Some(user.into()), nats_password: Some(password.into()) }
    }

    pub fn resolve_nats_url(&self) -> String {
        self.nats_url.clone().or_else(|| std::env::var("NATS_URL").ok()).unwrap_or_else(|| "nats://localhost:4222".to_string())
    }
}

#[derive(Debug, Clone)]
pub enum ChatTransport {
    Local(LocalSocketTransport),
    Nats(RealNatsTransport),
}

impl ChatTransport {
    pub fn from_config(config: ChatTransportConfig) -> ChatResult<Self> {
        match config.kind {
            ChatTransportKind::LocalSocket => Ok(Self::Local(LocalSocketTransport::new(config.socket_path)?)),
            ChatTransportKind::Nats => {
                let url = config.resolve_nats_url();
                // No hardcoded credential fallback — anonymous NATS connections are valid
                // (e.g. local dev servers with no auth configured). B00T_HIVE_NATS_USER/
                // B00T_HIVE_NATS_PASSWORD is the common hive-wide credential convention.
                let user = config.nats_user.or_else(|| std::env::var("B00T_HIVE_NATS_USER").ok());
                let password = config.nats_password.or_else(|| std::env::var("B00T_HIVE_NATS_PASSWORD").ok());
                Ok(Self::Nats(RealNatsTransport::new(url, user, password)))
            }
        }
    }

    pub async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        match self { ChatTransport::Local(t) => t.send(message).await, ChatTransport::Nats(t) => t.send(message).await }
    }

    pub async fn send_task(&self, task: &TaskMessage) -> ChatResult<()> {
        match self { ChatTransport::Nats(t) => t.send_task(task).await, ChatTransport::Local(_) => Err(ChatError::Other("task dispatch requires NATS transport".into())) }
    }

    pub async fn subscribe_tasks(&self, agent_id: &str) -> ChatResult<tokio::sync::mpsc::UnboundedReceiver<TaskMessage>> {
        match self { ChatTransport::Nats(t) => t.subscribe_tasks(agent_id).await, ChatTransport::Local(_) => Err(ChatError::Other("task subscription requires NATS transport".into())) }
    }
}

#[derive(Debug, Clone)]
pub struct LocalSocketTransport { socket_path: PathBuf }

impl LocalSocketTransport {
    pub fn new(path_override: Option<PathBuf>) -> ChatResult<Self> {
        let socket_path = if let Some(path) = path_override { path } else { default_socket_path()? };
        Ok(Self { socket_path })
    }

    async fn ensure_parent_dir(path: &Path) -> ChatResult<()> {
        if let Some(parent) = path.parent() { if !parent.exists() { fs::create_dir_all(parent).await?; } }
        Ok(())
    }

    pub async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        Self::ensure_parent_dir(&self.socket_path).await?;
        let payload = serde_json::to_vec(message)?;
        let connect_future = UnixStream::connect(&self.socket_path);
        let stream = match timeout(Duration::from_secs(1), connect_future).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => { warn!("local chat socket unavailable: {}", e); return Err(ChatError::NotConnected); }
            Err(_) => { warn!("local chat socket connection timed out"); return Err(ChatError::NotConnected); }
        };
        let mut stream = stream;
        stream.write_all(&payload).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;
        Ok(())
    }

    pub fn socket_path(&self) -> &Path { &self.socket_path }
}

#[derive(Debug, Clone)]
pub struct RealNatsTransport {
    url: String,
    user: Option<String>,
    password: Option<String>,
    client: std::sync::Arc<tokio::sync::RwLock<Option<async_nats::Client>>>,
}

impl RealNatsTransport {
    pub fn new(url: String, user: Option<String>, password: Option<String>) -> Self {
        Self { url, user, password, client: std::sync::Arc::new(tokio::sync::RwLock::new(None)) }
    }

    async fn ensure_connected(&self) -> ChatResult<async_nats::Client> {
        let mut guard = self.client.write().await;
        if let Some(ref client) = *guard { return Ok(client.clone()); }
        let opts = match (&self.user, &self.password) {
            (Some(u), Some(p)) => ConnectOptions::new().user_and_password(u.clone(), p.clone()),
            _ => ConnectOptions::new(),
        };
        let client = opts.connect(&self.url).await.map_err(|e| ChatError::Other(format!("NATS connect failed ({}): {}", self.url, e)))?;
        info!("NATS connected to {} as {}", self.url, self.user.as_deref().unwrap_or("anonymous"));
        *guard = Some(client.clone());
        Ok(client)
    }

    fn chat_subject(msg: &ChatMessage) -> String { format!("b00t.chat.{}.{}", msg.channel, msg.sender) }

    async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        let client = self.ensure_connected().await?;
        let subject = Self::chat_subject(message);
        let payload = serde_json::to_vec(message)?;
        client.publish(subject.clone(), payload.into()).await.map_err(|e| ChatError::Other(format!("NATS publish failed: {}", e)))?;
        debug!("NATS published to {}", subject);
        Ok(())
    }

    async fn send_task(&self, task: &TaskMessage) -> ChatResult<()> {
        let client = self.ensure_connected().await?;
        let subject = task.subject();
        let payload = serde_json::to_vec(task)?;
        client.publish(subject.clone(), payload.into()).await.map_err(|e| ChatError::Other(format!("NATS task publish failed: {}", e)))?;
        info!("Task {} dispatched to {} via NATS", task.task_id, task.to_agent);
        Ok(())
    }

    async fn subscribe_tasks(&self, agent_id: &str) -> ChatResult<tokio::sync::mpsc::UnboundedReceiver<TaskMessage>> {
        let client = self.ensure_connected().await?;
        let subject = TaskMessage::agent_subject(agent_id);
        let mut subscriber = client.subscribe(subject.clone()).await.map_err(|e| ChatError::Other(format!("NATS task subscribe failed: {}", e)))?;
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                match serde_json::from_slice::<TaskMessage>(&msg.payload) {
                    Ok(task) => { if tx.send(task).is_err() { break; } }
                    Err(e) => { warn!("NATS task deserialize failed: {}", e); }
                }
            }
        });
        info!("Subscribed to NATS tasks on {}", subject);
        Ok(rx)
    }
}

pub fn default_socket_path() -> ChatResult<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| ChatError::InvalidSocketPath("unable to resolve home directory".into()))?;
    Ok(home.join(".b00t/chat.channel.socket"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_nats_url_defaults() { let cfg = ChatTransportConfig::default(); assert!(cfg.resolve_nats_url().starts_with("nats://")); }

    #[test]
    fn test_task_subject_routing() {
        let task = TaskMessage::new("deploy", "orchestrator", "worker-7", serde_json::json!({"v": "v2"}));
        assert_eq!(task.subject(), "b00t.tasks.worker-7");
        assert_eq!(TaskMessage::agent_subject("worker-7"), "b00t.tasks.worker-7");
        assert_eq!(TaskMessage::broadcast_subject(), "b00t.tasks.*");
    }

    #[tokio::test]
    async fn test_local_send_stale_socket_returns_not_connected() {
        let transport = LocalSocketTransport::new(Some(PathBuf::from("/tmp/b00t_no_such_socket_xyz.sock"))).unwrap();
        let msg = ChatMessage::new("test", "tester", "hello");
        assert!(matches!(transport.send(&msg).await, Err(ChatError::NotConnected) | Err(ChatError::Io(_))));
    }

    #[tokio::test]
    async fn test_local_send_existing_file_not_socket_returns_error() {
        let tmp = std::env::temp_dir().join("b00t_chat_test_not_a_socket.txt");
        std::fs::write(&tmp, b"not a socket").unwrap();
        let transport = LocalSocketTransport::new(Some(tmp.clone())).unwrap();
        let msg = ChatMessage::new("test", "tester", "hello");
        assert!(transport.send(&msg).await.is_err());
        let _ = std::fs::remove_file(tmp);
    }
}
