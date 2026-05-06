use async_trait::async_trait;
use uuid::Uuid;

use crate::traits::*;
use crate::types::*;

/// Eisenhower quadrant for a task.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Quadrant {
    Do,          // Urgent + Important → Allow immediately
    Schedule,    // Not Urgent + Important → Hook with timer
    Delegate,    // Urgent + Not Important → Hook with player tag
    Eliminate,   // Not Urgent + Not Important → Deny
}

impl Quadrant {
    pub fn from_urgency_importance(urgency: f64, importance: f64) -> Self {
        let urgent = urgency >= 0.5;
        let important = importance >= 0.5;
        match (urgent, important) {
            (true,  true)  => Quadrant::Do,
            (false, true)  => Quadrant::Schedule,
            (true,  false) => Quadrant::Delegate,
            (false, false) => Quadrant::Eliminate,
        }
    }
}

/// A gate that applies the Eisenhower Matrix to prioritize actions.
/// Returns Allow, Hook (timer or event), or Deny based on urgency and importance.
pub struct EisenhowerGate {
    name: String,
    urgency_threshold: f64,    // default 0.5
    importance_threshold: f64, // default 0.5
    schedule_ms: u64,          // default 60000 (1 min)
    description: String,
}

impl EisenhowerGate {
    pub fn new(name: &str) -> Self {
        EisenhowerGate {
            name: name.to_string(),
            urgency_threshold: 0.5,
            importance_threshold: 0.5,
            schedule_ms: 60_000,
            description: format!("Eisenhower Matrix gate '{}'", name),
        }
    }

    pub fn with_thresholds(mut self, urgency: f64, importance: f64) -> Self {
        self.urgency_threshold = urgency;
        self.importance_threshold = importance;
        self
    }

    pub fn with_schedule_ms(mut self, ms: u64) -> Self {
        self.schedule_ms = ms;
        self
    }
}

#[async_trait]
impl GovernanceGate for EisenhowerGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, context: &GateCheckContext) -> GateResult {
        // Parse urgency and importance from metadata
        let urgency = context
            .metadata
            .get("urgency")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);
        let importance = context
            .metadata
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        let quadrant = Quadrant::from_urgency_importance(urgency, importance);

        match quadrant {
            Quadrant::Do => GateResult::Allow,
            Quadrant::Schedule => GateResult::Hook(HookToken {
                id: Uuid::new_v4(),
                hook_type: HookType::TimerMs(self.schedule_ms),
                created_at: chrono::Utc::now(),
                ttl_ms: Some(self.schedule_ms + 5000),
                description: format!(
                    "Eisenhower Schedule: '{}' deferred for {}ms",
                    action, self.schedule_ms
                ),
            }),
            Quadrant::Delegate => GateResult::Hook(HookToken {
                id: Uuid::new_v4(),
                hook_type: HookType::Event(format!("delegate:{}", action)),
                created_at: chrono::Utc::now(),
                ttl_ms: Some(300_000), // 5 min TTL
                description: format!(
                    "Eisenhower Delegate: '{}' waiting for assignment",
                    action
                ),
            }),
            Quadrant::Eliminate => GateResult::Deny {
                reason: format!(
                    "Eisenhower Eliminate: '{}' is neither urgent nor important",
                    action
                ),
                escalation_path: None,
            },
        }
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[tokio::test]
    async fn test_do_quadrant() {
        let gate = EisenhowerGate::new("test");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({
                "urgency": 0.9,
                "importance": 0.9,
            }),
        };

        let result = gate.check("do-something", &context).await;
        assert!(matches!(result, GateResult::Allow), "Expected Allow, got {:?}", result);
    }

    #[tokio::test]
    async fn test_schedule_quadrant() {
        let gate = EisenhowerGate::new("test");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({
                "urgency": 0.3,
                "importance": 0.9,
            }),
        };

        let result = gate.check("do-something", &context).await;
        match result {
            GateResult::Hook(token) => {
                assert!(
                    matches!(token.hook_type, HookType::TimerMs(_)),
                    "Expected TimerMs hook, got {:?}",
                    token.hook_type
                );
                if let HookType::TimerMs(ms) = token.hook_type {
                    assert_eq!(ms, 60_000);
                }
                assert!(token.description.contains("Schedule"));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_delegate_quadrant() {
        let gate = EisenhowerGate::new("test");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({
                "urgency": 0.9,
                "importance": 0.3,
            }),
        };

        let result = gate.check("do-something", &context).await;
        match result {
            GateResult::Hook(token) => {
                assert!(
                    matches!(token.hook_type, HookType::Event(_)),
                    "Expected Event hook, got {:?}",
                    token.hook_type
                );
                if let HookType::Event(ref e) = token.hook_type {
                    assert!(e.contains("delegate:"));
                }
                assert!(token.description.contains("Delegate"));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_eliminate_quadrant() {
        let gate = EisenhowerGate::new("test");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({
                "urgency": 0.3,
                "importance": 0.3,
            }),
        };

        let result = gate.check("do-something", &context).await;
        match result {
            GateResult::Deny { reason, escalation_path } => {
                assert!(reason.contains("Eliminate"));
                assert!(escalation_path.is_none());
            }
            _ => panic!("Expected Deny result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_default_thresholds() {
        // No metadata: defaults both to 0.5
        // urgency=0.5 >= 0.5 -> urgent=true
        // importance=0.5 >= 0.5 -> important=true
        // (true, true) -> Quadrant::Do -> Allow
        let gate = EisenhowerGate::new("test");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = gate.check("do-something", &context).await;
        assert!(
            matches!(result, GateResult::Allow),
            "Expected Allow with default (0.5, 0.5), got {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_custom_thresholds() {
        let gate = EisenhowerGate::new("test")
            .with_thresholds(0.5, 0.5);
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({
                "urgency": 0.1,
                "importance": 0.6,
            }),
        };

        let result = gate.check("do-something", &context).await;
        match result {
            GateResult::Hook(token) => {
                assert!(
                    matches!(token.hook_type, HookType::TimerMs(_)),
                    "Expected TimerMs hook, got {:?}",
                    token.hook_type
                );
                assert!(token.description.contains("Schedule"));
            }
            _ => panic!("Expected Hook(Schedule), got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_boundary_exact_threshold() {
        let gate = EisenhowerGate::new("test");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({
                "urgency": 0.5,
                "importance": 0.5,
            }),
        };

        let result = gate.check("do-something", &context).await;
        assert!(
            matches!(result, GateResult::Allow),
            "Expected Allow for exact boundary (0.5, 0.5), got {:?}",
            result
        );
    }
}
