//! High level chat client helper used by b00t-cli.

use crate::{
    error::{ChatError, ChatResult},
    message::{ChatMessage, NotificationMessage, TaskMessage},
    transport::{ChatTransport, ChatTransportConfig, ChatTransportKind},
};

/// Thin async client wrapper around the underlying transport.
#[derive(Debug, Clone)]
pub struct ChatClient {
    transport: ChatTransport,
}

impl ChatClient {
    /// Build a client for the requested transport.
    pub fn new(config: ChatTransportConfig) -> ChatResult<Self> {
        Ok(Self {
            transport: ChatTransport::from_config(config)?,
        })
    }

    /// Convenience helper for the default local transport.
    pub fn local_default() -> ChatResult<Self> {
        Self::new(ChatTransportConfig {
            kind: ChatTransportKind::LocalSocket,
            socket_path: None,
            nats_url: None,
            nats_user: None,
            nats_password: None,
        })
    }

    /// NATS transport with credentials (falls back to env NATS_URL and default b00t/b00t-hive-lan).
    pub fn nats(
        url: Option<String>,
        user: Option<String>,
        password: Option<String>,
    ) -> ChatResult<Self> {
        let url = url
            .or_else(|| std::env::var("NATS_URL").ok())
            .unwrap_or_else(|| "nats://localhost:4222".to_string());
        Self::new(ChatTransportConfig {
            kind: ChatTransportKind::Nats,
            socket_path: None,
            nats_url: Some(url),
            nats_user: user.or_else(|| Some("b00t".to_string())),
            nats_password: password.or_else(|| Some("b00t-hive-lan".to_string())),
        })
    }

    /// Send a message asynchronously.
    pub async fn send(&self, message: &ChatMessage) -> ChatResult<()> {
        self.transport.send(message).await
    }

    /// Helper that builds message + sends it.
    pub async fn send_text(
        &self,
        channel: impl Into<String>,
        sender: impl Into<String>,
        body: impl Into<String>,
    ) -> ChatResult<()> {
        let msg = ChatMessage::new(channel, sender, body);
        self.send(&msg).await
    }

    /// Send a task to another agent (NATS transport only).
    pub async fn send_task(&self, task: &TaskMessage) -> ChatResult<()> {
        self.transport.send_task(task).await
    }

    /// Subscribe to tasks for this agent. Returns a receiver of TaskMessages.
    pub async fn subscribe_tasks(
        &self,
        agent_id: &str,
    ) -> ChatResult<tokio::sync::mpsc::UnboundedReceiver<TaskMessage>> {
        self.transport.subscribe_tasks(agent_id).await
    }

    /// Publish a notification to NATS (e.g., MCP server event).
    pub async fn publish_notification(&self, notification: &NotificationMessage) -> ChatResult<()> {
        self.transport.publish_notification(notification).await
    }

    /// Subscribe to notifications matching a NATS subject wildcard (e.g., "b00t.notify.>").
    pub async fn subscribe_notifications(
        &self,
        wildcard: &str,
    ) -> ChatResult<tokio::sync::mpsc::UnboundedReceiver<NotificationMessage>> {
        self.transport.subscribe_notifications(wildcard).await
    }

    /// Return the transport identifier for telemetry.
    pub fn transport_kind(&self) -> &'static str {
        match &self.transport {
            ChatTransport::Local(_) => "local",
            ChatTransport::Nats(_) => "nats",
        }
    }

    pub fn transport(&self) -> &ChatTransport {
        &self.transport
    }
}

impl From<ChatTransportKind> for ChatTransportConfig {
    fn from(kind: ChatTransportKind) -> Self {
        ChatTransportConfig {
            kind,
            socket_path: None,
            nats_url: None,
            nats_user: None,
            nats_password: None,
        }
    }
}

impl TryFrom<ChatTransportKind> for ChatClient {
    type Error = ChatError;

    fn try_from(kind: ChatTransportKind) -> Result<Self, Self::Error> {
        Self::new(kind.into())
    }
}
