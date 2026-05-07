use thiserror::Error;

/// Unified error type for the b00t-c0re-a2a crate.
#[derive(Error, Debug)]
pub enum A2AError {
    /// Agent card not found by name
    #[error("Agent card not found: {0}")]
    AgentNotFound(String),

    /// Skill not found by ID
    #[error("Skill not found: {0}")]
    SkillNotFound(String),

    /// Task timed out
    #[error("Task {0} timed out")]
    TaskTimeout(String),

    /// I/O error from the store layer
    #[error("Store error: {0}")]
    StoreError(#[from] std::io::Error),

    /// JSON serialization / deserialization error
    #[error("Serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    /// Generic runtime error wrapping another error
    #[error("Runtime error: {0}")]
    RuntimeError(String),
}

/// Convenience alias for crate-level Results.
pub type A2AResult<T> = Result<T, A2AError>;

impl From<Box<dyn std::error::Error>> for A2AError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        A2AError::RuntimeError(err.to_string())
    }
}
