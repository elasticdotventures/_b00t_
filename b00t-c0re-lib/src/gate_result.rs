//! Governance gate result types for Zellij interaction.
//!
//! Implements the 3-return governance pattern (Allow/Deny/Hook)
//! used by the Zellij gate to route user interactions.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Governance gate result — the 3-return pattern.
///
/// Maps to bash exit codes for compatibility with shell gate wrappers:
/// - Allow → exit 0 (proceed)
/// - Deny  → exit 1 (block)
/// - Hook  → exit 2 (defer/snapshot)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GateResult {
    /// Action is approved — proceed immediately.
    Allow,
    /// Action is denied — blocked permanently.
    Deny,
    /// Action is deferred — agent snapshots and hooks into event loop.
    Hook,
}

impl GateResult {
    /// Convert to POSIX exit code for shell compatibility.
    ///
    /// # Exit codes
    /// - `0` = Allow
    /// - `1` = Deny
    /// - `2` = Hook
    pub fn exit_code(&self) -> i32 {
        match self {
            GateResult::Allow => 0,
            GateResult::Deny => 1,
            GateResult::Hook => 2,
        }
    }

    /// Human-readable label for audit/log display.
    pub fn as_label(&self) -> &'static str {
        match self {
            GateResult::Allow => "Allow",
            GateResult::Deny => "Deny",
            GateResult::Hook => "Hook",
        }
    }

    /// Returns true if the action is allowed to proceed.
    pub fn is_allowed(&self) -> bool {
        matches!(self, GateResult::Allow)
    }

    /// Returns true if the action is denied.
    pub fn is_denied(&self) -> bool {
        matches!(self, GateResult::Deny)
    }

    /// Returns true if the action is deferred via hook.
    pub fn is_hook(&self) -> bool {
        matches!(self, GateResult::Hook)
    }
}

/// Display impl shows the human-readable label.
///
/// Required by the Zellij gate system for audit trail formatting.
impl std::fmt::Display for GateResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_label())
    }
}

/// A complete gate decision with audit metadata and caching.
///
/// Produced by the governance gate system when checking an action.
/// Carries the result plus metadata for audit trail and caching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateDecision {
    /// The governance result for this check.
    pub result: GateResult,
    /// JSON audit entry for the decision trail.
    pub audit_entry: serde_json::Value,
    /// Suggested timestamp for the next gate check (None = no scheduling).
    #[serde(default)]
    pub next_check: Option<DateTime<Utc>>,
    /// How long this decision is valid for caching (None = no caching).
    #[serde(default)]
    pub cached_until: Option<DateTime<Utc>>,
}

impl GateDecision {
    /// Create a new gate decision with the given result and audit entry.
    pub fn new(result: GateResult, audit_entry: serde_json::Value) -> Self {
        Self {
            result,
            audit_entry,
            next_check: None,
            cached_until: None,
        }
    }

    /// Set the next check timestamp for scheduled re-evaluation.
    pub fn with_next_check(mut self, next: DateTime<Utc>) -> Self {
        self.next_check = Some(next);
        self
    }

    /// Set the cache expiry for this decision.
    pub fn with_cache(mut self, cached_until: DateTime<Utc>) -> Self {
        self.cached_until = Some(cached_until);
        self
    }

    /// Returns true if this decision is still valid (not past cached_until).
    pub fn is_cache_valid(&self) -> bool {
        self.cached_until
            .map(|t| Utc::now() < t)
            .unwrap_or(false)
    }
}

/// A Zellij gate check record — wraps a [`GateDecision`] with session metadata.
///
/// Stored in the KV store as part of the gate cache for audit trail
/// and to prevent redundant user prompts within the cache window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZellijGate {
    /// When the gate was last checked.
    pub checked_at: DateTime<Utc>,
    /// The Zellij session name (from ZELLIJ_SESSION_NAME).
    pub session: String,
    /// The agent ID that triggered the check.
    pub agent_id: String,
    /// The gate decision result.
    pub result: GateDecision,
}

impl ZellijGate {
    /// Create a new Zellij gate record.
    pub fn new(session: String, agent_id: String, result: GateDecision) -> Self {
        Self {
            checked_at: Utc::now(),
            session,
            agent_id,
            result,
        }
    }

    /// Returns true if the gate check has a valid cached result.
    pub fn is_cache_valid(&self) -> bool {
        self.result.is_cache_valid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gate_result_exit_codes() {
        assert_eq!(GateResult::Allow.exit_code(), 0);
        assert_eq!(GateResult::Deny.exit_code(), 1);
        assert_eq!(GateResult::Hook.exit_code(), 2);
    }

    #[test]
    fn test_gate_result_display() {
        assert_eq!(GateResult::Allow.to_string(), "Allow");
        assert_eq!(GateResult::Deny.to_string(), "Deny");
        assert_eq!(GateResult::Hook.to_string(), "Hook");
    }

    #[test]
    fn test_gate_result_is_methods() {
        assert!(GateResult::Allow.is_allowed());
        assert!(!GateResult::Allow.is_denied());
        assert!(!GateResult::Allow.is_hook());

        assert!(!GateResult::Deny.is_allowed());
        assert!(GateResult::Deny.is_denied());
        assert!(!GateResult::Deny.is_hook());

        assert!(!GateResult::Hook.is_allowed());
        assert!(!GateResult::Hook.is_denied());
        assert!(GateResult::Hook.is_hook());
    }

    #[test]
    fn test_gate_decision_serialization() {
        let decision = GateDecision::new(
            GateResult::Allow,
            serde_json::json!({"action": "test", "timestamp": "2026-06-20T00:00:00Z"}),
        );
        let json = serde_json::to_string(&decision).unwrap();
        let parsed: GateDecision = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.result, GateResult::Allow);
        assert_eq!(parsed.audit_entry["action"], "test");
    }

    #[test]
    fn test_gate_decision_cache_valid() {
        let future = Utc::now() + chrono::Duration::minutes(5);
        let decision = GateDecision::new(
            GateResult::Allow,
            serde_json::json!({}),
        )
        .with_cache(future);
        assert!(decision.is_cache_valid());

        let past = Utc::now() - chrono::Duration::minutes(5);
        let expired = GateDecision::new(
            GateResult::Allow,
            serde_json::json!({}),
        )
        .with_cache(past);
        assert!(!expired.is_cache_valid());
    }

    #[test]
    fn test_zellij_gate_creation() {
        let decision = GateDecision::new(GateResult::Hook, serde_json::json!({}));
        let gate = ZellijGate::new(
            "my-session".to_string(),
            "agent-42".to_string(),
            decision.clone(),
        );
        assert_eq!(gate.session, "my-session");
        assert_eq!(gate.agent_id, "agent-42");
        assert_eq!(gate.result.result, GateResult::Hook);
    }
}
