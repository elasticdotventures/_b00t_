// b00t-c0re-lib/src/reviewer/mod.rs
// 🤓 Reviewer governance types — UFO-grounded, ledgerr-evidenced.
//    Every verdict-driven command MUST produce an evidence node.
//    Without evidence → command DID NOT HAPPEN → rollback.
//
// Inherits Satisfies<Constraint> pattern from Tax-Lawyer architecture.
// UFO grounding: ReviewVerdict = Moment::Mode, CommandEvidence = Endurant::SubKind.

pub mod governance;
pub mod evidence;

pub use governance::{
    EvidenceNode,
    HarnessAction,
    ReviewerCommand,
    SatisfiesResult,
    VerdictConstraint,
    VerdictDisposition,
    VerdictEvaluation,
};
pub use evidence::{
    CommandEvidence,
    ProduceEvidence,
    evidence_approve,
    evidence_request_changes,
};

/// Canonical verdict values — must match CakeTicket CHECK constraint
pub const VERDICT_APPROVE: &str = "APPROVE";
pub const VERDICT_REQUEST_CHANGES: &str = "REQUEST_CHANGES";
pub const VERDICT_REJECT: &str = "REJECT";

/// Parse a verdict string into a VerdictDisposition
pub fn parse_verdict(raw: &str) -> Option<VerdictDisposition> {
    match raw.trim().to_uppercase().as_str() {
        "APPROVE" => Some(VerdictDisposition::Approve),
        "REQUEST_CHANGES" | "REQUEST_CHANGE" => Some(VerdictDisposition::RequestChanges {
            reasons: vec![],
        }),
        "REJECT" => Some(VerdictDisposition::RequestChanges {
            reasons: vec!["Review rejected".into()],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_approve() {
        assert_eq!(parse_verdict("APPROVE"), Some(VerdictDisposition::Approve));
    }

    #[test]
    fn test_parse_request_changes() {
        assert!(matches!(
            parse_verdict("REQUEST_CHANGES"),
            Some(VerdictDisposition::RequestChanges { .. })
        ));
    }

    #[test]
    fn test_parse_unknown() {
        assert_eq!(parse_verdict("MAYBE"), None);
    }

    #[test]
    fn test_parse_case_insensitive() {
        assert_eq!(parse_verdict("approve"), Some(VerdictDisposition::Approve));
    }

    #[test]
    fn test_parse_whitespace() {
        assert_eq!(parse_verdict("  APPROVE  "), Some(VerdictDisposition::Approve));
    }

    // ── UFO grounding tests ────────────────────────────────────────────────
    // 🤓 ReviewVerdict is a Moment::Mode — intrinsic, dependent on review event.
    //    These tests verify the type-level invariants expected by the ontology.

    #[test]
    fn test_verdict_is_exhaustive() {
        // Every possible verdict string must parse to Some
        for v in [VERDICT_APPROVE, VERDICT_REQUEST_CHANGES, VERDICT_REJECT] {
            assert!(parse_verdict(v).is_some(), "verdict '{}' should parse", v);
        }
    }

    #[test]
    fn test_verdict_matches_cake_constraint() {
        // Must match CakeTicket CHECK constraint: IN ('APPROVE','REQUEST_CHANGES','REJECT',NULL)
        let valid = ["APPROVE", "REQUEST_CHANGES", "REJECT"];
        for v in valid {
            assert!(parse_verdict(v).is_some());
        }
    }
}
