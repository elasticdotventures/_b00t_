//! Error types for the ACP library

use thiserror::Error;

/// ACP-specific error types
#[derive(Error, Debug)]
pub enum ACPError {
    /// NATS connection or transport error
    #[error("NATS error: {0}")]
    NatsError(#[from] async_nats::Error),

    /// Message serialization failed
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Message deserialization failed  
    #[error("Deserialization error: {0}")]
    DeserializationError(serde_json::Error),

    /// Connection timeout
    #[error("Connection timeout")]
    ConnectionTimeout,

    /// Request timeout
    #[error("Request timeout")]
    RequestTimeout,

    /// Receive timeout
    #[error("Receive timeout")]
    ReceiveTimeout,

    /// Step timeout (barrier synchronization)
    #[error("Step timeout - not all agents completed step {step} within {timeout_ms}ms")]
    StepTimeout { step: u64, timeout_ms: u64 },

    /// Invalid agent configuration
    #[error("Invalid agent configuration: {0}")]
    InvalidConfig(String),

    /// Agent not found in step barrier
    #[error("Agent '{agent_id}' not found in step barrier")]
    AgentNotFound { agent_id: String },

    /// Step barrier already contains agent
    #[error("Agent '{agent_id}' already exists in step barrier")]
    AgentAlreadyExists { agent_id: String },

    /// Invalid step number
    #[error("Invalid step number: expected {expected}, got {actual}")]
    InvalidStep { expected: u64, actual: u64 },

    /// Message validation failed
    #[error("Message validation failed: {reason}")]
    InvalidMessage { reason: String },

    /// Authentication failed
    #[error("Authentication failed: {reason}")]
    AuthenticationFailed { reason: String },

    /// Permission denied
    #[error("Permission denied: {reason}")]
    PermissionDenied { reason: String },

    /// Transport not connected
    #[error("Transport not connected")]
    NotConnected,

    /// Generic I/O error
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Generic error for unexpected conditions
    #[error("Unexpected error: {0}")]
    Other(String),
}

/// Convenience type alias
pub type Result<T> = std::result::Result<T, ACPError>;

impl ACPError {
    /// Create an invalid config error
    pub fn invalid_config(msg: impl Into<String>) -> Self {
        Self::InvalidConfig(msg.into())
    }

    /// Create an invalid message error
    pub fn invalid_message(reason: impl Into<String>) -> Self {
        Self::InvalidMessage { reason: reason.into() }
    }

    /// Create an authentication failed error
    pub fn auth_failed(reason: impl Into<String>) -> Self {
        Self::AuthenticationFailed { reason: reason.into() }
    }

    /// Create an authentication failed error (alias)
    pub fn authentication_failed(reason: impl Into<String>) -> Self {
        Self::AuthenticationFailed { reason: reason.into() }
    }

    /// Create a permission denied error
    pub fn permission_denied(reason: impl Into<String>) -> Self {
        Self::PermissionDenied { reason: reason.into() }
    }

    /// Create a generic other error
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }

    /// Check if error is recoverable (should retry)
    pub fn is_recoverable(&self) -> bool {
        matches!(self, 
            Self::ConnectionTimeout |
            Self::RequestTimeout | 
            Self::ReceiveTimeout |
            Self::NotConnected |
            Self::NatsError(_)
        )
    }

    /// Check if error is related to timing/synchronization
    pub fn is_timing_error(&self) -> bool {
        matches!(self,
            Self::ConnectionTimeout |
            Self::RequestTimeout |
            Self::ReceiveTimeout |
            Self::StepTimeout { .. }
        )
    }

    /// Check if error is related to authentication/authorization
    pub fn is_auth_error(&self) -> bool {
        matches!(self,
            Self::AuthenticationFailed { .. } |
            Self::PermissionDenied { .. }
        )
    }
}