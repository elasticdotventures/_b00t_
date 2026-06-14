//! # b00t chat
//!
//! A lightweight coordination channel for PromptExecution agents. The chat
//! pipeline exposes a local Unix domain socket that agents can use to exchange
//! JSON messages while optionally mirroring the payloads to NATS (currently
//! implemented as telemetry stubs).
//!
//! ## Highlights
//!
//! - Local IPC **named pipe** at `~/.b00t/chat.channel.socket`
//! - Simple JSON [`ChatMessage`](crate::message::ChatMessage) envelope
//! - Async client for CLI usage via [`ChatClient`](crate::client::ChatClient)
//! - Inbox utilities that let MCP servers surface unread notifications
//!
//! ```no_run
//! use b00t_chat::{ChatClient, ChatMessage};
//!
//! # #[tokio::main]
//! # async fn main() -> anyhow::Result<()> {
//! let client = ChatClient::local_default()?;
//! let message = ChatMessage::new("mission.alpha", "frontend", "UI ready for review");
//! client.send(&message).await?;
//! # Ok(())
//! # }
//! ```

// pub mod agent;  // 🤓 agent.rs uses full NATS Agent from old ACP - chat refactor simplified to stubs
pub mod client;
pub mod discovery;
pub mod error;
pub mod ipc_transport;
pub mod message;
pub mod metrics;
pub mod protocol;
pub mod router;
pub mod security;
pub mod server;
pub mod skill;
pub mod transport;
pub mod transports;

// pub use agent::{Agent, AgentConfig};  // 🤓 Disabled - needs chat-compatible refactor
pub use client::ChatClient;
pub use discovery::{SocketRegistry, SocketRegistryBuilder};
pub use error::{ChatError, ChatResult};
pub use ipc_transport::{
    AgentEndpoint, AgentEvent, AgentWatcher, BroadcastTransport, DirectTransport,
    DiscoverableTransport, IpcTransport, TransportKind,
};
pub use message::ChatMessage;
pub use metrics::{ChatMetrics, LatencyTimer};
pub use protocol::{ACPMessage, MessageType, StepBarrier};
pub use router::{Destination, MessageRouter, MessageRouterBuilder};
pub use security::{
    AcpJwtValidator, AcpSecurityContext, NamespaceEnforcer, fetch_jwt_from_website,
};
pub use server::{ChatInbox, LocalChatServer, spawn_local_server};
pub use skill::{BootCommand, ModelAction, parse_b00t_command};
pub use transport::{ChatTransport, ChatTransportConfig, ChatTransportKind, default_socket_path};
pub use transports::{MqttTransport, NatsTransport};

// Type aliases for compatibility
pub use ChatError as ACPError;
pub use ChatResult as Result;
pub type JsonValue = serde_json::Value;
