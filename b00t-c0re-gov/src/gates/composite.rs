use async_trait::async_trait;
use uuid::Uuid;

use crate::traits::*;
use crate::types::*;

/// A gate that returns a Hook that fires when ANY of its child hooks fire.
/// The children are created by delegating to sub-gates.
pub struct AnyOfGate {
    name: String,
    sub_gates: Vec<Box<dyn GovernanceGate>>,
    description: String,
}

impl AnyOfGate {
    /// Create an AnyOf gate from a list of sub-gates.
    /// On `check()`, it checks all sub-gates and collects their Hook results.
    pub fn new(name: &str, sub_gates: Vec<Box<dyn GovernanceGate>>, description: &str) -> Self {
        AnyOfGate {
            name: name.to_string(),
            sub_gates,
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl GovernanceGate for AnyOfGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, context: &GateCheckContext) -> GateResult {
        let mut children = Vec::new();

        for gate in &self.sub_gates {
            match gate.check(action, context).await {
                GateResult::Hook(token) => {
                    children.push(token);
                }
                GateResult::Allow => {
                    // If any gate allows immediately, the AnyOf is satisfied trivially.
                    // We can still return Allow — but the semantics say we should
                    // return a hook. We'll add a trivial (0ms) timer child.
                    children.push(HookToken {
                        id: Uuid::new_v4(),
                        hook_type: HookType::TimerMs(0),
                        created_at: chrono::Utc::now(),
                        ttl_ms: Some(5000),
                        description: format!("Allow child for gate '{}'", gate.name()),
                    });
                }
                GateResult::Deny { reason, escalation_path: _ } => {
                    // A denial in AnyOf doesn't stop the entire check;
                    // we still wait for the other gates. But we note it.
                    // We can add a timer that fires immediately to represent "skipped".
                    children.push(HookToken {
                        id: Uuid::new_v4(),
                        hook_type: HookType::TimerMs(0),
                        created_at: chrono::Utc::now(),
                        ttl_ms: Some(5000),
                        description: format!("Denied ({}), skipped", reason),
                    });
                }
            }
        }

        GateResult::Hook(HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::AnyOf(children),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: format!(
                "AnyOf gate '{}': waiting for ANY of {} sub-gates for '{}'",
                self.name,
                self.sub_gates.len(),
                action
            ),
        })
    }

    fn description(&self) -> &str {
        &self.description
    }
}

/// A gate that returns a Hook that fires when ALL of its child hooks fire.
pub struct AllOfGate {
    name: String,
    sub_gates: Vec<Box<dyn GovernanceGate>>,
    description: String,
}

impl AllOfGate {
    /// Create an AllOf gate from a list of sub-gates.
    pub fn new(name: &str, sub_gates: Vec<Box<dyn GovernanceGate>>, description: &str) -> Self {
        AllOfGate {
            name: name.to_string(),
            sub_gates,
            description: description.to_string(),
        }
    }
}

#[async_trait]
impl GovernanceGate for AllOfGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, context: &GateCheckContext) -> GateResult {
        let mut children = Vec::new();

        for gate in &self.sub_gates {
            match gate.check(action, context).await {
                GateResult::Hook(token) => {
                    children.push(token);
                }
                GateResult::Allow => {
                    // If a gate allows immediately, add a 0ms timer as a "trivial child"
                    children.push(HookToken {
                        id: Uuid::new_v4(),
                        hook_type: HookType::TimerMs(0),
                        created_at: chrono::Utc::now(),
                        ttl_ms: Some(5000),
                        description: format!("Allow child for gate '{}'", gate.name()),
                    });
                }
                GateResult::Deny { reason, escalation_path } => {
                    // A denial in AllOf stops everything — we deny immediately
                    return GateResult::Deny {
                        reason: format!(
                            "AllOf gate '{}' denied because sub-gate '{}' denied: {}",
                            self.name,
                            gate.name(),
                            reason
                        ),
                        escalation_path,
                    };
                }
            }
        }

        GateResult::Hook(HookToken {
            id: Uuid::new_v4(),
            hook_type: HookType::AllOf(children),
            created_at: chrono::Utc::now(),
            ttl_ms: None,
            description: format!(
                "AllOf gate '{}': waiting for ALL of {} sub-gates for '{}'",
                self.name,
                self.sub_gates.len(),
                action
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
    use crate::gates::timer::TimerGate;

    #[tokio::test]
    async fn test_anyof_gate_returns_anyof_hook() {
        let gate1 = TimerGate::new("timer-1", 100, "Timer 1");
        let gate2 = TimerGate::new("timer-2", 200, "Timer 2");

        let anyof = AnyOfGate::new(
            "any-of-timers",
            vec![Box::new(gate1), Box::new(gate2)],
            "Wait for any timer",
        );

        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test".to_string(),
            action: "act".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = anyof.check("act", &context).await;

        match result {
            GateResult::Hook(token) => {
                match &token.hook_type {
                    HookType::AnyOf(children) => {
                        assert_eq!(children.len(), 2, "Should have 2 child hooks");
                        // Children should be TimerMs
                        for child in children {
                            assert!(
                                matches!(child.hook_type, HookType::TimerMs(_)),
                                "Expected TimerMs child, got {:?}",
                                child.hook_type
                            );
                        }
                    }
                    _ => panic!("Expected AnyOf hook type"),
                }
                assert!(token.description.contains("any-of-timers"));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_allof_gate_returns_allof_hook() {
        let gate1 = TimerGate::new("timer-1", 100, "Timer 1");
        let gate2 = TimerGate::new("timer-2", 200, "Timer 2");

        let allof = AllOfGate::new(
            "all-of-timers",
            vec![Box::new(gate1), Box::new(gate2)],
            "Wait for all timers",
        );

        let context = GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test".to_string(),
            action: "act".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = allof.check("act", &context).await;

        match result {
            GateResult::Hook(token) => {
                match &token.hook_type {
                    HookType::AllOf(children) => {
                        assert_eq!(children.len(), 2, "Should have 2 child hooks");
                    }
                    _ => panic!("Expected AllOf hook type"),
                }
                assert!(token.description.contains("all-of-timers"));
            }
            _ => panic!("Expected Hook result, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_anyof_gate_name_and_description() {
        let gate = AnyOfGate::new("my-anyof", vec![], "An AnyOf gate");
        assert_eq!(gate.name(), "my-anyof");
        assert_eq!(gate.description(), "An AnyOf gate");
    }

    #[tokio::test]
    async fn test_allof_gate_name_and_description() {
        let gate = AllOfGate::new("my-allof", vec![], "An AllOf gate");
        assert_eq!(gate.name(), "my-allof");
        assert_eq!(gate.description(), "An AllOf gate");
    }

    #[tokio::test]
    async fn test_allof_denies_if_subgate_denies() {
        use crate::traits::GateCheckContext;

        struct DenyGate;

        #[async_trait]
        impl GovernanceGate for DenyGate {
            fn name(&self) -> &str {
                "deny-gate"
            }

            async fn check(&self, _action: &str, _context: &GateCheckContext) -> GateResult {
                GateResult::Deny {
                    reason: "always denies".to_string(),
                    escalation_path: None,
                }
            }

            fn description(&self) -> &str {
                "Always denies"
            }
        }

        let allof = AllOfGate::new(
            "strict-allof",
            vec![Box::new(DenyGate), Box::new(TimerGate::new("t", 100, "t"))],
            "Strict check",
        );

        let context = GateCheckContext {
            agent_id: "test".to_string(),
            task: "test".to_string(),
            action: "test".to_string(),
            metadata: serde_json::json!({}),
        };

        let result = allof.check("test", &context).await;
        match result {
            GateResult::Deny { reason, .. } => {
                assert!(reason.contains("strict-allof"), "Deny reason: {reason}");
                assert!(reason.contains("always denies"), "Deny reason: {reason}");
            }
            other => panic!("Expected Deny, got {:?}", other),
        }
    }
}
