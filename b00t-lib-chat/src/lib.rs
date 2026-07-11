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

pub mod agent;
pub mod assignment;
pub mod bridge;
pub mod client;
pub mod discovery;
pub mod dataframe_receipt;
pub mod error;
pub mod gossip;
pub mod ipc_transport;
pub mod ledgrrr;
pub mod mesh;
pub mod message;
pub mod metrics;
pub mod protocol;
pub mod router;
pub mod security;
pub mod server;
pub mod skill;
pub mod transport;
pub mod transports;

pub use agent::{Agent, AgentConfig};
pub use assignment::{
    AssignmentEngine, AssignmentRule, Condition, ConditionOp, TaskTemplate, TimerSpec, TriggerKind,
};
pub use bridge::{McpBridge, McpServerSpec};
pub use client::ChatClient;
pub use discovery::{SocketRegistry, SocketRegistryBuilder};
pub use error::{ChatError, ChatResult};
pub use ipc_transport::{
    AgentEndpoint, AgentEvent, AgentWatcher, BroadcastTransport, DirectTransport,
    DiscoverableTransport, IpcTransport, TransportKind,
};
pub use message::{ChatMessage, NotificationMessage, TaskMessage};
pub use mesh::{
    AgentPresence, MeshFrame, MeshNodeConfig, NatsMeshNode, DEFAULT_DISCOVER_TIMEOUT,
    DEFAULT_PRESENCE_INTERVAL, DEFAULT_PRESENCE_TTL,
};
pub use ledgrrr::{FinopsCode, Ledgrrr, LocalLedgrrr, McpLedgrrr, MockLedgrrr, ReceiptConstraint, UsageReceipt};
pub use gossip::{GossipTable, GossipMember};
pub use dataframe_receipt::{
    ColumnValue, Dataframe, DataframeReceipt, Datatype, Field, FocusRecord,
};
pub use metrics::{ChatMetrics, LatencyTimer};
pub use protocol::{ACPMessage, MessageType, StepBarrier};
pub use router::{Destination, MessageRouter, MessageRouterBuilder};
pub use security::{
    fetch_jwt_from_website, AcpJwtValidator, AcpSecurityContext, NamespaceEnforcer,
};
pub use server::{spawn_local_server, ChatInbox, LocalChatServer};
pub use skill::{parse_b00t_command, BootCommand, ModelAction};
pub use transport::{default_socket_path, ChatTransport, ChatTransportConfig, ChatTransportKind};
pub use transports::{MqttTransport, NatsTransport};

// Type aliases for compatibility
pub use ChatError as ACPError;
pub use ChatResult as Result;
pub type JsonValue = serde_json::Value;
