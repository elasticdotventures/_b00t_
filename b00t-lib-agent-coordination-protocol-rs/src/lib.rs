//! # B00T Agent Coordination Protocol (ACP) StepSync
//!
//! A lightweight peer-to-peer coordination protocol for turn-based agent collaboration.
//! Implements RFC-style ACP with Lamport timing and step barriers.
//!
//! ## Features
//!
//! - **Step-based coordination**: Discrete rounds with barrier synchronization
//! - **NATS transport**: Built on async-nats for reliable messaging
//! - **Role-based permissions**: Isolated namespaces per GitHub user
//! - **Language bindings**: Python and TypeScript/JavaScript support
//!
//! ## Message Types
//!
//! - `STATUS`: Convey current state or logs
//! - `PROPOSE`: Suggest actions or mutations  
//! - `STEP`: Mark completion of step for barrier sync
//!
//! ## Example
//!
//! ```rust
//! use b00t_acp::{Agent, AgentConfig, MessageType};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Set environment variables:
//!     // NATS_URL=nats://c010.promptexecution.com:4222
//!     // B00T_HIVE_JWT=eyJ0eXAi...
//!     
//!     let config = AgentConfig::from_env(
//!         "claude.124435".to_string(),
//!         "account.elasticdotventures".to_string()
//!     );
//!
//!     let mut agent = Agent::new(config).await?;
//!     
//!     // Send status message
//!     agent.send_status("Agent initialized", serde_json::json!({"ready": true})).await?;
//!     
//!     // Propose an action
//!     agent.send_propose("execute_command", serde_json::json!({
//!         "command": "git status",
//!         "working_dir": "/tmp"
//!     })).await?;
//!     
//!     // Complete step
//!     agent.complete_step().await?;
//!     
//!     Ok(())
//! }
//! ```

pub mod agent;
pub mod protocol;
pub mod transport;
pub mod error;
pub mod security;

#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use agent::{Agent, AgentConfig};
pub use protocol::{ACPMessage, MessageType, StepBarrier};
pub use transport::{NatsTransport, NatsConfig};
pub use error::{ACPError, Result};
pub use security::{
    AcpJwtValidator, AcpSecurityContext, NamespaceEnforcer, 
    SubjectOperation, fetch_jwt_from_website
};

// Re-export commonly used types
pub use serde_json::Value as JsonValue;
pub use uuid::Uuid;