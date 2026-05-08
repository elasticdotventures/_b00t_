//! Governance error types — typed errors using thiserror.
//! Replaces catch-all anyhow with precise, matchable error variants.

use thiserror::Error;

/// Governance gate errors
#[derive(Error, Debug)]
pub enum GovernanceError {
    #[error("Gate not found: {0}")]
    GateNotFound(String),
    
    #[error("Hook expired: {0}")]
    HookExpired(String),
    
    #[error("Context corrupt: {0}")]
    ContextCorrupt(String),
    
    #[error("Insufficient calories: {available:.1} available, {required:.1} required")]
    InsufficientCalories { available: f64, required: f64 },

    #[error("Agent invocation failed: {0}")]
    InvocationFailed(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Context store errors
#[derive(Error, Debug)]
pub enum ContextStoreError {
    #[error("Corrupt context: {0}")]
    CorruptContext(String),
    
    #[error("Hook not found: {0}")]
    HookNotFound(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Scheduler errors
#[derive(Error, Debug)]
pub enum SchedulerError {
    #[error("Ring buffer full")]
    RingFull,
    
    #[error("Event channel closed")]
    EventChannelClosed,
    
    #[error("Governance error: {0}")]
    Governance(#[from] GovernanceError),
}

/// Convenience alias for governance results.
pub type GovResult<T> = Result<T, GovernanceError>;

/// Convenience alias for context store results.
pub type StoreResult<T> = Result<T, ContextStoreError>;

/// Convenience alias for scheduler results.
pub type SchedResult<T> = Result<T, SchedulerError>;
