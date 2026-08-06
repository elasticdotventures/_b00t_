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

/// ScopeStore errors — see scope_store.rs / _b00t_ issue #894.
///
/// Split into Transient (backend hiccup, safe to retry) vs. Structural
/// (the request itself is invalid, retrying won't help) via
/// [`ScopeError::is_transient`], so callers can implement retry/backoff
/// without matching on every variant.
#[derive(Error, Debug)]
pub enum ScopeError {
    #[error("scope backend unavailable: {0}")]
    BackendUnavailable(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("key not found in scope: {0}")]
    NotFound(String),

    #[error("write rejected: {0}")]
    WriteRejected(String),

    #[error("invalid scope id: {0}")]
    InvalidScopeId(String),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl ScopeError {
    /// True when the failure is a backend hiccup a caller may reasonably
    /// retry (unavailable backend, transient IO); false when the request
    /// itself is invalid and retrying it unchanged won't help.
    pub fn is_transient(&self) -> bool {
        matches!(self, ScopeError::BackendUnavailable(_) | ScopeError::Io(_))
    }
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

/// Convenience alias for scope store results.
pub type ScopeResult<T> = Result<T, ScopeError>;
