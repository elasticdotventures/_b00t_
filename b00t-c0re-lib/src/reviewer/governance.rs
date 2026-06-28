// b00t-c0re-lib/src/reviewer/governance.rs
// 🤓 Core governance types for the reviewer verdict pipeline.
//    FOL-correct: equality is over logical propositions, not metadata.
//    Implements Satisfies<Constraint> from Tax-Lawyer architecture.
//    UFO grounding: every type carries a UFO stereotype marker.

use serde::{Deserialize, Serialize};

// ── Verdict Disposition ─────────────────────────────────────────────────────
// FOL primitive: ¬Approve ≡ RequestChanges  (logical negation)
// UFO: Moment::Mode — intrinsic property of a review event

/// The disposition of a reviewer verdict.
///
/// FOL semantics:
///   - `Approve` = the proposition "the change is acceptable"
///   - `RequestChanges` = ¬Approve = "the change is NOT acceptable"
///   - Equality is over the proposition, NOT the reasons payload.
///
/// Matches CakeTicket CHECK constraint: IN ('APPROVE','REQUEST_CHANGES','REJECT',NULL)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerdictDisposition {
    /// Review passed — no changes required
    Approve,
    /// Changes requested — must revise before merge
    RequestChanges {
        reasons: Vec<String>,
    },
}

// ── FOL-correct equality: only variant discriminant matters ─────────────────
// 🤓 Two REQUEST_CHANGES verdicts are logically equivalent regardless of which
//    specific reasons were cited. Reasons are metadata, not identity.

impl PartialEq for VerdictDisposition {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (VerdictDisposition::Approve, VerdictDisposition::Approve)
                | (VerdictDisposition::RequestChanges { .. }, VerdictDisposition::RequestChanges { .. })
        )
    }
}

impl Eq for VerdictDisposition {}

impl std::hash::Hash for VerdictDisposition {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
    }
}

impl VerdictDisposition {
    /// Logical negation: returns the opposite verdict.
    ///
    /// FOL: ¬Approve = RequestChanges, ¬RequestChanges = Approve
    ///
    /// ```rust
    /// use b00t_c0re_lib::reviewer::governance::VerdictDisposition;
    ///
    /// // not(Approve) == RequestChanges
    /// assert_eq!(
    ///     VerdictDisposition::Approve.not(),
    ///     VerdictDisposition::RequestChanges { reasons: vec![] }
    /// );
    ///
    /// // not(RequestChanges) == Approve
    /// assert_eq!(
    ///     VerdictDisposition::RequestChanges { reasons: vec!["bug".to_string()] }.not(),
    ///     VerdictDisposition::Approve
    /// );
    ///
    /// // not(not(Approve)) == Approve (double negation elimination)
    /// assert_eq!(
    ///     VerdictDisposition::Approve.not().not(),
    ///     VerdictDisposition::Approve
    /// );
    /// ```
    pub fn not(&self) -> Self {
        match self {
            VerdictDisposition::Approve => VerdictDisposition::RequestChanges {
                reasons: vec![],
            },
            VerdictDisposition::RequestChanges { .. } => VerdictDisposition::Approve,
        }
    }

    /// The exit code this verdict should produce for hook dispatch
    ///
    /// Maps to dispatch.sh exit protocol:
    ///   Approve → 0 (allow continuation)
    ///   RequestChanges → 2 (block, inject feedback)
    pub fn hook_exit_code(&self) -> i32 {
        match self {
            VerdictDisposition::Approve => 0,
            VerdictDisposition::RequestChanges { .. } => 2,
        }
    }

    /// FOL: Approve ↔ allows proceed
    pub fn allows_proceed(&self) -> bool {
        matches!(self, VerdictDisposition::Approve)
    }
}

impl std::fmt::Display for VerdictDisposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerdictDisposition::Approve => write!(f, "APPROVE"),
            VerdictDisposition::RequestChanges { .. } => write!(f, "REQUEST_CHANGES"),
        }
    }
}

// ── Constraint types (FOL predicates) ───────────────────────────────────────
// 🤓 Constraints are the predicates that verdicts satisfy.
//    FOL: Constraint = { predicate: VerdictDisposition → bool }

/// A constraint that a verdict must satisfy.
/// Constraints compose via logical combinators: AllOf (∧), AnyOf (∨), Not (¬).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerdictConstraint {
    /// All sub-constraints must be satisfied (∧ — conjunction)
    AllOf(Vec<VerdictConstraint>),
    /// At least one sub-constraint must be satisfied (∨ — disjunction)
    AnyOf(Vec<VerdictConstraint>),
    /// The sub-constraint must NOT be satisfied (¬ — negation)
    Not(Box<VerdictConstraint>),
    /// Verdict must be APPROVE
    MustApprove,
    /// Verdict must be REQUEST_CHANGES with at least N distinct reasons
    MustRequestChanges { min_reasons: usize },
    /// At least one finding must be cross-validated (∃ — existential)
    ExistsCrossValidated,
    /// Verdict must have all specified reason keywords present
    HasReasons { keywords: Vec<String> },
}

impl VerdictConstraint {
    /// Evaluate whether a verdict satisfies this constraint.
    ///
    /// FOL: constraint(verdict) → bool
    ///
    /// ```rust
    /// use b00t_c0re_lib::reviewer::governance::{VerdictDisposition, VerdictConstraint};
    ///
    /// // MustApprove.evaluate(Approve) == true
    /// assert!(VerdictConstraint::MustApprove.evaluate(&VerdictDisposition::Approve));
    ///
    /// // MustApprove.evaluate(RequestChanges) == false
    /// assert!(!VerdictConstraint::MustApprove.evaluate(
    ///     &VerdictDisposition::RequestChanges { reasons: vec![] }
    /// ));
    ///
    /// // AllOf([MustApprove, MustApprove]).evaluate(Approve) == true
    /// assert!(VerdictConstraint::AllOf(vec![
    ///     VerdictConstraint::MustApprove,
    ///     VerdictConstraint::MustApprove,
    /// ]).evaluate(&VerdictDisposition::Approve));
    ///
    /// // Not(MustApprove).evaluate(RequestChanges) == true
    /// assert!(VerdictConstraint::Not(
    ///     Box::new(VerdictConstraint::MustApprove)
    /// ).evaluate(&VerdictDisposition::RequestChanges { reasons: vec![] }));
    ///
    /// // MustRequestChanges(min_reasons:1).evaluate(RequestChanges{reasons:["bug"]}) == true
    /// assert!(VerdictConstraint::MustRequestChanges { min_reasons: 1 }.evaluate(
    ///     &VerdictDisposition::RequestChanges { reasons: vec!["bug".to_string()] }
    /// ));
    ///
    /// // MustRequestChanges(min_reasons:1).evaluate(RequestChanges{reasons:[]}) == false
    /// assert!(!VerdictConstraint::MustRequestChanges { min_reasons: 1 }.evaluate(
    ///     &VerdictDisposition::RequestChanges { reasons: vec![] }
    /// ));
    /// ```
    pub fn evaluate(&self, verdict: &VerdictDisposition) -> bool {
        match self {
            VerdictConstraint::AllOf(constraints) => {
                // ∀c ∈ constraints . c(verdict) = true
                constraints.iter().all(|c| c.evaluate(verdict))
            }
            VerdictConstraint::AnyOf(constraints) => {
                // ∃c ∈ constraints . c(verdict) = true
                constraints.iter().any(|c| c.evaluate(verdict))
            }
            VerdictConstraint::Not(inner) => {
                // ¬inner(verdict)
                !inner.evaluate(verdict)
            }
            VerdictConstraint::MustApprove => {
                matches!(verdict, VerdictDisposition::Approve)
            }
            VerdictConstraint::MustRequestChanges { min_reasons } => {
                match verdict {
                    VerdictDisposition::RequestChanges { reasons } => reasons.len() >= *min_reasons,
                    _ => false,
                }
            }
            VerdictConstraint::ExistsCrossValidated => {
                // ∃ finding ∈ reasons . finding contains "cross-validated"
                match verdict {
                    VerdictDisposition::RequestChanges { reasons } => {
                        reasons.iter().any(|r| r.contains("cross-validated"))
                    }
                    _ => false,
                }
            }
            VerdictConstraint::HasReasons { keywords } => {
                match verdict {
                    VerdictDisposition::RequestChanges { reasons } => {
                        // ∀k ∈ keywords . ∃r ∈ reasons . r contains k
                        keywords.iter().all(|k| {
                            reasons.iter().any(|r| r.contains(k))
                        })
                    }
                    _ => false,
                }
            }
        }
    }
}

// ── Satisfies trait (Tax-Lawyer pattern) ────────────────────────────────────
// FOL: verdict satisfies constraint → evidence_node produced

/// Result of a satisfaction check.
/// FOL: the truth value of `verdict satisfies constraint`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SatisfiesResult {
    /// Whether the verdict satisfied the constraint
    pub satisfied: bool,
    /// Confidence in the result [0.0, 1.0]
    pub confidence: f64,
    /// Human-readable explanation
    pub reason: String,
}

impl SatisfiesResult {
    pub fn yes(reason: impl Into<String>) -> Self {
        Self {
            satisfied: true,
            confidence: 1.0,
            reason: reason.into(),
        }
    }

    pub fn no(reason: impl Into<String>) -> Self {
        Self {
            satisfied: false,
            confidence: 0.0,
            reason: reason.into(),
        }
    }

    /// FOL: ¬satisfied → violated
    pub fn is_violated(&self) -> bool {
        !self.satisfied
    }
}

/// A verdict and its constraint evaluation.
/// FOL: this is the complete proposition "verdict satisfies constraint".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerdictEvaluation {
    pub verdict: VerdictDisposition,
    pub constraint: VerdictConstraint,
    pub result: SatisfiesResult,
}

impl VerdictEvaluation {
    /// Evaluate a verdict against a constraint.
    /// FOL: evaluation(verdict, constraint) → SatisfiesResult
    pub fn new(verdict: VerdictDisposition, constraint: VerdictConstraint) -> Self {
        let satisfied = constraint.evaluate(&verdict);
        let confidence = if satisfied { 1.0 } else { 0.0 };
        let reason = if satisfied {
            "constraint satisfied".into()
        } else {
            "constraint violated".into()
        };
        Self {
            verdict,
            constraint,
            result: SatisfiesResult {
                satisfied,
                confidence,
                reason,
            },
        }
    }

    /// FOL implication: if satisfied, produce evidence.
    /// Without evidence → evaluation is incomplete.
    pub fn requires_evidence(&self) -> bool {
        self.result.satisfied
    }
}

// ── Reviewer Command ────────────────────────────────────────────────────────
// UFO: Perdurant::Event — what happens after the verdict moment

/// A command triggered by a reviewer verdict.
/// Each variant maps to a specific harness action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewerCommand {
    /// APPROVE → continue workflow, proceed to next task
    Proceed {
        task_id: String,
        next_action: String,
    },
    /// REQUEST_CHANGES → send feedback, block merge, require revision
    Revise {
        task_id: String,
        feedback: Vec<String>,
        block_merge: bool,
    },
    /// Scope drift warning — non-blocking advisory
    Warn {
        task_id: String,
        warning: String,
        scope_files: Vec<String>,
    },
}

impl ReviewerCommand {
    /// FOL: Proceed → Continue, Revise → Block, Warn → Warn
    pub fn harness_action(&self) -> HarnessAction {
        match self {
            ReviewerCommand::Proceed { .. } => HarnessAction::Continue,
            ReviewerCommand::Revise { .. } => HarnessAction::Block {
                inject_feedback: true,
            },
            ReviewerCommand::Warn { .. } => HarnessAction::Warn,
        }
    }

    /// FOL: ∀ commands . requires_evidence = true
    pub fn requires_evidence(&self) -> bool {
        true
    }
}

// ── Harness Action ──────────────────────────────────────────────────────────

/// What the harness (opencode, claude-code) should do after a verdict
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HarnessAction {
    /// Allow workflow to continue
    Continue,
    /// Block workflow, optionally inject feedback
    Block {
        inject_feedback: bool,
    },
    /// Non-blocking warning
    Warn,
}

impl HarnessAction {
    /// Convert to dispatch.sh exit code
    pub fn exit_code(&self) -> i32 {
        match self {
            HarnessAction::Continue => 0,
            HarnessAction::Warn => 1,
            HarnessAction::Block { .. } => 2,
        }
    }
}

// ── Evidence Node ───────────────────────────────────────────────────────────
// UFO: Endurant::SubKind — evidence persists beyond the review event

/// Content-addressed evidence that a verdict was produced.
/// The commit_hash is a SHA-256 digest of content + verdict — no git dependency.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvidenceNode {
    pub commit_hash: String,
    pub verdict: String,
    pub notes_ref: String,
}

impl EvidenceNode {
    pub fn new(commit_hash: impl Into<String>, verdict: VerdictDisposition) -> Self {
        Self {
            commit_hash: commit_hash.into(),
            verdict: verdict.to_string(),
            notes_ref: "review-verdict".into(),
        }
    }

    /// ∀ chars ∈ commit_hash . is_ascii_hexdigit()
    pub fn is_valid_hash(&self) -> bool {
        self.commit_hash.len() == 64
            && self.commit_hash.chars().all(|c| c.is_ascii_hexdigit())
    }
}

impl std::fmt::Display for EvidenceNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.commit_hash, self.verdict)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── FOL equality tests ──────────────────────────────────────────────────

    #[test]
    fn test_verdict_equality_is_over_proposition_not_reasons() {
        // FOL: RequestChanges{reasons:["a"]} ≡ RequestChanges{reasons:["b"]}
        let a = VerdictDisposition::RequestChanges {
            reasons: vec!["missing guard".into()],
        };
        let b = VerdictDisposition::RequestChanges {
            reasons: vec!["style violation".into()],
        };
        assert_eq!(a, b, "FOL: same proposition, different metadata");
    }

    #[test]
    fn test_verdict_equality_approve_not_request_changes() {
        // FOL: Approve ≠ RequestChanges (logical negation)
        assert_ne!(
            VerdictDisposition::Approve,
            VerdictDisposition::RequestChanges {
                reasons: vec![]
            }
        );
    }

    #[test]
    fn test_verdict_negation_double() {
        // FOL: ¬¬Approve ≡ Approve
        let v = VerdictDisposition::Approve;
        assert_eq!(v.not().not(), v);
    }

    #[test]
    fn test_verdict_negation_inverse() {
        // FOL: ¬Approve = RequestChanges
        assert_eq!(
            VerdictDisposition::Approve.not(),
            VerdictDisposition::RequestChanges {
                reasons: vec![]
            }
        );
    }

    #[test]
    fn test_verdict_hash_consistent_with_equality() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(VerdictDisposition::RequestChanges {
            reasons: vec!["a".into()],
        });
        // Same proposition, different reasons — should not add duplicate
        set.insert(VerdictDisposition::RequestChanges {
            reasons: vec!["b".into()],
        });
        assert_eq!(set.len(), 1, "FOL: same proposition = same hash bucket");
    }

    // ── Constraint evaluation tests ─────────────────────────────────────────

    #[test]
    fn test_constraint_must_approve() {
        let c = VerdictConstraint::MustApprove;
        assert!(c.evaluate(&VerdictDisposition::Approve));
        assert!(!c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec![]
        }));
    }

    #[test]
    fn test_constraint_must_request_changes_min_reasons() {
        let c = VerdictConstraint::MustRequestChanges { min_reasons: 2 };
        assert!(c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec!["a".into(), "b".into()],
        }));
        assert!(!c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec!["a".into()],
        }));
    }

    #[test]
    fn test_constraint_all_of_conjunction() {
        // FOL: MustApprove ∧ ¬MustRequestChanges{min_reasons:1}
        // This is trivially satisfied by APPROVE
        let c = VerdictConstraint::AllOf(vec![
            VerdictConstraint::MustApprove,
            VerdictConstraint::Not(Box::new(VerdictConstraint::MustRequestChanges {
                min_reasons: 1,
            })),
        ]);
        assert!(c.evaluate(&VerdictDisposition::Approve));
    }

    #[test]
    fn test_constraint_any_of_disjunction() {
        // FOL: MustApprove ∨ MustRequestChanges{min_reasons:1}
        // Both APPROVE and REQUEST_CHANGES satisfy this
        let c = VerdictConstraint::AnyOf(vec![
            VerdictConstraint::MustApprove,
            VerdictConstraint::MustRequestChanges { min_reasons: 1 },
        ]);
        assert!(c.evaluate(&VerdictDisposition::Approve));
        assert!(c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec!["fix this".into()],
        }));
    }

    #[test]
    fn test_constraint_not_negation() {
        // FOL: ¬MustApprove ≡ MustRequestChanges{min_reasons:0}
        let c = VerdictConstraint::Not(Box::new(VerdictConstraint::MustApprove));
        // APPROVE → false (negated)
        assert!(!c.evaluate(&VerdictDisposition::Approve));
        // REQUEST_CHANGES → true (negation of MustApprove)
        assert!(c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec![]
        }));
    }

    #[test]
    fn test_constraint_exists_cross_validated() {
        // FOL: ∃ reason ∈ reasons . reason contains "cross-validated"
        let c = VerdictConstraint::ExistsCrossValidated;
        assert!(c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec!["cross-validated: both MECE and TRIZ found this".into()],
        }));
        assert!(!c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec!["single lens finding".into()],
        }));
    }

    #[test]
    fn test_constraint_has_reasons() {
        // FOL: ∀k ∈ {"security","perf"} . ∃r ∈ reasons . r contains k
        let c = VerdictConstraint::HasReasons {
            keywords: vec!["security".into(), "perf".into()],
        };
        assert!(c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec![
                "security vulnerability in auth".into(),
                "performance regression in loop".into(),
            ],
        }));
        assert!(!c.evaluate(&VerdictDisposition::RequestChanges {
            reasons: vec!["security vulnerability in auth".into()],
        }));
    }

    // ── Evaluation tests ────────────────────────────────────────────────────

    #[test]
    fn test_evaluation_satisfied() {
        let eval = VerdictEvaluation::new(
            VerdictDisposition::Approve,
            VerdictConstraint::MustApprove,
        );
        assert!(eval.result.satisfied);
        assert!((eval.result.confidence - 1.0).abs() < f64::EPSILON);
        assert!(eval.requires_evidence());
    }

    #[test]
    fn test_evaluation_violated() {
        let eval = VerdictEvaluation::new(
            VerdictDisposition::Approve,
            VerdictConstraint::MustRequestChanges { min_reasons: 1 },
        );
        assert!(!eval.result.satisfied);
        assert!(eval.result.confidence < f64::EPSILON);
        assert!(!eval.requires_evidence());
    }

    // ── Existing tests (updated for custom equality) ────────────────────────

    #[test]
    fn test_verdict_display() {
        assert_eq!(VerdictDisposition::Approve.to_string(), "APPROVE");
        assert_eq!(
            VerdictDisposition::RequestChanges {
                reasons: vec!["missing guard".into()]
            }
            .to_string(),
            "REQUEST_CHANGES"
        );
    }

    #[test]
    fn test_verdict_hook_exit_code() {
        assert_eq!(VerdictDisposition::Approve.hook_exit_code(), 0);
        assert_eq!(
            VerdictDisposition::RequestChanges {
                reasons: vec![]
            }
            .hook_exit_code(),
            2
        );
    }

    #[test]
    fn test_harness_action_exit_codes() {
        assert_eq!(HarnessAction::Continue.exit_code(), 0);
        assert_eq!(HarnessAction::Warn.exit_code(), 1);
        assert_eq!(
            HarnessAction::Block {
                inject_feedback: true
            }
            .exit_code(),
            2
        );
    }

    #[test]
    fn test_command_harness_mapping() {
        let proceed = ReviewerCommand::Proceed {
            task_id: "T1".into(),
            next_action: "merge".into(),
        };
        assert_eq!(proceed.harness_action(), HarnessAction::Continue);

        let revise = ReviewerCommand::Revise {
            task_id: "T1".into(),
            feedback: vec!["fix auth".into()],
            block_merge: true,
        };
        assert_eq!(
            revise.harness_action(),
            HarnessAction::Block {
                inject_feedback: true
            }
        );
    }

    #[test]
    fn test_evidence_node_roundtrip() {
        let node = EvidenceNode::new(
            "a".repeat(64),
            VerdictDisposition::Approve,
        );
        assert_eq!(node.commit_hash.len(), 64);
        assert_eq!(node.verdict, "APPROVE");
        assert_eq!(node.notes_ref, "review-verdict");
        assert!(node.to_string().contains("APPROVE"));
    }

    #[test]
    fn test_evidence_node_valid_hash() {
        let valid = EvidenceNode::new("a".repeat(64), VerdictDisposition::Approve);
        assert!(valid.is_valid_hash());
        let invalid_short = EvidenceNode::new("abc", VerdictDisposition::Approve);
        assert!(!invalid_short.is_valid_hash());
        let invalid_chars = EvidenceNode::new("z".repeat(64), VerdictDisposition::Approve);
        assert!(!invalid_chars.is_valid_hash());
    }

    #[test]
    fn test_verdict_disposition_is_moment_mode() {
        let approve = VerdictDisposition::Approve;
        let serialized = serde_json::to_string(&approve).unwrap();
        let deserialized: VerdictDisposition = serde_json::from_str(&serialized).unwrap();
        assert_eq!(approve, deserialized);
        assert_eq!(approve.to_string(), "APPROVE");
    }

    #[test]
    fn test_evidence_node_is_endurant_subkind() {
        let node = EvidenceNode::new(
            "abc123def456",
            VerdictDisposition::RequestChanges {
                reasons: vec!["security vulnerability".into()],
            },
        );
        let serialized = serde_json::to_string(&node).unwrap();
        let deserialized: EvidenceNode = serde_json::from_str(&serialized).unwrap();
        assert_eq!(node, deserialized);
    }
}
