use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// A2A Task — the fundamental unit of agent-to-agent work.
///
/// One agent sends a `Task` to another agent's skill, and the remote agent
/// processes it through a defined lifecycle returning results as `Artifact`s.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier for this task
    pub id: Uuid,

    /// The skill that this task targets
    pub skill_id: String,

    /// Input payload for the skill
    pub input: serde_json::Value,

    /// Current lifecycle state
    pub state: TaskState,

    /// Artifacts produced during execution
    pub artifacts: Vec<Artifact>,

    /// Message history for this task
    pub history: Vec<Message>,

    /// Task-level metadata
    pub metadata: TaskMetadata,
}

/// The lifecycle states an A2A Task can traverse.
///
/// The canonical lifecycle is:
/// `Submitted` → `Working` → `Completed` (success)
/// `Submitted` → `Working` → `Failed` (error)
/// `Submitted` → `Working` → `InputRequired` → `Working` → `Completed`
/// Any non-terminal state can transition to `Canceled`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskState {
    /// Task created, not yet picked up by the remote agent
    Submitted,

    /// Agent is actively processing the task
    Working,

    /// Agent needs more information from the sender to continue
    InputRequired,

    /// Task completed successfully — artifacts are available
    Completed,

    /// Task failed — error information in artifacts
    Failed,

    /// Task was canceled by the sender
    Canceled,
}

/// An artifact produced by a task (output, file, result, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Human-readable name
    pub name: String,

    /// MIME type of the content (e.g. `"text/plain"`, `"application/json"`)
    pub mime_type: String,

    /// Content payload — can be text, structured JSON, a file path, etc.
    pub content: serde_json::Value,

    /// Arbitrary metadata key-value pairs
    pub metadata: HashMap<String, String>,
}

/// A message in the task history, recording communication between parties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Who sent this message
    pub role: MessageRole,

    /// The message content
    pub content: String,

    /// When the message was sent
    pub timestamp: DateTime<Utc>,
}

/// Role of a message sender.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    /// The remote agent
    Agent,

    /// A human user
    Human,

    /// System-level message (e.g. state transitions)
    System,
}

/// Metadata attached to every task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMetadata {
    /// When the task was created
    pub created_at: DateTime<Utc>,

    /// When the task was last updated
    pub updated_at: DateTime<Utc>,

    /// Identifier for the sender (agent or user)
    pub sender: String,

    /// Priority level (0–255, higher = more urgent)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<u8>,

    /// Time-to-live in seconds; the task is abandoned after this duration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

// ---------------------------------------------------------------------------
// Constructors and helpers
// ---------------------------------------------------------------------------

impl Task {
    /// Create a new `Task` in `Submitted` state.
    pub fn new(skill_id: &str, input: serde_json::Value, sender: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            skill_id: skill_id.to_string(),
            input,
            state: TaskState::Submitted,
            artifacts: Vec::new(),
            history: Vec::new(),
            metadata: TaskMetadata {
                created_at: now,
                updated_at: now,
                sender: sender.to_string(),
                priority: None,
                ttl_seconds: None,
            },
        }
    }

    /// Transition this task to a new state, updating the timestamp and
    /// recording a system message for the transition.
    pub fn transition_to(&mut self, new_state: TaskState) {
        let old_state = format!("{:?}", self.state);
        self.state = new_state;
        self.metadata.updated_at = Utc::now();
        self.history.push(Message {
            role: MessageRole::System,
            content: format!("State changed: {} → {:?}", old_state, self.state),
            timestamp: Utc::now(),
        });
    }

    /// Add an artifact to this task and record a system message.
    pub fn add_artifact(&mut self, artifact: Artifact) {
        self.metadata.updated_at = Utc::now();
        self.history.push(Message {
            role: MessageRole::System,
            content: format!("Artifact added: {}", artifact.name),
            timestamp: Utc::now(),
        });
        self.artifacts.push(artifact);
    }

    /// Add a human or agent message to the history.
    pub fn add_message(&mut self, role: MessageRole, content: &str) {
        self.metadata.updated_at = Utc::now();
        self.history.push(Message {
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
        });
    }

    /// Check if the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            TaskState::Completed | TaskState::Failed | TaskState::Canceled
        )
    }

    /// Set the task priority.
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.metadata.priority = Some(priority);
        self
    }

    /// Set the task TTL.
    pub fn with_ttl(mut self, ttl_seconds: u64) -> Self {
        self.metadata.ttl_seconds = Some(ttl_seconds);
        self
    }
}

impl TaskState {
    /// Returns `true` if this state is terminal (task cannot progress further).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskState::Completed | TaskState::Failed | TaskState::Canceled
        )
    }
}

impl Artifact {
    /// Create a new text artifact.
    pub fn text(name: &str, content: &str) -> Self {
        Self {
            name: name.to_string(),
            mime_type: "text/plain".to_string(),
            content: serde_json::Value::String(content.to_string()),
            metadata: HashMap::new(),
        }
    }

    /// Create a new JSON artifact.
    pub fn json(name: &str, value: serde_json::Value) -> Self {
        Self {
            name: name.to_string(),
            mime_type: "application/json".to_string(),
            content: value,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to this artifact.
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

impl Message {
    /// Create a new message.
    pub fn new(role: MessageRole, content: &str) -> Self {
        Self {
            role,
            content: content.to_string(),
            timestamp: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("code-gen", serde_json::json!({"prompt": "Hello"}), "user-1");
        assert_eq!(task.skill_id, "code-gen");
        assert_eq!(task.state, TaskState::Submitted);
        assert!(!task.is_terminal());
        assert_eq!(task.metadata.sender, "user-1");
    }

    #[test]
    fn test_task_lifecycle_submitted_to_completed() {
        let mut task = Task::new("test-skill", serde_json::json!({"x": 1}), "sender");
        assert_eq!(task.state, TaskState::Submitted);

        task.transition_to(TaskState::Working);
        assert_eq!(task.state, TaskState::Working);

        task.add_artifact(Artifact::text("result", "done"));
        task.transition_to(TaskState::Completed);
        assert_eq!(task.state, TaskState::Completed);
        assert!(task.is_terminal());
        assert_eq!(task.artifacts.len(), 1);
        assert_eq!(task.history.len(), 3); // Submitted→Working, Artifact added, Working→Completed
    }

    #[test]
    fn test_task_lifecycle_input_required() {
        let mut task = Task::new("conv", serde_json::json!({"q": "hello"}), "user");
        task.transition_to(TaskState::Working);
        task.transition_to(TaskState::InputRequired);
        assert_eq!(task.state, TaskState::InputRequired);

        task.add_message(MessageRole::Human, "Here is the additional info");
        task.transition_to(TaskState::Working);
        task.transition_to(TaskState::Completed);
        assert!(task.is_terminal());
    }

    #[test]
    fn test_task_failure() {
        let mut task = Task::new("skill-1", serde_json::json!({"data": "bad"}), "tester");
        task.transition_to(TaskState::Working);
        task.add_artifact(Artifact::text("error", "Invalid input: expected number"));
        task.transition_to(TaskState::Failed);
        assert_eq!(task.state, TaskState::Failed);
        assert!(task.is_terminal());
    }

    #[test]
    fn test_task_canceled() {
        let mut task = Task::new("long-job", serde_json::json!({}), "user");
        task.transition_to(TaskState::Working);
        task.transition_to(TaskState::Canceled);
        assert_eq!(task.state, TaskState::Canceled);
        assert!(task.is_terminal());
    }

    #[test]
    fn test_priority_and_ttl() {
        let task = Task::new("urgent", serde_json::json!({}), "ops")
            .with_priority(200)
            .with_ttl(60);
        assert_eq!(task.metadata.priority, Some(200));
        assert_eq!(task.metadata.ttl_seconds, Some(60));
    }

    #[test]
    fn test_artifact_types() {
        let text_art = Artifact::text("readme", "Hello world");
        assert_eq!(text_art.mime_type, "text/plain");

        let json_art = Artifact::json("config", serde_json::json!({"key": "val"}));
        assert_eq!(json_art.mime_type, "application/json");
        assert_eq!(json_art.content["key"], "val");

        let meta_art = Artifact::text("log", "error").with_metadata("severity", "high");
        assert_eq!(meta_art.metadata.get("severity").unwrap(), "high");
    }

    #[test]
    fn test_serialization_roundtrip() {
        let task =
            Task::new("code-gen", serde_json::json!({"lang": "rust"}), "alice").with_priority(100);
        let json = serde_json::to_string_pretty(&task).unwrap();
        let deserialized: Task = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, task.id);
        assert_eq!(deserialized.skill_id, "code-gen");
        assert_eq!(deserialized.metadata.priority, Some(100));
        assert_eq!(deserialized.metadata.sender, "alice");
    }
}
