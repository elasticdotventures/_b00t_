//! Core chat message structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Canonical representation of a chat event exchanged between b00t agents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Logical channel identifier (team, mission, etc.).
    pub channel: String,
    /// Free-form sender descriptor (user, agent, subsystem).
    pub sender: String,
    /// Human readable payload body.
    pub body: String,
    /// Optional structured metadata attached to the message.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    /// UTC timestamp supplied by the origin transport.
    pub timestamp: DateTime<Utc>,
}

impl ChatMessage {
    /// Create a new chat message with the given parameters.
    pub fn new(
        channel: impl Into<String>,
        sender: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            channel: channel.into(),
            sender: sender.into(),
            body: body.into(),
            metadata: serde_json::Value::Null,
            timestamp: Utc::now(),
        }
    }

    /// Attach metadata to the message.
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }
}
