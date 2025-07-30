//! # b00t Kubernetes Subsystem
//!
//! Agent-friendly Kubernetes orchestration using kube-rs ecosystem.
//! Extends the existing k8s datum with modern kube-rs client capabilities.

pub mod client;
pub mod config;
pub mod error;
pub mod resources;
pub mod translation;
pub mod utils;

// Re-export core types for convenient usage
pub use client::K8sClient;
pub use config::K8sConfig;
pub use error::{Error, Result};

// Re-export existing K8sDatum for compatibility
pub use crate::datum_k8s::K8sDatum;

/// Version information for the k8s subsystem
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default namespace for b00t operations
pub const DEFAULT_NAMESPACE: &str = "default";

/// Label prefix for b00t-managed resources
pub const LABEL_PREFIX: &str = "b00t.elastic.ventures";

/// Resource label for identifying b00t-managed k8s resources
pub const MANAGED_BY_LABEL: &str = "app.kubernetes.io/managed-by";
pub const MANAGED_BY_VALUE: &str = "b00t";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_NAMESPACE, "default");
        assert_eq!(LABEL_PREFIX, "b00t.elastic.ventures");
        assert_eq!(MANAGED_BY_LABEL, "app.kubernetes.io/managed-by");
        assert_eq!(MANAGED_BY_VALUE, "b00t");
    }
}
