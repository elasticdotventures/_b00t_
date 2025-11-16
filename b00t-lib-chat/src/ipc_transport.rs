//! Polymorphic IPC transport abstraction for multi-channel agent coordination.
//!
//! This module defines the core traits that all IPC backends must implement,
//! enabling seamless switching between Unix sockets, Redis pub/sub, NATS,
//! and other messaging systems.

use crate::message::ChatMessage;
use crate::error::ChatResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

/// Core transport trait for sending/receiving messages.
///
/// All IPC backends (Unix sockets, Redis, NATS) implement this trait,
/// allowing polymorphic message routing.
#[async_trait]
pub trait IpcTransport: Send + Sync + Debug {
    /// Send a message through this transport.
    async fn send(&self, message: &ChatMessage) -> ChatResult<()>;

    /// Receive the next message (blocking).
    ///
    /// Returns `None` if the transport is closed or timed out.
    async fn recv(&self) -> ChatResult<Option<ChatMessage>>;

    /// Close the transport gracefully.
    async fn close(&self) -> ChatResult<()>;

    /// Get the transport kind for telemetry/debugging.
    fn kind(&self) -> TransportKind;

    /// Check if transport is currently connected/available.
    async fn is_available(&self) -> bool;
}

/// Broadcast-capable transport (pub/sub semantics).
#[async_trait]
pub trait BroadcastTransport: IpcTransport {
    /// Subscribe to a channel/topic.
    async fn subscribe(&mut self, channel: &str) -> ChatResult<()>;

    /// Unsubscribe from a channel/topic.
    async fn unsubscribe(&mut self, channel: &str) -> ChatResult<()>;

    /// Publish to a channel (broadcast to all subscribers).
    async fn publish(&self, channel: &str, message: &ChatMessage) -> ChatResult<()>;

    /// List all active subscriptions.
    fn subscriptions(&self) -> Vec<String>;
}

/// Point-to-point transport (direct agent-to-agent).
#[async_trait]
pub trait DirectTransport: IpcTransport {
    /// Connect to a specific agent's endpoint.
    async fn connect_to(&mut self, agent_id: &str) -> ChatResult<()>;

    /// Get the connected agent ID.
    fn connected_agent(&self) -> Option<String>;
}

/// Transport discovery trait for finding available agents.
#[async_trait]
pub trait DiscoverableTransport: Send + Sync {
    /// Discover all available agents.
    async fn discover_agents(&self) -> ChatResult<Vec<AgentEndpoint>>;

    /// Watch for new agents appearing (returns stream).
    async fn watch_agents(&self) -> ChatResult<AgentWatcher>;
}

/// Agent endpoint information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndpoint {
    pub agent_id: String,
    pub endpoint_uri: String,
    pub transport_kind: TransportKind,
    pub last_seen: std::time::SystemTime,
    pub metadata: Option<serde_json::Value>,
}

/// Agent watcher for monitoring agent availability.
pub struct AgentWatcher {
    inner: Box<dyn futures::Stream<Item = AgentEvent> + Send + Unpin>,
}

impl AgentWatcher {
    pub fn new<S>(stream: S) -> Self
    where
        S: futures::Stream<Item = AgentEvent> + Send + Unpin + 'static,
    {
        Self {
            inner: Box::new(stream),
        }
    }

    pub async fn next(&mut self) -> Option<AgentEvent> {
        use futures::StreamExt;
        self.inner.next().await
    }
}

/// Events from agent discovery.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Discovered(AgentEndpoint),
    Updated(AgentEndpoint),
    Lost(String), // agent_id
}

/// Transport type identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransportKind {
    UnixSocket,
    Redis,
    Nats,
    Tcp,
    Memory, // For testing
}

impl std::fmt::Display for TransportKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportKind::UnixSocket => write!(f, "unix"),
            TransportKind::Redis => write!(f, "redis"),
            TransportKind::Nats => write!(f, "nats"),
            TransportKind::Tcp => write!(f, "tcp"),
            TransportKind::Memory => write!(f, "memory"),
        }
    }
}

/// Macro for implementing basic IpcTransport boilerplate.
#[macro_export]
macro_rules! impl_ipc_transport_basics {
    ($type:ty, $kind:expr) => {
        fn kind(&self) -> $crate::ipc_transport::TransportKind {
            $kind
        }

        async fn close(&self) -> $crate::error::ChatResult<()> {
            // Default: no-op
            Ok(())
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_kind_display() {
        assert_eq!(TransportKind::UnixSocket.to_string(), "unix");
        assert_eq!(TransportKind::Redis.to_string(), "redis");
        assert_eq!(TransportKind::Nats.to_string(), "nats");
    }

    #[test]
    fn test_agent_endpoint_serialization() {
        let endpoint = AgentEndpoint {
            agent_id: "alpha".to_string(),
            endpoint_uri: "/tmp/b00t/agents/alpha.sock".to_string(),
            transport_kind: TransportKind::UnixSocket,
            last_seen: std::time::SystemTime::now(),
            metadata: Some(serde_json::json!({"role": "specialist"})),
        };

        let json = serde_json::to_string(&endpoint).unwrap();
        let deserialized: AgentEndpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(endpoint.agent_id, deserialized.agent_id);
    }
}
