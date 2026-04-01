// Orchestrator abstraction layer
// Provides orchestrator-agnostic deployment via adapters

pub mod adapter;
pub mod detection;
pub mod k8s_adapter;

pub use adapter::{AdapterOutput, McpCommand, Orchestrator, OrchestratorAdapter};
pub use detection::detect_orchestrator;
pub use k8s_adapter::K8sAdapter;
