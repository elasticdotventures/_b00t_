//! Eisenhower Matrix router — pure logic for Zellij gate integration.
//!
//! Uses `b00t_c0re_lib` types (EisenhowerQuadrant, GateResult) to classify
//! actions by urgency+importance and produce routing decisions. This is the
//! shared classification layer used by both CLI (ZellijGate) and WASM targets.

use b00t_c0re_lib::{EisenhowerQuadrant, GateResult};

/// A stateless Eisenhower router that maps (urgency, importance) to
/// a quadrant and the corresponding gate result.
///
/// # Routing table
/// | Urgent | Important | Quadrant                 | Gate Result        |
/// |--------|-----------|--------------------------|--------------------|
/// | true   | true      | UrgentImportant          | Allow              |
/// | false  | true      | NotUrgentImportant       | Hook (menu)        |
/// | true   | false     | UrgentNotImportant       | Hook (subagent)    |
/// | false  | false     | NotUrgentNotImportant    | Deny               |
///
/// # Examples
/// ```
/// # use b00t_c0re_gov::eisenhower::EisenhowerRouter;
/// let (quadrant, result) = EisenhowerRouter::route(true, true);
/// assert!(result.is_allowed());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct EisenhowerRouter;

impl EisenhowerRouter {
    /// Create a new Eisenhower router.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Classify an action by urgency and importance, returning both the
    /// Eisenhower quadrant and the corresponding gate routing decision.
    ///
    /// This is the main entry point — takes two booleans and returns the
    /// complete routing pair.
    pub fn route(urgency: bool, importance: bool) -> (EisenhowerQuadrant, GateResult) {
        let quadrant = EisenhowerQuadrant::classify(urgency, importance);
        let result = Self::result_for_quadrant(quadrant);
        (quadrant, result)
    }

    /// Map an Eisenhower quadrant directly to a gate result.
    ///
    /// Pure function — deterministic mapping with no side effects.
    pub fn result_for_quadrant(quadrant: EisenhowerQuadrant) -> GateResult {
        match quadrant {
            EisenhowerQuadrant::UrgentImportant => GateResult::Allow,
            EisenhowerQuadrant::NotUrgentImportant => GateResult::Hook,
            EisenhowerQuadrant::UrgentNotImportant => GateResult::Hook,
            EisenhowerQuadrant::NotUrgentNotImportant => GateResult::Deny,
        }
    }

    /// Map an Eisenhower quadrant to a human-readable hook reason string.
    ///
    /// Used for audit trails and HookToken descriptions in the
    /// b00t-c0re-gov richer type system.
    pub fn hook_reason(quadrant: EisenhowerQuadrant) -> &'static str {
        match quadrant {
            EisenhowerQuadrant::NotUrgentImportant => "menu-interaction",
            EisenhowerQuadrant::UrgentNotImportant => "subagent-delegate",
            EisenhowerQuadrant::UrgentImportant => "confirm-required",
            EisenhowerQuadrant::NotUrgentNotImportant => "eliminated",
        }
    }
}

impl Default for EisenhowerRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn test_urgent_important_returns_allow() {
        let (_quadrant, result) = EisenhowerRouter::route(true, true);
        assert!(
            result.is_allowed(),
            "Urgent+Important should return Allow, got {result:?}"
        );
    }

    #[test]
    fn test_not_urgent_important_returns_hook() {
        let (quadrant, result) = EisenhowerRouter::route(false, true);
        assert_eq!(quadrant, EisenhowerQuadrant::NotUrgentImportant);
        assert!(
            result.is_hook(),
            "NotUrgent+Important should return Hook, got {result:?}"
        );
    }

    #[test]
    fn test_urgent_not_important_returns_hook() {
        let (quadrant, result) = EisenhowerRouter::route(true, false);
        assert_eq!(quadrant, EisenhowerQuadrant::UrgentNotImportant);
        assert!(
            result.is_hook(),
            "Urgent+NotImportant should return Hook, got {result:?}"
        );
    }

    #[test]
    fn test_not_urgent_not_important_returns_deny() {
        let (_quadrant, result) = EisenhowerRouter::route(false, false);
        assert!(
            result.is_denied(),
            "NotUrgent+NotImportant should return Deny, got {result:?}"
        );
    }

    #[test]
    fn test_all_four_quadrants_covered() {
        let cases = [
            (
                true,
                true,
                EisenhowerQuadrant::UrgentImportant,
                GateResult::Allow,
            ),
            (
                false,
                true,
                EisenhowerQuadrant::NotUrgentImportant,
                GateResult::Hook,
            ),
            (
                true,
                false,
                EisenhowerQuadrant::UrgentNotImportant,
                GateResult::Hook,
            ),
            (
                false,
                false,
                EisenhowerQuadrant::NotUrgentNotImportant,
                GateResult::Deny,
            ),
        ];

        for (urgency, importance, expected_quadrant, expected_result) in cases {
            let (quadrant, result) = EisenhowerRouter::route(urgency, importance);
            assert_eq!(
                quadrant, expected_quadrant,
                "Quadrant mismatch for ({urgency}, {importance})"
            );
            assert_eq!(
                result, expected_result,
                "Result mismatch for ({urgency}, {importance})"
            );
        }
    }

    #[test]
    fn test_hook_reason_matches_quadrant() {
        assert_eq!(
            EisenhowerRouter::hook_reason(EisenhowerQuadrant::UrgentImportant),
            "confirm-required"
        );
        assert_eq!(
            EisenhowerRouter::hook_reason(EisenhowerQuadrant::NotUrgentImportant),
            "menu-interaction"
        );
        assert_eq!(
            EisenhowerRouter::hook_reason(EisenhowerQuadrant::UrgentNotImportant),
            "subagent-delegate"
        );
        assert_eq!(
            EisenhowerRouter::hook_reason(EisenhowerQuadrant::NotUrgentNotImportant),
            "eliminated"
        );
    }

    #[test]
    fn test_router_default() {
        let _router = EisenhowerRouter::default();
        let (_q, r) = EisenhowerRouter::route(true, true);
        assert!(r.is_allowed());
    }
}
