use async_trait::async_trait;
use uuid::Uuid;

use crate::traits::*;
use crate::types::*;

/// A gate that returns a Hook that waits for a specific event to be emitted.
pub struct EventGate {
    name: String,
    event_id: String,
    description: String,
}

impl EventGate {
    /// Create a gate that waits for `event_id` to be emitted.
    pub fn new(name: &str, event_id: &str, description: &str) -> Self {
        EventGate {
            name: name.to_string(),
            event_id: event_id.to_string(),
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl GovernanceGate for EventGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, _context: &GateCheckContext) -> GateResult {
        GateResult::Hook(HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::Event(self.event_id.clone()),
            created_at: chrono::Utc::now(),
            ttl_ms: None, // Events wait indefinitely (until cancelled or TTL)
            description: format!(
                "Event gate '{}': waiting for event '{}' before allowing '{}'",
                self.name, self.event_id, action
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
    async fn test_event_gate_returns_hook() {
        let gate = EventGate::new(
            "wait-approval",
            "approval.granted",
            "Wait for human approval",
        );
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "deploy".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = gate.check("deploy", &context).await;

        match result {
            GateResult::Hook(token) => {
                assert_eq!(
                    token.hook_type,
                    HookType::Event("approval.granted".to_string())
                );
                assert!(token.ttl_ms.is_none());
                assert!(token.description.contains("wait-approval"));
                assert!(token.description.contains("approval.granted"));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_event_gate_name_and_description() {
        let gate = EventGate::new("my-event-gate", "event.xyz", "An event gate");
        assert_eq!(gate.name(), "my-event-gate");
        assert_eq!(gate.description(), "An event gate");
    }
}
