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

/// Evaluate a constraint against a verdict from Rhai scripts.
/// Both parameters accept JSON strings or simple verdict-name strings.
///
/// # Rhai usage
/// ```rhai
/// let ok = evaluate_constraint(`"MustApprove"`, "APPROVE");
/// let nested = evaluate_constraint(`{"AllOf":["MustApprove",{"Not":"MustApprove"}]}`, "APPROVE");
/// let with_reasons = evaluate_constraint(
///     `{"MustRequestChanges":{"min_reasons":2}}`,
///     `{"RequestChanges":{"reasons":["bad code","missing test"]}}`
/// );
/// ```
pub fn evaluate_constraint_json(constraint_json: &str, verdict_str: &str) -> Result<bool, String> {
    let constraint: VerdictConstraint = serde_json::from_str(constraint_json)
        .map_err(|e| format!("Invalid constraint JSON: {}", e))?;

    let verdict = parse_verdict(verdict_str)
        .or_else(|| serde_json::from_str::<VerdictDisposition>(verdict_str).ok())
        .ok_or_else(|| {
            format!(
                "Unknown verdict: {}. Expected APPROVE or REQUEST_CHANGES",
                verdict_str
            )
        })?;

    Ok(constraint.evaluate(&verdict))
}

/// Emit a verdict with evidence — the full governance round-trip.
/// Creates a content-addressed evidence record from the verdict.
///
/// Returns a JSON object with { verdict, content_hash, evidence_node_hash, timestamp }.
///
/// # Rhai usage
/// ```rhai
/// let evidence = emit_verdict("APPROVE", "some content to hash");
/// // evidence.verdict == "APPROVE"
/// // evidence.content_hash == "sha256..."
/// ```
pub fn emit_verdict(verdict_str: &str, content: &str) -> Result<String, String> {
    let verdict = parse_verdict(verdict_str)
        .ok_or_else(|| format!("Unknown verdict: {}", verdict_str))?;

    let evidence = crate::reviewer::evidence::CommandEvidence::new(
        content.as_bytes(),
        &verdict,
        "rhai-emit-verdict",
    );
    let node = evidence.to_evidence_node();

    serde_json::to_string(&serde_json::json!({
        "verdict": evidence.verdict,
        "content_hash": evidence.content_hash,
        "evidence_node": node.commit_hash,
        "timestamp": evidence.timestamp,
    }))
    .map_err(|e| format!("serialize: {}", e))
}

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

    // ── evaluate_constraint_json tests ─────────────────────────────────────

    #[test]
    fn test_evaluate_constraint_must_approve() {
        assert!(evaluate_constraint_json(r#""MustApprove""#, "APPROVE").unwrap());
        assert!(!evaluate_constraint_json(r#""MustApprove""#, "REQUEST_CHANGES").unwrap());
    }

    #[test]
    fn test_evaluate_constraint_not() {
        let json = r#"{"Not":"MustApprove"}"#;
        assert!(evaluate_constraint_json(json, "REQUEST_CHANGES").unwrap());
        assert!(!evaluate_constraint_json(json, "APPROVE").unwrap());
    }

    #[test]
    fn test_evaluate_constraint_all_of() {
        // MustApprove AND NOT MustRequestChanges{min_reasons:1}
        let json = r#"{"AllOf":["MustApprove",{"Not":{"MustRequestChanges":{"min_reasons":1}}}]}"#;
        assert!(evaluate_constraint_json(json, "APPROVE").unwrap());
        assert!(!evaluate_constraint_json(json, "REQUEST_CHANGES").unwrap());
    }

    #[test]
    fn test_evaluate_constraint_with_reasons() {
        // MustRequestChanges with min_reasons=2 against a verdict with 2 reasons
        let constraint = r#"{"MustRequestChanges":{"min_reasons":2}}"#;
        let verdict = r#"{"RequestChanges":{"reasons":["bad code","missing test"]}}"#;
        assert!(evaluate_constraint_json(constraint, verdict).unwrap());
    }

    #[test]
    fn test_evaluate_constraint_with_reasons_insufficient() {
        let constraint = r#"{"MustRequestChanges":{"min_reasons":2}}"#;
        let verdict = r#"{"RequestChanges":{"reasons":["bad code"]}}"#;
        assert!(!evaluate_constraint_json(constraint, verdict).unwrap());
    }

    #[test]
    fn test_evaluate_constraint_has_reasons() {
        let constraint = r#"{"HasReasons":{"keywords":["security","perf"]}}"#;
        let verdict = r#"{"RequestChanges":{"reasons":["security vulnerability","perf regression"]}}"#;
        assert!(evaluate_constraint_json(constraint, verdict).unwrap());
    }

    #[test]
    fn test_evaluate_constraint_invalid_json() {
        assert!(evaluate_constraint_json("not json", "APPROVE").is_err());
    }

    #[test]
    fn test_evaluate_constraint_unknown_verdict() {
        assert!(evaluate_constraint_json(r#""MustApprove""#, "GARBAGE").is_err());
    }
}
