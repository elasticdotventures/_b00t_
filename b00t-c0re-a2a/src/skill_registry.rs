use std::collections::HashMap;
use std::sync::Arc;

use crate::agent_card::Skill;
use crate::error::{A2AError, A2AResult};
use crate::task::Task;

/// A registered skill handler — accepts a `Task` and returns a `Task` with
/// updated state and artifacts.
pub type SkillHandler = Arc<dyn Fn(Task) -> Result<Task, Box<dyn std::error::Error>> + Send + Sync>;

/// A skill along with its handler function.
struct RegisteredSkill {
    skill: Skill,
    handler: SkillHandler,
}

/// Registry of available skills on this agent.
///
/// The `SkillRegistry` maps skill IDs to handler functions. When a `Task`
/// arrives targeting a particular skill, the registry dispatches it to the
/// corresponding handler.
#[derive(Default)]
pub struct SkillRegistry {
    skills: HashMap<String, RegisteredSkill>,
}

impl SkillRegistry {
    /// Create a new, empty `SkillRegistry`.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Register a skill with its handler function.
    ///
    /// If a skill with the same ID already exists, it is overwritten.
    pub fn register(&mut self, skill: Skill, handler: SkillHandler) {
        self.skills.insert(
            skill.id.clone(),
            RegisteredSkill { skill, handler },
        );
    }

    /// Execute a task by dispatching it to the registered skill handler.
    ///
    /// Returns `A2AError::SkillNotFound` if the skill is not registered.
    /// The handler receives the task and should return the updated task.
    pub fn execute(&self, task: &Task) -> A2AResult<Task> {
        let registered = self
            .skills
            .get(&task.skill_id)
            .ok_or_else(|| A2AError::SkillNotFound(task.skill_id.clone()))?;

        let result = (registered.handler)(task.clone())
            .map_err(|e| A2AError::RuntimeError(format!("Handler error: {}", e)))?;

        Ok(result)
    }

    /// List all registered skills.
    pub fn list_skills(&self) -> Vec<Skill> {
        self.skills.values().map(|rs| rs.skill.clone()).collect()
    }

    /// Get a skill by its ID.
    pub fn get_skill(&self, id: &str) -> Option<&Skill> {
        self.skills.get(id).map(|rs| &rs.skill)
    }

    /// Check if a skill is registered.
    pub fn has_skill(&self, id: &str) -> bool {
        self.skills.contains_key(id)
    }

    /// Unregister a skill by ID.
    ///
    /// Returns `A2AError::SkillNotFound` if the skill doesn't exist.
    pub fn unregister(&mut self, id: &str) -> A2AResult<()> {
        if self.skills.remove(id).is_some() {
            Ok(())
        } else {
            Err(A2AError::SkillNotFound(id.to_string()))
        }
    }

    /// Number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Returns `true` if no skills are registered.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

// Manual Debug impl to avoid requiring Debug on the handler closure
impl std::fmt::Debug for SkillRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillRegistry")
            .field("skill_count", &self.skills.len())
            .field(
                "skill_ids",
                &self.skills.keys().cloned().collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{Task, TaskState};
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_register_and_list() {
        let mut registry = SkillRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);

        let handler: SkillHandler = Arc::new(|_task| Ok(Task::new("s1", serde_json::json!({}), "test")));
        registry.register(
            Skill::new("s1", "Greeter", "Greets the user", serde_json::json!({}), serde_json::json!({})),
            handler,
        );

        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        assert!(registry.has_skill("s1"));
        assert!(!registry.has_skill("s2"));

        let skills = registry.list_skills();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].id, "s1");
    }

    #[test]
    fn test_execute_skill() {
        let mut registry = SkillRegistry::new();

        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = Arc::clone(&call_count);

        let handler: SkillHandler = Arc::new(move |mut task| {
            count_clone.fetch_add(1, Ordering::SeqCst);
            task.transition_to(TaskState::Working);
            task.add_artifact(super::super::task::Artifact::text("result", "done"));
            task.transition_to(TaskState::Completed);
            Ok(task)
        });

        registry.register(
            Skill::new("calculator", "Calculator", "Does math", serde_json::json!({}), serde_json::json!({})),
            handler,
        );

        let task = Task::new("calculator", serde_json::json!({"a": 1, "b": 2}), "user");
        let result = registry.execute(&task).unwrap();

        assert_eq!(result.state, TaskState::Completed);
        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.artifacts[0].name, "result");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_execute_skill_not_found() {
        let registry = SkillRegistry::new();
        let task = Task::new("missing-skill", serde_json::json!({}), "user");
        let err = registry.execute(&task).unwrap_err();
        assert!(matches!(err, A2AError::SkillNotFound(_)));
    }

    #[test]
    fn test_get_skill() {
        let mut registry = SkillRegistry::new();
        let handler: SkillHandler = Arc::new(|t| Ok(t));
        registry.register(
            Skill::new("code-gen", "Code Gen", "Generates code", serde_json::json!({"type": "object"}), serde_json::json!({"type": "string"})),
            handler,
        );

        let skill = registry.get_skill("code-gen").unwrap();
        assert_eq!(skill.name, "Code Gen");
        assert_eq!(skill.input_schema["type"], "object");

        assert!(registry.get_skill("nonexistent").is_none());
    }

    #[test]
    fn test_unregister() {
        let mut registry = SkillRegistry::new();
        let handler: SkillHandler = Arc::new(|t| Ok(t));
        registry.register(
            Skill::new("temp", "Temp", "Temporary", serde_json::json!({}), serde_json::json!({})),
            handler,
        );
        assert!(registry.has_skill("temp"));

        registry.unregister("temp").unwrap();
        assert!(!registry.has_skill("temp"));

        let err = registry.unregister("temp").unwrap_err();
        assert!(matches!(err, A2AError::SkillNotFound(_)));
    }

    #[test]
    fn test_handler_error_propagation() {
        let mut registry = SkillRegistry::new();
        let handler: SkillHandler = Arc::new(|_task| {
            Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "handler failed")) as Box<dyn std::error::Error>)
        });
        registry.register(
            Skill::new("failing", "Failing", "Always fails", serde_json::json!({}), serde_json::json!({})),
            handler,
        );

        let task = Task::new("failing", serde_json::json!({}), "user");
        let err = registry.execute(&task).unwrap_err();
        assert!(err.to_string().contains("handler failed"));
    }

    #[test]
    fn test_multiple_skills() {
        let mut registry = SkillRegistry::new();
        registry.register(
            Skill::new("a", "A", "", serde_json::json!({}), serde_json::json!({})),
            Arc::new(|t| Ok(t)),
        );
        registry.register(
            Skill::new("b", "B", "", serde_json::json!({}), serde_json::json!({})),
            Arc::new(|t| Ok(t)),
        );

        assert_eq!(registry.len(), 2);
        let skills = registry.list_skills();
        let ids: Vec<&str> = skills.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
    }
}
