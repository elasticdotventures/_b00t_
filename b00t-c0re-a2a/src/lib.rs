//! # b00t-c0re-a2a — A2A (Agent-to-Agent) Protocol Rust SDK Core
//!
//! This crate implements the core data types for the **A2A v1.0** protocol:
//!
//! - **AgentCard** — Discovery document describing an agent's identity, skills,
//!   endpoint, and authentication requirements.
//! - **Task** — The fundamental unit of agent-to-agent work with a full
//!   lifecycle: `Submitted → Working → Completed | Failed | Canceled`.
//! - **Artifact / Message** — Output and communication types used in tasks.
//! - **AgentStore** — File-backed persistent storage for agent cards.
//! - **SkillRegistry** — Maps skill IDs to handler functions for dispatching
//!   incoming tasks.
//!
//! ## Quick Start
//!
//! ```rust
//! use b00t_c0re_a2a::agent_card::{AgentCard, Skill};
//! use b00t_c0re_a2a::task::Task;
//! use url::Url;
//!
//! let card = AgentCard::new(
//!     "my-agent",
//!     "An example A2A agent",
//!     Url::parse("stdio://my-agent").unwrap(),
//! );
//! let task = Task::new("example-skill", serde_json::json!({"prompt": "hi"}), "user");
//! ```

pub mod agent_card;
pub mod agent_store;
pub mod error;
pub mod heartbeat;
pub mod hive;
pub mod http_transport;
pub mod recruitment;
pub mod skill_registry;
pub mod task;
pub mod travel;

pub use agent_card::{AgentCard, AgentReputation, AuthenticationScheme, Skill};
pub use error::{A2AError, A2AResult};
pub use hive::HiveRegistry;
pub use http_transport::A2aHttpTransport;
pub use recruitment::{rank_agents, score_agent_for_skills};
pub use skill_registry::SkillRegistry;
pub use task::{Artifact, Message, MessageRole, Task, TaskMetadata, TaskState};
pub use travel::{TravelAgent, TravelManifest, TravelState};
