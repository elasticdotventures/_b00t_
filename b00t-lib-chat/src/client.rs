//! High level chat client helper used by b00t-cli.

use crate::{
    error::{ChatError, ChatResult},
    message::ChatMessage,
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
        })
    }

    /// Convenience helper for the stubbed NATS transport.
    pub fn nats(url: Option<String>) -> ChatResult<Self> {
        Self::new(ChatTransportConfig {
            kind: ChatTransportKind::Nats,
            socket_path: None,
            nats_url: url,
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

    /// Return the transport identifier for telemetry.
    pub fn transport_kind(&self) -> &'static str {
        match &self.transport {
            ChatTransport::Local(_) => "local",
            ChatTransport::Nats(_) => "nats",
        }
    }
}

impl From<ChatTransportKind> for ChatTransportConfig {
    fn from(kind: ChatTransportKind) -> Self {
        ChatTransportConfig {
            kind,
            socket_path: None,
            nats_url: None,
        }
    }
}

impl TryFrom<ChatTransportKind> for ChatClient {
    type Error = ChatError;

    fn try_from(kind: ChatTransportKind) -> Result<Self, Self::Error> {
        Self::new(kind.into())
    }
}
