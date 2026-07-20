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
pub mod dataframe_receipt;
pub mod discovery;
pub mod error;
pub mod flash_sheet;
pub mod gossip;
pub mod hive_transport;
pub mod ipc_transport;
pub mod ledgrrr;
pub mod mesh;
pub mod message;
pub mod metrics;
pub mod protocol;
pub mod router;
pub mod s5;
pub mod security;
pub mod server;
pub mod skill;
pub mod state_machine;
pub mod transport;
pub mod transports;
pub mod type_introspection;

pub use agent::{Agent, AgentConfig};
pub use assignment::{
    AssignmentEngine, AssignmentRule, Condition, ConditionOp, TaskTemplate, TimerSpec, TriggerKind,
};
pub use bridge::{McpBridge, McpServerSpec};
pub use client::ChatClient;
pub use dataframe_receipt::{
    ColumnValue, Dataframe, DataframeReceipt, Datatype, Field, FocusRecord,
};
pub use discovery::{SocketRegistry, SocketRegistryBuilder};
pub use error::{ChatError, ChatResult};
pub use flash_sheet::{
    CellAddress, CellChange, CellExpression, CellGuard, CellHook, FlashSheet, FlashSheetError,
    SheetCell, SheetColumn, SheetMetadata, SheetRow, SoulConcept, SoulKind, SymbolicRule,
    flash_sheet_type_descriptors,
};
pub use gossip::{GossipMember, GossipTable};
pub use ipc_transport::{
    AgentEndpoint, AgentEvent, AgentWatcher, BroadcastTransport, DirectTransport,
    DiscoverableTransport, IpcTransport, TransportKind,
};
pub use ledgrrr::{
    FinopsCode, Ledgrrr, LocalLedgrrr, McpLedgrrr, MockLedgrrr, ReceiptConstraint, UsageReceipt,
};
pub use mesh::{
    AgentPresence, DEFAULT_DISCOVER_TIMEOUT, DEFAULT_PRESENCE_INTERVAL, DEFAULT_PRESENCE_TTL,
    MeshFrame, MeshNodeConfig, NatsMeshNode,
};
pub use message::{ChatMessage, NotificationMessage, TaskMessage};
pub use metrics::{ChatMetrics, LatencyTimer};
pub use protocol::{ACPMessage, MessageType, StepBarrier};
pub use router::{Destination, MessageRouter, MessageRouterBuilder};
pub use s5::{S5Document, S5ParseError, parse_s5, render_s5};
pub use security::{
    AcpJwtValidator, AcpSecurityContext, NamespaceEnforcer, fetch_jwt_from_website,
};
pub use server::{ChatInbox, LocalChatServer, spawn_local_server};
pub use skill::{BootCommand, ModelAction, parse_b00t_command};
pub use state_machine::{
    ClifPayload, LogicalAddress, StateDispatchOutcome, StateMachineEvent, StateMachineGraphEdge,
    StateMachineGraphNode, StateMachineGraphSnapshot, StateMachineSpec, StateMachineVisualState,
    StateNode, StateTransformOutcome, StateTransition, state_machine_type_descriptors,
};
pub use transport::{ChatTransport, ChatTransportConfig, ChatTransportKind, default_socket_path};
pub use transports::{MqttTransport, NatsTransport};
pub use type_introspection::{
    FieldDescriptor, TypeDescriptor, TypeIntrospection, TypeMetadata, TypeShape, VariantDescriptor,
};

// Type aliases for compatibility
pub use ChatError as ACPError;
pub use ChatResult as Result;
pub type JsonValue = serde_json::Value;
