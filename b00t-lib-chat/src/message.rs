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

/// ACP Task message — agent-to-agent work delegation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMessage {
    pub task_id: String,
    pub action: String,
    pub payload: serde_json::Value,
    pub from_agent: String,
    pub to_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DateTime<Utc>>,
    #[serde(default = "default_priority")]
    pub priority: String,
    pub timestamp: DateTime<Utc>,
}

fn default_priority() -> String { "normal".to_string() }

impl TaskMessage {
    pub fn new(
        action: impl Into<String>,
        from_agent: impl Into<String>,
        to_agent: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            task_id: uuid::Uuid::new_v4().to_string(),
            action: action.into(),
            payload,
            from_agent: from_agent.into(),
            to_agent: to_agent.into(),
            deadline: None,
            priority: "normal".to_string(),
            timestamp: Utc::now(),
        }
    }

    pub fn with_deadline(mut self, deadline: DateTime<Utc>) -> Self { self.deadline = Some(deadline); self }
    pub fn with_priority(mut self, p: impl Into<String>) -> Self { self.priority = p.into(); self }

    pub fn subject(&self) -> String { format!("b00t.tasks.{}", self.to_agent) }
    pub fn broadcast_subject() -> &'static str { "b00t.tasks.*" }
    pub fn agent_subject(agent_id: &str) -> String { format!("b00t.tasks.{}", agent_id) }
}
