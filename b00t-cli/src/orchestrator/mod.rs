// Orchestrator abstraction layer
// Provides orchestrator-agnostic deployment via adapters

pub mod adapter;
pub mod k8s_adapter;
pub mod detection;

pub use adapter::{OrchestratorAdapter, AdapterOutput, McpCommand, Orchestrator};
pub use k8s_adapter::K8sAdapter;
pub use detection::detect_orchestrator;
