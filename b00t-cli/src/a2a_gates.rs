//! Expose HiveGuard gates as A2A skills.
//! Each guard pattern becomes an A2A Skill in the SkillRegistry.

use std::sync::Arc;

use b00t_c0re_gov::gates::eisenhower::EisenhowerGate;
use b00t_c0re_gov::traits::*;
use b00t_c0re_a2a::agent_card::Skill;
use b00t_c0re_a2a::skill_registry::SkillRegistry;
use b00t_c0re_a2a::task::Task;

/// Register all governance gates as A2A skills.
/// Each gate becomes a callable skill that other agents can invoke.
pub fn register_governance_skills(registry: &mut SkillRegistry) {
    // ── Eisenhower gate as an A2A skill ────────────────────────────────────
    registry.register(
        Skill {
            id: "gov/eisenhower-check".into(),
            name: "Eisenhower Priority Check".into(),
            description: "Check a task's urgency and importance, returns Allow/Hook/Deny".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "urgency": {"type": "number", "description": "0.0-1.0 urgency"},
                    "importance": {"type": "number", "description": "0.0-1.0 importance"}
                },
                "required": ["urgency", "importance"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "decision": {"type": "string", "enum": ["allow", "hook", "deny"]},
                    "reason": {"type": "string"},
                    "quadrant": {"type": "string"}
                }
            }),
        },
        Arc::new(|task: Task| -> Result<Task, Box<dyn std::error::Error>> {
            let urgency = task.input["urgency"].as_f64().unwrap_or(0.5);
            let importance = task.input["importance"].as_f64().unwrap_or(0.5);

            let gate = EisenhowerGate::new("a2a-eisenhower");
            let context = GateCheckContext {
                agent_id: task.metadata.sender.clone(),
                task: task.id.to_string(),
                action: task.skill_id.clone(),
                metadata: serde_json::json!({
                    "urgency": urgency,
                    "importance": importance,
                }),
            };

            // EisenhowerGate::check is async; we block on it here since
            // the SkillRegistry handler signature is synchronous.
            let gate_result = tokio::runtime::Handle::current()
                .block_on(gate.check("a2a-eisenhower-check", &context));

            use b00t_c0re_gov::types::GateResult as GovGateResult;
            let (decision, reason, quadrant) = match &gate_result {
                GovGateResult::Allow => (
                    "allow",
                    "Task is urgent AND important — proceed immediately",
                    "Do",
                ),
                GovGateResult::Hook(token) => {
                    let q = if token.description.contains("Schedule") {
                        "Schedule"
                    } else {
                        "Delegate"
                    };
                    ("hook", token.description.as_str(), q)
                }
                GovGateResult::Deny { reason: r, .. } => ("deny", r.as_str(), "Eliminate"),
            };

            let output = serde_json::json!({
                "decision": decision,
                "reason": reason,
                "quadrant": quadrant,
            });

            let mut updated = task;
            updated.add_artifact(b00t_c0re_a2a::task::Artifact::json(
                "eisenhower-result",
                output,
            ));
            Ok(updated)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_eisenhower_skill() {
        let mut registry = SkillRegistry::new();
        register_governance_skills(&mut registry);

        assert!(registry.has_skill("gov/eisenhower-check"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_eisenhower_skill_allow() {
        let mut registry = SkillRegistry::new();
        register_governance_skills(&mut registry);

        let task = Task::new(
            "gov/eisenhower-check",
            serde_json::json!({
                "urgency": 0.9,
                "importance": 0.8,
            }),
            "test-agent",
        );

        let result = registry.execute(&task).unwrap();
        assert_eq!(result.state, b00t_c0re_a2a::task::TaskState::Submitted);
        assert_eq!(result.artifacts.len(), 1);

        let artifact = &result.artifacts[0];
        assert_eq!(artifact.name, "eisenhower-result");
        assert_eq!(artifact.content["decision"], "allow");
        assert_eq!(artifact.content["quadrant"], "Do");
    }

    #[test]
    fn test_eisenhower_skill_deny() {
        let mut registry = SkillRegistry::new();
        register_governance_skills(&mut registry);

        let task = Task::new(
            "gov/eisenhower-check",
            serde_json::json!({
                "urgency": 0.1,
                "importance": 0.1,
            }),
            "test-agent",
        );

        let result = registry.execute(&task).unwrap();
        let artifact = &result.artifacts[0];
        assert_eq!(artifact.content["decision"], "deny");
        assert_eq!(artifact.content["quadrant"], "Eliminate");
    }

    #[test]
    fn test_eisenhower_skill_hook_schedule() {
        let mut registry = SkillRegistry::new();
        register_governance_skills(&mut registry);

        let task = Task::new(
            "gov/eisenhower-check",
            serde_json::json!({
                "urgency": 0.1,
                "importance": 0.9,
            }),
            "test-agent",
        );

        let result = registry.execute(&task).unwrap();
        let artifact = &result.artifacts[0];
        assert_eq!(artifact.content["decision"], "hook");
        assert_eq!(artifact.content["quadrant"], "Schedule");
    }

    #[test]
    fn test_eisenhower_skill_hook_delegate() {
        let mut registry = SkillRegistry::new();
        register_governance_skills(&mut registry);

        let task = Task::new(
            "gov/eisenhower-check",
            serde_json::json!({
                "urgency": 0.9,
                "importance": 0.1,
            }),
            "test-agent",
        );

        let result = registry.execute(&task).unwrap();
        let artifact = &result.artifacts[0];
        assert_eq!(artifact.content["decision"], "hook");
        assert_eq!(artifact.content["quadrant"], "Delegate");
    }

    #[test]
    fn test_eisenhower_skill_defaults() {
        let mut registry = SkillRegistry::new();
        register_governance_skills(&mut registry);

        // Empty input: defaults to (0.5, 0.5) => Do quadrant => allow
        let task = Task::new(
            "gov/eisenhower-check",
            serde_json::json!({}),
            "test-agent",
        );

        let result = registry.execute(&task).unwrap();
        let artifact = &result.artifacts[0];
        assert_eq!(artifact.content["decision"], "allow");
    }
}
