use async_trait::async_trait;
use uuid::Uuid;

use crate::traits::*;
use crate::types::*;

/// A gate that returns a Hook with a timer of `duration_ms`.
/// The agent must wait until the timer expires before the action is allowed.
pub struct TimerGate {
    name: String,
    duration_ms: u64,
    description: String,
}

impl TimerGate {
    /// Create a gate that returns a Hook with a timer of `duration_ms`.
    pub fn new(name: &str, duration_ms: u64, description: &str) -> Self {
        TimerGate {
            name: name.to_string(),
            duration_ms,
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl GovernanceGate for TimerGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, _context: &GateCheckContext) -> GateResult {
        GateResult::Hook(HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::TimerMs(self.duration_ms),
            created_at: chrono::Utc::now(),
            ttl_ms: Some(self.duration_ms + 5000), // 5s grace period
            description: format!(
                "Timer gate '{}': waiting {}ms before allowing '{}'",
                self.name, self.duration_ms, action
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
    async fn test_timer_gate_returns_hook() {
        let gate = TimerGate::new("wait-100ms", 100, "Wait 100ms before proceeding");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = gate.check("do-something", &context).await;

        match result {
            GateResult::Hook(token) => {
                assert_eq!(token.hook_type, HookType::TimerMs(100));
                assert!(token.ttl_ms.is_some());
                assert!(token.ttl_ms.unwrap() >= 105);
                assert!(token.description.contains("wait-100ms"));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_timer_gate_name_and_description() {
        let gate = TimerGate::new("slow-down", 5000, "Rate limiter");
        assert_eq!(gate.name(), "slow-down");
        assert_eq!(gate.description(), "Rate limiter");
    }
}
