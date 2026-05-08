use async_trait::async_trait;
use uuid::Uuid;

use crate::traits::*;
use crate::types::*;

/// A gate that returns a Hook with a cron expression.
/// The agent must wait until the cron schedule fires before the action is allowed.
/// Parses the cron expression from the `cron_expr` field in metadata.
pub struct CronGate {
    name: String,
    description: String,
}

impl CronGate {
    /// Create a gate that returns a Hook with a cron expression from metadata.
    pub fn new(name: &str, description: &str) -> Self {
        CronGate {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl GovernanceGate for CronGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, context: &GateCheckContext) -> GateResult {
        let cron_expr = context
            .metadata
            .get("cron_expr")
            .and_then(|v| v.as_str())
            .unwrap_or("* * * * *")
            .to_string();

        GateResult::Hook(HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Cron(cron_expr.clone()),
            created_at: chrono::Utc::now(),
            // ttl_ms: None — cron schedules are ongoing; a fixed TTL would cause
            // reap_expired_hooks() to discard long-lived schedules prematurely.
            // NOTE: EventScheduler currently skips HookType::Cron hooks
            // ("Cron not yet implemented — skip"); route to an external cron runner
            // (e.g. systemd timer, crond) to actually fire HookEvent::Fired.
            ttl_ms: None,
            description: format!(
                "Cron gate '{}': scheduled '{}' at cron '{}'",
                self.name, action, cron_expr
            ),
        })
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[tokio::test]
    async fn test_cron_gate_returns_hook_with_expression_from_metadata() {
        let gate = CronGate::new("daily-task", "Runs on a daily cron schedule");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({"cron_expr": "0 9 * * *"}),
        };

        let result = gate.check("do-something", &context).await;

        match result {
            GateResult::Hook(token) => {
                assert_eq!(token.hook_type, HookType::Cron("0 9 * * *".to_string()));
                assert!(token.ttl_ms.is_none(), "cron hooks must have no fixed TTL");
                assert!(token.description.contains("daily-task"));
                assert!(token.description.contains("0 9 * * *"));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_cron_gate_defaults_to_every_minute_when_no_metadata() {
        let gate = CronGate::new("fallback", "Uses default cron expression");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = gate.check("do-something", &context).await;

        match result {
            GateResult::Hook(token) => {
                assert_eq!(token.hook_type, HookType::Cron("* * * * *".to_string()));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_cron_gate_name_and_description() {
        let gate = CronGate::new("weekly-report", "Triggers every Monday at 9am");
        assert_eq!(gate.name(), "weekly-report");
        assert_eq!(gate.description(), "Triggers every Monday at 9am");
    }
}
