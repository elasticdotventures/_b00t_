//! Error definitions for the b00t chat transport layer.

use thiserror::Error;

/// Unified error type for chat transports.
#[derive(Debug, Error)]
pub enum ChatError {
    /// JSON serialization or deserialization failure.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Underlying I/O failure (filesystem, socket, etc.).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Attempted to talk to a transport that is not connected.
    #[error("chat transport not connected")]
    NotConnected,

    /// Named pipe / socket path could not be resolved.
    #[error("invalid socket path: {0}")]
    InvalidSocketPath(String),

    /// Stubbed transports that are not fully implemented yet.
    #[error("transport not implemented: {0}")]
    NotImplemented(&'static str),

    /// Generic error for miscellaneous cases.
    #[error("{0}")]
    Other(String),
}

/// Convenience result type used across the chat crate.
pub type ChatResult<T> = std::result::Result<T, ChatError>;

impl ChatError {
    /// Build an [`ChatError::Other`] from any displayable value.
    pub fn other(msg: impl Into<String>) -> Self {
        ChatError::Other(msg.into())
    }
}
