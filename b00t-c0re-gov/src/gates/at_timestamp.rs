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

        let now_secs = chrono::Utc::now().timestamp();
        // TTL = (target - now) ms + 60 s grace so the hook stays valid until it can fire.
        // If timestamp is in the past or zero, use None (hook fires immediately or is ignored).
        let ttl_ms = if timestamp > now_secs {
            let diff_secs = (timestamp - now_secs) as u64;
            Some(diff_secs.saturating_mul(1_000).saturating_add(60_000))
        } else {
            None
        };

        GateResult::Hook(HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::AtTimestamp(timestamp),
            created_at: chrono::Utc::now(),
            ttl_ms,
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
        // Use a timestamp 7 days in the future so this test stays valid regardless of wall time.
        let timestamp: i64 = chrono::Utc::now().timestamp() + 7 * 86_400;
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
                // timestamp is in the future → TTL must be set and exceed 24h
                assert!(token.ttl_ms.is_some(), "future timestamp must have a TTL");
                assert!(
                    token.ttl_ms.unwrap() > 86_400_000,
                    "TTL should exceed 24h for a far-future timestamp"
                );
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
                // timestamp 0 is in the past → no TTL
                assert!(token.ttl_ms.is_none(), "past/zero timestamp must have no TTL");
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
