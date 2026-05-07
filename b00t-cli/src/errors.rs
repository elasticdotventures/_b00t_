//! Typed errors for b00t-cli commands — migrated from anyhow to thiserror.
//!
//! This module provides precise, matchable error types for each command domain.
//! Consumers can match on specific variants rather than string-comparing anyhow
//! messages.
//!
//! # Re-exports
//!
//! All governance error types are re-exported here so that b00t-cli consumers
//! can match on `b00t_cli::errors::GovernanceError` without importing the
//! governance crate directly.

use thiserror::Error;

// ---------------------------------------------------------------------------
// Crew command errors
// ---------------------------------------------------------------------------

/// Errors that can occur during crew operations (recruit, hire, roster).
#[derive(Error, Debug)]
pub enum CrewError {
    /// Agent not found in the store
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Agent store I/O error
    #[error("Agent store error: {0}")]
    StoreError(#[from] std::io::Error),

    /// JSON serialization / deserialization error
    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

/// Convenience alias for crew command results.
pub type CrewResult<T> = Result<T, CrewError>;

// ---------------------------------------------------------------------------
// Re-exports from governance crate
// ---------------------------------------------------------------------------

/// Governance error — re-exported from b00t-c0re-gov.
pub use b00t_c0re_gov::errors::GovernanceError;

/// Context store error — re-exported from b00t-c0re-gov.
pub use b00t_c0re_gov::errors::ContextStoreError;

/// Scheduler error — re-exported from b00t-c0re-gov.
pub use b00t_c0re_gov::errors::SchedulerError;

/// Convenience alias for governance results — re-exported.
pub use b00t_c0re_gov::errors::GovResult;

/// Convenience alias for context store results — re-exported.
pub use b00t_c0re_gov::errors::StoreResult;

/// Convenience alias for scheduler results — re-exported.
pub use b00t_c0re_gov::errors::SchedResult;
