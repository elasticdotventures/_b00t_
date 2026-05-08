use async_trait::async_trait;
use uuid::Uuid;

use crate::traits::*;
use crate::types::*;

/// A gate that returns a Hook with a specific Unix timestamp.
/// The agent must wait until the timestamp is reached before the action is allowed.
/// Reads the timestamp from the `timestamp` field in metadata as i64 Unix seconds.
pub struct AtTimestampGate {
    name: String,
    description: String,
}

impl AtTimestampGate {
    /// Create a gate that returns a Hook with a Unix timestamp from metadata.
    pub fn new(name: &str, description: &str) -> Self {
        AtTimestampGate {
            name: name.to_string(),
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl GovernanceGate for AtTimestampGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, context: &GateCheckContext) -> GateResult {
        let timestamp = context
            .metadata
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        GateResult::Hook(HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::AtTimestamp(timestamp),
            created_at: chrono::Utc::now(),
            ttl_ms: Some(86_400_000), // 24h default TTL for timestamp hooks
            description: format!(
                "AtTimestamp gate '{}': paused '{}' until Unix timestamp {}",
                self.name, action, timestamp
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
    async fn test_at_timestamp_gate_returns_hook_with_timestamp_from_metadata() {
        let gate = AtTimestampGate::new("release-window", "Only allow after release date");
        let timestamp: i64 = 1_756_300_000; // Some future date
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "deploy-release".to_string(),
            metadata: serde_json::json!({"timestamp": timestamp}),
        };

        let result = gate.check("deploy-release", &context).await;

        match result {
            GateResult::Hook(token) => {
                assert_eq!(token.hook_type, HookType::AtTimestamp(timestamp));
                assert!(token.ttl_ms.is_some());
                assert!(token.description.contains("release-window"));
                assert!(token.description.contains(&timestamp.to_string()));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_at_timestamp_gate_defaults_to_zero_when_no_metadata() {
        let gate = AtTimestampGate::new("no-ts", "No timestamp provided");
        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test task".to_string(),
            action: "do-something".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = gate.check("do-something", &context).await;

        match result {
            GateResult::Hook(token) => {
                assert_eq!(token.hook_type, HookType::AtTimestamp(0));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_at_timestamp_gate_name_and_description() {
        let gate = AtTimestampGate::new("time-lock", "Delays until a specific time");
        assert_eq!(gate.name(), "time-lock");
        assert_eq!(gate.description(), "Delays until a specific time");
    }
}
