// Orchestrator abstraction layer
// Provides orchestrator-agnostic deployment via adapters

pub mod adapter;
pub mod detection;
pub mod k8s_adapter;
pub mod agent_messaging;

pub use adapter::{AdapterOutput, McpCommand, Orchestrator, OrchestratorAdapter};
pub use detection::detect_orchestrator;
pub use k8s_adapter::K8sAdapter;
pub use agent_messaging::{AgentMessage, MessageRouter, DeliveryResult};
