//! `Satisfies<T>` trait — type-level constraint evaluation pattern.
//!
//! Mirrors what `evidence.rs` persists (`record_satisfies`) but at the Rust
//! type level with deterministic results. Every domain type in the Tax-Lawyer
//! platform implements `Satisfies<C>` for the constraints it must meet, and
//! the `EvidenceBridge` helper auto-wires NS-9 (`record_is_a`) and NS-10
//! (`record_audited_by`) calls.
//!
//! # Usage
//! ```ignore
//! use ufo_types::satisfies::{Satisfies, SatisfiesResult, Disposition};
//!
//! let activity = AuRdActivity { /* ... */ };
//! let eligibility = AuRdEligibility::new(2025);
//! let result = activity.satisfies(&eligibility);
//! assert!(matches!(result.disposition, Disposition::Satisfied));
//! ```

use serde::{Deserialize, Serialize};

use crate::stereotype::UfoStereotype;

// ── Satisfies trait ──────────────────────────────────────────────────────────

/// Evaluate whether `Self` satisfies the given constraint `C`.
///
/// This is the core evaluation pattern for the Tax-Lawyer platform. Every
/// domain type implements this for the constraints it must meet (e.g.,
/// `AuRdActivity` satisfies `AuRdEligibility` under ITAA 1997 Div 355).
///
/// Implementors MUST produce deterministic results — the same inputs always
/// produce the same `SatisfiesResult`.
pub trait Satisfies<C> {
    /// Evaluate this entity against the constraint, returning a structured
    /// result with disposition, confidence, and evidence node IDs.
    fn satisfies(&self, constraint: &C) -> SatisfiesResult;
}

// ── Result types ─────────────────────────────────────────────────────────────

/// The outcome of a `Satisfies<C>` evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SatisfiesResult {
    /// Whether the constraint was satisfied, violated, or undetermined.
    pub disposition: Disposition,

    /// Confidence in the evaluation [0.0, 1.0].
    /// 1.0 = completely certain; 0.0 = purely speculative.
    pub confidence: f64,

    /// Blake3-hashed evidence node IDs from arc-kit-au.
    /// These form the audit trail for ATO/IRS defense.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_nodes: Vec<NodeId>,

    /// UFO stereotype for this evaluation result.
    /// Always `Mode` — satisfying a constraint is an intrinsic property
    /// of the entity, not a mediator between entities.
    #[serde(default = "default_ufo_category")]
    pub ufo_category: UfoStereotype,
}

fn default_ufo_category() -> UfoStereotype {
    UfoStereotype::Mode("SatisfiesResult".into())
}

impl SatisfiesResult {
    /// Create a satisfied result with the given confidence.
    pub fn satisfied(confidence: f64) -> Self {
        Self {
            disposition: Disposition::Satisfied,
            confidence,
            evidence_nodes: Vec::new(),
            ufo_category: UfoStereotype::Mode("SatisfiesResult".into()),
        }
    }

    /// Create a violated result with a reason and confidence.
    pub fn violated(reason: impl Into<String>, confidence: f64) -> Self {
        Self {
            disposition: Disposition::Violated {
                reason: reason.into(),
            },
            confidence,
            evidence_nodes: Vec::new(),
            ufo_category: UfoStereotype::Mode("SatisfiesResult".into()),
        }
    }

    /// Create an unknown result — evaluation could not be completed.
    pub fn unknown(confidence: f64) -> Self {
        Self {
            disposition: Disposition::Unknown,
            confidence,
            evidence_nodes: Vec::new(),
            ufo_category: UfoStereotype::Mode("SatisfiesResult".into()),
        }
    }

    /// Attach evidence node IDs (Blake3 hashes) for audit trail.
    pub fn with_evidence(mut self, nodes: Vec<NodeId>) -> Self {
        self.evidence_nodes = nodes;
        self
    }

    /// Returns true if the constraint is satisfied.
    pub fn is_satisfied(&self) -> bool {
        matches!(self.disposition, Disposition::Satisfied)
    }

    /// Returns true if the constraint is violated.
    pub fn is_violated(&self) -> bool {
        matches!(self.disposition, Disposition::Violated { .. })
    }
}

/// The disposition (outcome) of a constraint evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Disposition {
    /// Constraint is fully satisfied.
    Satisfied,

    /// Constraint is violated, with a human-readable reason.
    #[serde(rename = "violated")]
    Violated {
        /// Why the constraint was violated (e.g., "activity_type is not Core")
        reason: String,
    },

    /// Evaluation could not be completed — insufficient data, ambiguous inputs.
    Unknown,
}

impl std::fmt::Display for Disposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Disposition::Satisfied => write!(f, "Satisfied"),
            Disposition::Violated { reason } => write!(f, "Violated({reason})"),
            Disposition::Unknown => write!(f, "Unknown"),
        }
    }
}

// ── NodeId — Blake3 evidence node identifier ─────────────────────────────────

/// A Blake3 evidence node ID from the arc-kit-au evidence graph.
///
/// Wraps a hex-encoded Blake3 hash that uniquely identifies an evidence
/// node in the provenance chain.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct NodeId(pub String);

impl NodeId {
    /// Create a new NodeId from a hex Blake3 hash.
    pub fn new(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Return the hex hash string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeId({})", self.0)
    }
}

impl From<&str> for NodeId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for NodeId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

// ── Evidence bridge traits ───────────────────────────────────────────────────

use crate::stereotype::Stereotyped;

/// A domain constraint that can report which ISO standard(s) it embodies.
///
/// Implemented by constraint types (e.g., `AuRdEligibility`, `UsRdcFourPartTest`)
/// so that domain `Satisfies` impls can auto-call `record_audited_by()` (NS-10).
pub trait IsoAuditable {
    /// ISO standard identifier(s) that govern this constraint.
    ///
    /// Examples:
    /// - `"ISO 17442"` for LEI validation
    /// - `"ISO 4217"` for currency handling
    /// - `"ITAA 1997 Div 355"` for AU R&D eligibility
    fn iso_standard_ids(&self) -> Vec<String>;
}

/// A helper that bridges `Satisfies<T>` results with the evidence layer.
///
/// Domain types implementing both `Satisfies<C>` and `Stereotyped` can use
/// this to produce a `SatisfiesResult` while also recording:
/// - NS-9: `record_is_a(subject, stereotype)` for ontological provenance
/// - NS-10: `record_audited_by(subject, iso_standard)` for compliance
///
/// # Example
/// ```ignore
/// pub struct AuRdActivity { pub activity_id: String, /* ... */ }
///
/// impl Satisfies<AuRdEligibility> for AuRdActivity {
///     fn satisfies(&self, _c: &AuRdEligibility) -> SatisfiesResult {
///         SatisfiesResult::satisfied(0.95)
///     }
/// }
///
/// impl Stereotyped for AuRdActivity {
///     fn ufo_stereotype(&self) -> UfoStereotype {
///         UfoStereotype::SubKind {
///             name: "AuRdActivity".into(),
///             parent: "Activity".into(),
///         }
///     }
/// }
///
/// impl IsoAuditable for AuRdEligibility {
///     fn iso_standard_ids(&self) -> Vec<String> {
///         vec!["ITAA 1997 Div 355".into()]
///     }
/// }
///
/// // Bridge — record evidence:
/// let activity = AuRdActivity::new();
/// let constraint = AuRdEligibility::new(2025);
/// let result = EvidenceBridge::evaluate_and_record(
///     &activity,
///     &constraint,
///     activity.activity_id(),
/// );
/// // Internally calls:
/// //   1. activity.satisfies(&constraint)
/// //   2. record_is_a(activity_id, activity.ufo_stereotype().to_string())
/// //   3. record_audited_by(activity_id, constraint.iso_standard_ids())
/// ```
pub struct EvidenceBridge;

impl EvidenceBridge {
    /// Evaluate `satisfies()` and produce the stereotype + ISO labels needed
    /// for evidence recording, without actually calling the evidence layer.
    ///
    /// Returns `(SatisfiesResult, ufo_stereotype_label, iso_standard_ids)`
    /// so the caller can pass them to `record_is_a()` and `record_audited_by()`.
    pub fn evaluate<E, C>(
        entity: &E,
        constraint: &C,
    ) -> (SatisfiesResult, UfoStereotype, Vec<String>)
    where
        E: Satisfies<C> + Stereotyped,
        C: IsoAuditable,
    {
        let result = entity.satisfies(constraint);
        let stereotype = entity.ufo_stereotype();
        let iso_ids = constraint.iso_standard_ids();
        (result, stereotype, iso_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test domain types ─────────────────────────────────────────────────

    /// Example constraint: a company must have a valid LEI.
    #[derive(Debug)]
    struct LeiRequired;

    impl IsoAuditable for LeiRequired {
        fn iso_standard_ids(&self) -> Vec<String> {
            vec!["ISO 17442".into()]
        }
    }

    /// Example domain entity: a company.
    #[derive(Debug)]
    struct TestCompany {
        has_valid_lei: bool,
    }

    impl Satisfies<LeiRequired> for TestCompany {
        fn satisfies(&self, _c: &LeiRequired) -> SatisfiesResult {
            if self.has_valid_lei {
                SatisfiesResult::satisfied(0.99)
            } else {
                SatisfiesResult::violated("Missing or invalid LEI", 1.0)
            }
        }
    }

    impl Stereotyped for TestCompany {
        fn ufo_stereotype(&self) -> UfoStereotype {
            UfoStereotype::Kind("Company".into())
        }
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn satisfies_result_satisfied() {
        let r = SatisfiesResult::satisfied(0.95);
        assert!(r.is_satisfied());
        assert!(!r.is_violated());
        assert_eq!(r.confidence, 0.95);
    }

    #[test]
    fn satisfies_result_violated() {
        let r = SatisfiesResult::violated("no LEI", 1.0);
        assert!(!r.is_satisfied());
        assert!(r.is_violated());
        match &r.disposition {
            Disposition::Violated { reason } => assert_eq!(reason, "no LEI"),
            _ => panic!("expected Violated"),
        }
    }

    #[test]
    fn satisfies_result_unknown() {
        let r = SatisfiesResult::unknown(0.3);
        assert!(!r.is_satisfied());
        assert!(!r.is_violated());
        assert_eq!(r.confidence, 0.3);
    }

    #[test]
    fn satisfies_result_with_evidence() {
        let nodes = vec![NodeId::new("abc123"), NodeId::new("def456")];
        let r = SatisfiesResult::satisfied(0.8).with_evidence(nodes.clone());
        assert_eq!(r.evidence_nodes, nodes);
    }

    #[test]
    fn disposition_display() {
        assert_eq!(Disposition::Satisfied.to_string(), "Satisfied");
        assert_eq!(
            Disposition::Violated {
                reason: "no LEI".into()
            }
            .to_string(),
            "Violated(no LEI)"
        );
        assert_eq!(Disposition::Unknown.to_string(), "Unknown");
    }

    #[test]
    fn satisfies_result_roundtrips_json() {
        let r = SatisfiesResult {
            disposition: Disposition::Satisfied,
            confidence: 0.87,
            evidence_nodes: vec![NodeId::new("hash1")],
            ufo_category: UfoStereotype::Mode("TestResult".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: SatisfiesResult = serde_json::from_str(&json).unwrap();
        assert_eq!(r.disposition, back.disposition);
        assert_eq!(r.confidence, back.confidence);
        assert_eq!(r.evidence_nodes, back.evidence_nodes);
    }

    #[test]
    fn trait_satisfies_compiles_and_works() {
        let company = TestCompany {
            has_valid_lei: true,
        };
        let result = company.satisfies(&LeiRequired);
        assert!(result.is_satisfied());
    }

    #[test]
    fn trait_satisfies_violated_works() {
        let company = TestCompany {
            has_valid_lei: false,
        };
        let result = company.satisfies(&LeiRequired);
        assert!(result.is_violated());
    }

    #[test]
    fn evidence_bridge_produces_labels() {
        let company = TestCompany {
            has_valid_lei: true,
        };
        let constraint = LeiRequired;
        let (result, stereotype, iso_ids) = EvidenceBridge::evaluate(&company, &constraint);
        assert!(result.is_satisfied());
        assert_eq!(stereotype.to_string(), "Kind:Company");
        assert_eq!(iso_ids, vec!["ISO 17442"]);
    }

    #[test]
    fn node_id_roundtrips() {
        let n = NodeId::new("abc123def456");
        let json = serde_json::to_string(&n).unwrap();
        let back: NodeId = serde_json::from_str(&json).unwrap();
        assert_eq!(n, back);
        assert_eq!(n.as_str(), "abc123def456");
    }

    #[test]
    fn node_id_display() {
        let n = NodeId::new("hash1");
        assert_eq!(n.to_string(), "NodeId(hash1)");
    }

    #[test]
    fn node_id_from_str() {
        let n: NodeId = "test".into();
        assert_eq!(n.as_str(), "test");
    }
}
