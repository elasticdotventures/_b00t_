use b00t_c0re_a2a::agent_card::{AgentCard, AuthenticationScheme, Skill};
use b00t_c0re_a2a::agent_store::AgentStore;
use b00t_c0re_a2a::error::A2AError;
use b00t_c0re_a2a::skill_registry::SkillRegistry;
use b00t_c0re_a2a::task::{Artifact, Task, TaskState};
use std::sync::Arc;
use url::Url;

// ---------------------------------------------------------------------------
// Agent Card integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_agent_card_creation_and_serialization() {
    let url = Url::parse("http://agent.example.com:8080").unwrap();
    let card = AgentCard::new("integration-agent", "A fully-featured agent", url.clone())
        .with_skill(Skill::new(
            "code-gen",
            "Code Generator",
            "Generates Rust code from natural language",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "language": {"type": "string"},
                    "prompt": {"type": "string"}
                }
            }),
            serde_json::json!({
                "type": "object",
                "properties": {
                    "code": {"type": "string"},
                    "explanation": {"type": "string"}
                }
            }),
        ))
        .with_skill(Skill::new(
            "search",
            "Web Search",
            "Searches the web for information",
            serde_json::json!({"type": "object"}),
            serde_json::json!({"type": "array"}),
        ))
        .with_default_skill("code-gen")
        .with_auth(AuthenticationScheme::bearer("test-token-123"))
        .with_auth(AuthenticationScheme::oauth2("https://auth.example.com/token"));

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&card).expect("serialization should succeed");
    assert!(json.contains("integration-agent"));
    assert!(json.contains("code-gen"));
    assert!(json.contains("bearer"));

    // Deserialize back
    let deserialized: AgentCard =
        serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(deserialized.name, "integration-agent");
    assert_eq!(deserialized.skills.len(), 2);
    assert_eq!(deserialized.default_skill.as_deref(), Some("code-gen"));
    assert_eq!(deserialized.authentication.len(), 2);

    // Verify skill lookup
    let code_skill = deserialized.find_skill("code-gen").unwrap();
    assert_eq!(code_skill.name, "Code Generator");
    assert_eq!(code_skill.input_schema["properties"]["language"]["type"].as_str().unwrap(), "string");
    assert_eq!(code_skill.input_schema["type"].as_str().unwrap(), "object");

    let search_skill = deserialized.find_skill("search").unwrap();
    assert_eq!(search_skill.name, "Web Search");

    // Nonexistent skill
    assert!(deserialized.find_skill("nonexistent").is_none());
}

// ---------------------------------------------------------------------------
// Task Lifecycle integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_task_full_lifecycle() {
    let mut task = Task::new("code-gen", serde_json::json!({"prompt": "write a hello world"}), "user-1");

    // Initial state
    assert_eq!(task.state, TaskState::Submitted);
    assert!(!task.is_terminal());
    assert_eq!(task.metadata.sender, "user-1");

    // Processing
    task.transition_to(TaskState::Working);
    assert_eq!(task.state, TaskState::Working);

    // Add output
    task.add_artifact(Artifact::json("output", serde_json::json!({
        "code": "fn main() { println!(\"Hello\"); }",
        "language": "rust"
    })));

    // Complete
    task.transition_to(TaskState::Completed);
    assert_eq!(task.state, TaskState::Completed);
    assert!(task.is_terminal());

    // Verify artifacts
    assert_eq!(task.artifacts.len(), 1);
    assert_eq!(task.artifacts[0].name, "output");
    assert_eq!(task.artifacts[0].mime_type, "application/json");

    // Verify history
    assert!(task.history.len() >= 3);
    assert!(task.history.iter().any(|m| m.content.contains("Submitted")));
    assert!(task.history.iter().any(|m| m.content.contains("Artifact added")));
    assert!(task.history.iter().any(|m| m.content.contains("Completed")));
}

#[test]
fn test_task_input_required_then_complete() {
    let mut task = Task::new("conversation", serde_json::json!({"query": "book a flight"}), "user");
    task.transition_to(TaskState::Working);
    task.transition_to(TaskState::InputRequired);
    assert_eq!(task.state, TaskState::InputRequired);

    // User provides input
    task.add_message(
        b00t_c0re_a2a::task::MessageRole::Human,
        "From NYC to London on Dec 25",
    );
    task.transition_to(TaskState::Working);
    task.add_artifact(Artifact::text("confirmation", "Flight booked!"));
    task.transition_to(TaskState::Completed);
    assert!(task.is_terminal());
}

#[test]
fn test_task_failure_with_artifacts() {
    let mut task = Task::new("file-processor", serde_json::json!({"path": "/invalid"}), "svc");
    task.transition_to(TaskState::Working);
    task.add_artifact(
        Artifact::text("error", "File not found: /invalid")
            .with_metadata("error_code", "404"),
    );
    task.transition_to(TaskState::Failed);
    assert!(task.is_terminal());

    assert_eq!(task.artifacts[0].name, "error");
    assert_eq!(
        task.artifacts[0].metadata.get("error_code").unwrap(),
        "404"
    );
}

#[test]
fn test_task_canceled() {
    let mut task = Task::new("long-job", serde_json::json!({"duration": 3600}), "ops");
    task.transition_to(TaskState::Working);
    assert!(!task.is_terminal());

    task.transition_to(TaskState::Canceled);
    assert!(task.is_terminal());
    assert_eq!(task.state, TaskState::Canceled);
}

#[test]
fn test_task_serialization_roundtrip() {
    let task = Task::new("test", serde_json::json!({"key": "value"}), "alice")
        .with_priority(50)
        .with_ttl(300);
    let json = serde_json::to_string(&task).unwrap();
    let deserialized: Task = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, task.id);
    assert_eq!(deserialized.metadata.priority, Some(50));
    assert_eq!(deserialized.metadata.ttl_seconds, Some(300));
}

// ---------------------------------------------------------------------------
// AgentStore integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_agent_store_lifecycle() {
    let tmpdir = std::env::temp_dir().join(format!(
        "b00t_a2a_int_test_{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&tmpdir).unwrap();
    let store = AgentStore::with_path(tmpdir.join("agents"));

    // Save multiple agents
    let url_a = Url::parse("http://agent-a.local").unwrap();
    let card_a = AgentCard::new("agent-a", "Agent A", url_a)
        .with_skill(Skill::new("ping", "Ping", "Ping test", serde_json::json!({}), serde_json::json!({})));
    store.save(&card_a).unwrap();

    let url_b = Url::parse("http://agent-b.local").unwrap();
    let card_b = AgentCard::new("agent-b", "Agent B", url_b)
        .with_skill(Skill::new("pong", "Pong", "Pong test", serde_json::json!({}), serde_json::json!({})));
    store.save(&card_b).unwrap();

    // List
    let cards = store.list().unwrap();
    assert_eq!(cards.len(), 2);

    // Load individually
    let loaded_a = store.load("agent-a").unwrap().expect("should exist");
    assert_eq!(loaded_a.description, "Agent A");
    assert_eq!(loaded_a.skills.len(), 1);

    // Search by skill
    let results = store.search_by_skill("Ping").unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "agent-a");

    // Count
    assert_eq!(store.count().unwrap(), 2);

    // Delete
    store.delete("agent-a").unwrap();
    assert_eq!(store.count().unwrap(), 1);
    assert!(store.load("agent-a").unwrap().is_none());

    // Delete nonexistent
    let err = store.delete("ghost").unwrap_err();
    assert!(matches!(err, A2AError::AgentNotFound(_)));

    // Cleanup
    let _ = std::fs::remove_dir_all(&tmpdir);
}

// ---------------------------------------------------------------------------
// SkillRegistry integration tests
// ---------------------------------------------------------------------------

#[test]
fn test_skill_registry_register_and_execute() {
    let mut registry = SkillRegistry::new();

    // Register a "greeter" skill
    registry.register(
        Skill::new(
            "greeter",
            "Greeter",
            "Greets the user by name",
            serde_json::json!({"type": "object", "properties": {"name": {"type": "string"}}}),
            serde_json::json!({"type": "object", "properties": {"greeting": {"type": "string"}}}),
        ),
        Arc::new(|mut task| {
            let name = task.input.get("name").and_then(|v| v.as_str()).unwrap_or("World");
            task.add_artifact(Artifact::text("greeting", &format!("Hello, {}!", name)));
            task.transition_to(TaskState::Completed);
            Ok(task)
        }),
    );

    let skill = registry.get_skill("greeter").unwrap();
    assert_eq!(skill.name, "Greeter");

    let task = Task::new("greeter", serde_json::json!({"name": "Alice"}), "user");
    let result = registry.execute(&task).unwrap();

    assert_eq!(result.state, TaskState::Completed);
    assert_eq!(result.artifacts.len(), 1);
    assert_eq!(result.artifacts[0].content, "Hello, Alice!");
}

#[test]
fn test_skill_registry_not_found() {
    let registry = SkillRegistry::new();
    let task = Task::new("unknown", serde_json::json!({}), "user");
    let err = registry.execute(&task).unwrap_err();
    assert!(matches!(err, A2AError::SkillNotFound(_)));
}

#[test]
fn test_multiple_skills_in_registry() {
    let mut registry = SkillRegistry::new();

    registry.register(
        Skill::new("translate", "Translator", "Translates text", serde_json::json!({}), serde_json::json!({})),
        Arc::new(|mut t| {
            t.transition_to(TaskState::Completed);
            Ok(t)
        }),
    );

    registry.register(
        Skill::new("summarize", "Summarizer", "Summarizes text", serde_json::json!({}), serde_json::json!({})),
        Arc::new(|mut t| {
            t.transition_to(TaskState::Completed);
            Ok(t)
        }),
    );

    assert_eq!(registry.len(), 2);
    assert!(registry.has_skill("translate"));
    assert!(registry.has_skill("summarize"));
    assert!(!registry.has_skill("nonexistent"));

    let task_t = Task::new("translate", serde_json::json!({"text": "hello"}), "user");
    let task_s = Task::new("summarize", serde_json::json!({"text": "long text"}), "user");

    assert!(registry.execute(&task_t).unwrap().is_terminal());
    assert!(registry.execute(&task_s).unwrap().is_terminal());
}

#[test]
fn test_skill_registry_unregister() {
    let mut registry = SkillRegistry::new();
    registry.register(
        Skill::new("temp", "Temp", "Temporary", serde_json::json!({}), serde_json::json!({})),
        Arc::new(|t| Ok(t)),
    );
    assert!(registry.has_skill("temp"));
    registry.unregister("temp").unwrap();
    assert!(!registry.has_skill("temp"));
}
