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
pub mod error;
pub mod message;
pub mod protocol;
pub mod security;
pub mod server;
pub mod transport;

// pub use agent::{Agent, AgentConfig};  // 🤓 Disabled - needs chat-compatible refactor
pub use client::ChatClient;
pub use error::{ChatError, ChatResult};
pub use message::ChatMessage;
pub use protocol::{ACPMessage, MessageType, StepBarrier};
pub use security::{fetch_jwt_from_website, AcpJwtValidator, AcpSecurityContext, NamespaceEnforcer};
pub use server::{spawn_local_server, ChatInbox, LocalChatServer};
pub use transport::{default_socket_path, ChatTransport, ChatTransportConfig, ChatTransportKind};

// Type aliases for compatibility
pub use ChatError as ACPError;
pub use ChatResult as Result;
pub type JsonValue = serde_json::Value;
