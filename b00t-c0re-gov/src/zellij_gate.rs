//! Zellij governance gate — bridges b00t-c0re-lib Eisenhower routing
//! into the b00t-c0re-gov GovernanceGate trait system.
//!
//! # Architecture
//!
//! ZellijGate (GovernanceGate impl) uses:
//! - EisenhowerRouter (b00t_c0re_lib types) for classify(urgent, important)
//! - GateAudit (JSONL append-only audit trail)
//! - Maps b00t_c0re_lib::GateResult to b00t_c0re_gov::types::GateResult
//!
//! # Routing table
//! | b00t-c0re-lib Quadrant     | b00t-c0re-gov GateResult                        |
//! |----------------------------|------------------------------------------------|
//! | UrgentImportant            | Allow                                           |
//! | NotUrgentImportant         | Hook(Event: "menu-interaction")                 |
//! | UrgentNotImportant         | Hook(Event: "subagent-delegate")                |
//! | NotUrgentNotImportant      | Deny { reason: "Eisenhower Eliminate: ..." }    |

use async_trait::async_trait;
use b00t_c0re_lib::EisenhowerQuadrant;
use uuid::Uuid;

use crate::eisenhower::EisenhowerRouter;
use crate::gate_audit::GateAudit;
use crate::traits::*;
use crate::types::*;

/// A Zellij governance gate that routes actions through the Eisenhower matrix.
///
/// Implements [`GovernanceGate`] so it can be composed with other gates
/// (AnyOfGate, AllOfGate, etc.) in the b00t-c0re-gov system.
///
/// # Detection bypass
/// When Zellij is not detected (no `ZELLIJ_SESSION_NAME` env var), the gate
/// returns `Allow` unconditionally — the gate is bypassed outside Zellij sessions.
#[derive(Debug, Clone)]
pub struct ZellijGate {
    /// Unique name for this gate instance.
    name: String,
    /// Whether to auto-detect Zellij and bypass when absent.
    auto_detect: bool,
    /// Whether to write audit entries to the JSONL trail.
    audit_enabled: bool,
    /// Optional human-readable description.
    description: String,
}

impl ZellijGate {
    /// Create a new Zellij gate with the given name.
    ///
    /// Defaults: auto-detection enabled, audit enabled.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            auto_detect: true,
            audit_enabled: true,
            description: format!("Zellij interaction gate '{name}' — Eisenhower matrix routing"),
        }
    }

    /// Disable Zellij auto-detection (always applies gate logic).
    #[must_use]
    pub fn without_auto_detect(mut self) -> Self {
        self.auto_detect = false;
        self
    }

    /// Disable audit trail writing.
    #[must_use]
    pub fn without_audit(mut self) -> Self {
        self.audit_enabled = false;
        self
    }

    /// Check whether Zellij is currently active (via `ZELLIJ_SESSION_NAME`).
    fn zellij_detected() -> bool {
        std::env::var("ZELLIJ_SESSION_NAME").is_ok()
    }

    /// Map a `b00t_c0re_lib::EisenhowerQuadrant` to a `b00t_c0re_gov::types::GateResult`.
    ///
    /// The mapping adds HookToken metadata for the two Hook variants so the
    /// scheduler knows what kind of interaction to expect.
    fn map_to_gov_result(quadrant: EisenhowerQuadrant, action: &str) -> crate::types::GateResult {
        let reason = EisenhowerRouter::hook_reason(quadrant);
        match quadrant {
            EisenhowerQuadrant::UrgentImportant => GateResult::Allow,
            EisenhowerQuadrant::NotUrgentImportant => {
                // NU+I → Hook with menu interaction event
                GateResult::Hook(HookToken {
                    id: Uuid::new_v4(),
                    hook_type: HookType::Event(format!("zellij:menu:{action}")),
                    created_at: chrono::Utc::now(),
                    ttl_ms: Some(300_000), // 5-minute TTL
                    description: format!(
                        "Zellij menu interaction required for '{action}' ({reason})"
                    ),
                })
            }
            EisenhowerQuadrant::UrgentNotImportant => {
                // U+NI → Hook with subagent delegation event
                GateResult::Hook(HookToken {
                    id: Uuid::new_v4(),
                    hook_type: HookType::Event(format!("zellij:subagent:{action}")),
                    created_at: chrono::Utc::now(),
                    ttl_ms: Some(300_000),
                    description: format!("Zellij subagent delegation for '{action}' ({reason})"),
                })
            }
            EisenhowerQuadrant::NotUrgentNotImportant => {
                // NU+NI → Deny with reason
                GateResult::Deny {
                    reason: format!(
                        "Eisenhower Eliminate: '{action}' is neither urgent nor important"
                    ),
                    escalation_path: None,
                }
            }
        }
    }

    /// Write an audit entry for this gate check.
    fn write_audit(action: &str, quadrant: EisenhowerQuadrant, agent_id: &str, session_id: &str) {
        let entry = GateAudit::new(
            action,
            quadrant.to_string(),
            EisenhowerRouter::hook_reason(quadrant),
            agent_id,
            session_id,
        );

        let path = GateAudit::default_path();
        if let Err(e) = entry.append_to_path(&path) {
            // Audit failures are non-fatal — log and continue
            eprintln!(
                "ZellijGate: failed to write audit entry to {}: {e}",
                path.display()
            );
        }
    }
}

#[async_trait]
impl GovernanceGate for ZellijGate {
    fn name(&self) -> &str {
        &self.name
    }

    async fn check(&self, action: &str, context: &GateCheckContext) -> GateResult {
        // Bypass gate if Zellij is not detected
        if self.auto_detect && !Self::zellij_detected() {
            return GateResult::Allow;
        }

        // Parse urgency and importance from context metadata
        let urgency = context
            .metadata
            .get("urgency")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let importance = context
            .metadata
            .get("importance")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Route through Eisenhower matrix
        let (quadrant, _lib_result) = EisenhowerRouter::route(urgency, importance);

        // Write audit trail if enabled
        if self.audit_enabled {
            Self::write_audit(action, quadrant, &context.agent_id, &context.task);
        }

        // Map to b00t-c0re-gov GateResult
        Self::map_to_gov_result(quadrant, action)
    }

    fn description(&self) -> &str {
        &self.description
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    fn test_context(urgency: bool, importance: bool) -> GateCheckContext {
        GateCheckContext {
            agent_id: "test-agent".to_string(),
            task: "test-session".to_string(),
            action: "build".to_string(),
            metadata: serde_json::json!({
                "urgency": urgency,
                "importance": importance,
            }),
        }
    }

    #[tokio::test]
    async fn test_urgent_important_returns_allow() {
        let gate = ZellijGate::new("test")
            .without_auto_detect()
            .without_audit();
        let ctx = test_context(true, true);
        let result = gate.check("build", &ctx).await;
        assert!(
            matches!(result, GateResult::Allow),
            "Expected Allow, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_not_urgent_important_returns_hook_menu() {
        let gate = ZellijGate::new("test")
            .without_auto_detect()
            .without_audit();
        let ctx = test_context(false, true);
        let result = gate.check("build", &ctx).await;
        match result {
            GateResult::Hook(token) => {
                assert!(
                    matches!(token.hook_type, HookType::Event(_)),
                    "Expected Event hook, got {:?}",
                    token.hook_type
                );
                if let HookType::Event(ref e) = token.hook_type {
                    assert!(e.contains("zellij:menu:"), "Expected menu event, got {e}");
                    assert!(e.contains("build"), "Event should contain action, got {e}");
                }
                assert!(
                    token.description.contains("menu interaction"),
                    "Description should mention menu: {}",
                    token.description
                );
            }
            other => panic!("Expected Hook, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_urgent_not_important_returns_hook_subagent() {
        let gate = ZellijGate::new("test")
            .without_auto_detect()
            .without_audit();
        let ctx = test_context(true, false);
        let result = gate.check("deploy", &ctx).await;
        match result {
            GateResult::Hook(token) => {
                assert!(
                    matches!(token.hook_type, HookType::Event(_)),
                    "Expected Event hook, got {:?}",
                    token.hook_type
                );
                if let HookType::Event(ref e) = token.hook_type {
                    assert!(
                        e.contains("zellij:subagent:"),
                        "Expected subagent event, got {e}"
                    );
                    assert!(e.contains("deploy"), "Event should contain action, got {e}");
                }
                assert!(
                    token.description.contains("subagent delegation"),
                    "Description should mention subagent: {}",
                    token.description
                );
            }
            other => panic!("Expected Hook, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_not_urgent_not_important_returns_deny() {
        let gate = ZellijGate::new("test")
            .without_auto_detect()
            .without_audit();
        let ctx = test_context(false, false);
        let result = gate.check("nop", &ctx).await;
        match result {
            GateResult::Deny {
                reason,
                escalation_path,
            } => {
                assert!(
                    reason.contains("Eisenhower Eliminate"),
                    "Deny reason should mention Eliminate: {reason}"
                );
                assert!(
                    reason.contains("nop"),
                    "Reason should contain action: {reason}"
                );
                assert!(escalation_path.is_none());
            }
            other => panic!("Expected Deny, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_name_and_description() {
        let gate = ZellijGate::new("my-zellij-gate");
        assert_eq!(gate.name(), "my-zellij-gate");
        assert!(gate.description().contains("my-zellij-gate"));
    }

    #[tokio::test]
    async fn test_default_metadata_returns_deny() {
        // urgency=false, importance=false when metadata is empty
        let gate = ZellijGate::new("test")
            .without_auto_detect()
            .without_audit();
        let ctx = GateCheckContext {
            agent_id: "agent".to_string(),
            task: "session".to_string(),
            action: "act".to_string(),
            metadata: serde_json::json!({}),
        };
        let result = gate.check("act", &ctx).await;
        match result {
            GateResult::Deny { .. } => {} // expected
            other => panic!("Empty metadata should default to Deny, got {other:?}"),
        }
    }
}
