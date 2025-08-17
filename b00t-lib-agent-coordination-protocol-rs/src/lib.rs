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
//!     let config = AgentConfig {
//!         agent_id: "claude.124435".to_string(),
//!         nats_url: "nats://c010.promptexecution.com:4222".to_string(),
//!         namespace: "account.elasticdotventures".to_string(),
//!         jwt_token: Some("eyJ0eXAi...".to_string()),
//!         role: "ai-assistant".to_string(),
//!         timeout_ms: 30000,
//!     };
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

#[cfg(feature = "python")]
pub mod python;

#[cfg(feature = "wasm")]
pub mod wasm;

pub use agent::{Agent, AgentConfig};
pub use protocol::{ACPMessage, MessageType, StepBarrier};
pub use transport::{NatsTransport, NatsConfig};
pub use error::{ACPError, Result};

// Re-export commonly used types
pub use serde_json::Value as JsonValue;
pub use uuid::Uuid;