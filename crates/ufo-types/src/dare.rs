//! DARED — Decision, Alternative, Risk, ExecutiveDecision types.
//!
//! DARED is an OODA state-change proposal framework codified as Rust generics.
//! Each DARED section maps to a UFO stereotype so the types appear in b00t's
//! first-order logic (ontology export, evidence graph, gate validation).
//!
//! # DARED lifecycle in OODA phases
//!
//! ```text
//! Observe ──→ Orient ──→ Decide ──→ Act ──→ Verify
//!   │            │          │         │         │
//!   │         Alternatives  │   ExecutiveDecision │
//!   │         researched    │     artifacts       │
//!   │                       │    produced          │
//!   │                    Decision                  │
//!   │                    committed           Acceptance
//!   │                                       criteria
//!   │                    Risks               checked
//!   │                    assessed
//! ```
//!
//! # UFO grounding
//!
//! | Section             | UFO Stereotype | Category  | Rationale                        |
//! |--------------------|---------------|-----------|----------------------------------|
//! | Decision           | Kind          | Endurant  | A committed course of action     |
//! | Alternative        | Kind          | Endurant  | A possible but rejected path     |
//! | Risk               | Mode          | Moment    | Potential quality of the decision|
//! | ExecutiveDecision  | Relator       | Moment    | Captain's sign-off mediating     |
//! |                    |               |           | Decision→Artifacts relationship  |
//! | DARED proposal     | Relator       | Moment    | Mediates all four sections       |
//!
//! # b00t ontology integration
//!
//! All DARED types implement `Stereotyped` (NS-9 bridge), are `Serialize +
//! Deserialize` (TOML ↔ JSONL evidence), and can participate in
//! `Satisfies<DaredAcceptanceCriteria>` constraint checks.

use serde::{Deserialize, Serialize};

use crate::stereotype::{Stereotyped, UfoStereotype};
use crate::satisfies::{IsoAuditable, Satisfies, SatisfiesResult};

// ══════════════════════════════════════════════════════════════════════════════
// OODA Phase — state machine
// ══════════════════════════════════════════════════════════════════════════════

/// OODA loop phase. A DARE proposal progresses through these states.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OodaPhase {
    /// Problem identified; scope unclear.
    Observe,
    /// Alternatives researched; tradeoffs mapped.
    Orient,
    /// Decision committed; risks assessed; proposal drafted.
    Decide,
    /// Endurant artifacts produced; migration executing.
    Act,
    /// Acceptance criteria checked against artifacts.
    Verify,
}

impl OodaPhase {
    /// Next phase in the DARE lifecycle.
    pub fn next(self) -> Option<OodaPhase> {
        match self {
            OodaPhase::Observe => Some(OodaPhase::Orient),
            OodaPhase::Orient => Some(OodaPhase::Decide),
            OodaPhase::Decide => Some(OodaPhase::Act),
            OodaPhase::Act => Some(OodaPhase::Verify),
            OodaPhase::Verify => None,
        }
    }

    /// Returns true if this phase precedes `other`.
    pub fn precedes(self, other: OodaPhase) -> bool {
        let mut current = Some(self);
        while let Some(phase) = current {
            if phase == other {
                return true;
            }
            current = phase.next();
        }
        false
    }
}

impl std::fmt::Display for OodaPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OodaPhase::Observe => write!(f, "Observe"),
            OodaPhase::Orient => write!(f, "Orient"),
            OodaPhase::Decide => write!(f, "Decide"),
            OodaPhase::Act => write!(f, "Act"),
            OodaPhase::Verify => write!(f, "Verify"),
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Decision — Perdurant (Kind → Endurant)
// ══════════════════════════════════════════════════════════════════════════════

/// The committed course of action. A Decision is a process/event that
/// unfolds over time — implementation, migration, adoption.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Decision {
    /// What we are committing to do.
    pub what: String,
    /// Who owns and executes this decision.
    pub who: String,
    /// When this decision takes effect.
    pub when: String,
    /// Bounded scope — what this decision covers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scope: Vec<String>,
    /// Explicitly excluded from scope.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub explicitly_out_of_scope: Vec<String>,
}

impl Stereotyped for Decision {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Decision".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Alternative — Abstract (Kind → Endurant, counterfactual)
// ══════════════════════════════════════════════════════════════════════════════

/// An alternative that was considered but rejected. Alternatives are
/// counterfactuals — they never materialized but shaped the decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Alternative {
    /// Name of this alternative.
    pub name: String,
    /// Why this alternative was a viable option.
    pub viable: String,
    /// The specific reason this alternative was NOT chosen.
    pub rejected_because: String,
}

impl Stereotyped for Alternative {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Alternative".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Risk — Moment (Mode → intrinsic quality of the decision)
// ══════════════════════════════════════════════════════════════════════════════

/// Severity classification for risks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskSeverity {
    /// Informational — no action required.
    Low,
    /// Should be monitored; mitigation optional.
    Medium,
    /// Active mitigation required.
    High,
    /// Blocker — must be resolved before proceeding.
    Critical,
}

impl std::fmt::Display for RiskSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskSeverity::Low => write!(f, "LOW"),
            RiskSeverity::Medium => write!(f, "MEDIUM"),
            RiskSeverity::High => write!(f, "HIGH"),
            RiskSeverity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A risk — a potential quality that inheres in the decision but has
/// not yet manifested. Each risk MUST have a mitigation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Risk {
    /// Name of this risk.
    pub name: String,
    /// Severity classification.
    pub severity: RiskSeverity,
    /// Description of what could go wrong.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// How this risk is mitigated.
    pub mitigation: String,
}

impl Stereotyped for Risk {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Mode("Risk".into())
    }
}

impl IsoAuditable for Risk {
    fn iso_standard_ids(&self) -> Vec<String> {
        vec!["ISO 31000:2018".into()]
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// ExecutiveDecision — Relator (Moment → mediates Decision→Artifacts)
// ══════════════════════════════════════════════════════════════════════════════

/// The captain's sign-off — commits the decision to artifacts. The
/// `summary` field is the set/category-theoretic label that classifies
/// this decision in b00t's ontology (e.g., "SubmoduleElimination ⊆
/// VendorGovernance ⊆ ArchitectureDecision").
///
/// An ExecutiveDecision mediates between the Decision (what to do) and
/// the persistent artifacts (what remains). It is a Relator in UFO terms:
/// existentially dependent on both the Decision and the Artifacts it connects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutiveDecision {
    /// Set/category-theoretic summary — classifies this decision in b00t's
    /// ontology. Form: "LeafCategory ⊆ ParentCategory ⊆ RootCategory".
    /// This is the semantic sugar that enables first-order logic queries
    /// like "all ArchitectureDecisions ∨ all VendorGovernance decisions".
    pub summary: String,

    /// Persistent artifacts that will exist after adoption.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,

    /// Artifacts that will be removed from the codebase.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<String>,

    /// Criteria that must pass for the DARED to be accepted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<String>,
}

impl Stereotyped for ExecutiveDecision {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Relator("ExecutiveDecision".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// DaredDocument trait — the core DARED contract
// ══════════════════════════════════════════════════════════════════════════════

/// Error produced when a DARED document fails validation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
pub enum DaredValidationError {
    /// A required section is empty or missing.
    #[error("missing section: {0}")]
    MissingSection(String),

    /// No alternatives were considered (must have at least one).
    #[error("no alternatives listed — DARED requires at least one Alternative with a rejection reason")]
    NoAlternatives,

    /// A risk has no mitigation.
    #[error("risk '{risk}' has no mitigation — every Risk requires a mitigation")]
    UnmitigatedRisk { risk: String },

    /// No executive decision artifacts declared.
    #[error("no executive decision artifacts declared — DARED requires at least one persistent artifact")]
    NoExecutiveDecisionArtifacts,

    /// An acceptance criterion is missing or empty.
    #[error("empty acceptance criterion at index {index}")]
    EmptyAcceptanceCriterion { index: usize },

    /// Summary field is empty (set/category-theoretic label required).
    #[error("executive decision summary is empty — DARED requires a set/category-theoretic summary")]
    EmptySummary,
}

/// The core DARED contract. Every DARED proposal MUST implement this trait.
///
/// Implementors are Rust types that can be serialized to TOML (for storage
/// in `_b00t_/datums/`), deserialized back, validated, and wired into
/// the b00t evidence graph via `Stereotyped` + `Satisfies<DaredAcceptanceCriteria>`.
pub trait DaredDocument: Stereotyped {
    /// The decision being proposed.
    fn decision(&self) -> &Decision;

    /// Alternatives considered and rejected.
    fn alternatives(&self) -> &[Alternative];

    /// Risks identified with mitigations.
    fn risks(&self) -> &[Risk];

    /// Executive decision with artifacts, removed items, and acceptance criteria.
    fn executive_decision(&self) -> &ExecutiveDecision;

    /// The proposal identifier (e.g., "DARED-001").
    fn proposal_id(&self) -> &str;

    /// Current OODA phase of this proposal.
    fn phase(&self) -> OodaPhase;

    /// Validate that the DARED document is structurally complete.
    ///
    /// Checks:
    /// 1. Decision.what is non-empty
    /// 2. At least one Alternative with a rejection reason
    /// 3. Every Risk has a non-empty mitigation
    /// 4. ExecutiveDecision.summary is non-empty (set/category label)
    /// 5. At least one artifact or removed item
    /// 6. Every acceptance criterion is non-empty
    fn validate(&self) -> Result<(), Vec<DaredValidationError>> {
        let mut errors = Vec::new();

        if self.decision().what.is_empty() {
            errors.push(DaredValidationError::MissingSection(
                "Decision.what".into(),
            ));
        }

        if self.alternatives().is_empty() {
            errors.push(DaredValidationError::NoAlternatives);
        }

        for risk in self.risks() {
            if risk.mitigation.is_empty() {
                errors.push(DaredValidationError::UnmitigatedRisk {
                    risk: risk.name.clone(),
                });
            }
        }

        if self.executive_decision().summary.is_empty() {
            errors.push(DaredValidationError::EmptySummary);
        }

        if self.executive_decision().artifacts.is_empty()
            && self.executive_decision().removed.is_empty()
        {
            errors.push(DaredValidationError::NoExecutiveDecisionArtifacts);
        }

        for (i, criterion) in self
            .executive_decision()
            .acceptance_criteria
            .iter()
            .enumerate()
        {
            if criterion.trim().is_empty() {
                errors.push(DaredValidationError::EmptyAcceptanceCriterion { index: i });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Returns true if the DARED is ready to move from Decide → Act.
    fn is_ready_to_act(&self) -> bool {
        self.phase() >= OodaPhase::Decide && self.validate().is_ok()
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// DaredProposal — concrete DARED document
// ══════════════════════════════════════════════════════════════════════════════

/// A complete DARED proposal. Serializes to/from TOML for storage in
/// `_b00t_/datums/DARED-*.tomllmd`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaredProposal {
    /// Unique identifier (e.g., "DARED-001").
    pub proposal_id: String,

    /// Short title of the proposal.
    pub title: String,

    /// Current OODA phase.
    pub phase: OodaPhase,

    /// The decision being proposed.
    pub decision: Decision,

    /// Alternatives considered and rejected.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<Alternative>,

    /// Risks identified with mitigations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub risks: Vec<Risk>,

    /// Executive decision with summary, artifacts, and acceptance criteria.
    pub executive_decision: ExecutiveDecision,
}

impl DaredProposal {
    /// Create a new DARED proposal in the Decide phase.
    pub fn new(
        proposal_id: impl Into<String>,
        title: impl Into<String>,
        decision: Decision,
        executive_decision: ExecutiveDecision,
    ) -> Self {
        Self {
            proposal_id: proposal_id.into(),
            title: title.into(),
            phase: OodaPhase::Decide,
            decision,
            alternatives: Vec::new(),
            risks: Vec::new(),
            executive_decision,
        }
    }

    /// Add an alternative to the proposal.
    pub fn with_alternative(mut self, alt: Alternative) -> Self {
        self.alternatives.push(alt);
        self
    }

    /// Add a risk to the proposal.
    pub fn with_risk(mut self, risk: Risk) -> Self {
        self.risks.push(risk);
        self
    }

    /// Advance to the next OODA phase.
    pub fn advance_phase(&mut self) -> bool {
        if let Some(next) = self.phase.next() {
            self.phase = next;
            true
        } else {
            false
        }
    }
}

impl Stereotyped for DaredProposal {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Relator("DaredProposal".into())
    }
}

impl DaredDocument for DaredProposal {
    fn decision(&self) -> &Decision {
        &self.decision
    }

    fn alternatives(&self) -> &[Alternative] {
        &self.alternatives
    }

    fn risks(&self) -> &[Risk] {
        &self.risks
    }

    fn executive_decision(&self) -> &ExecutiveDecision {
        &self.executive_decision
    }

    fn proposal_id(&self) -> &str {
        &self.proposal_id
    }

    fn phase(&self) -> OodaPhase {
        self.phase
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// DaredAcceptanceCriteria — constraint type
// ══════════════════════════════════════════════════════════════════════════════

/// Acceptance criteria for a DARED proposal. Used as the constraint type
/// in `Satisfies<DaredAcceptanceCriteria>` implementations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaredAcceptanceCriteria {
    /// The proposal must have at least this many alternatives.
    pub min_alternatives: usize,
    /// All risks must have mitigations.
    pub require_mitigations: bool,
    /// At least one artifact or removed item must be declared.
    pub require_artifacts: bool,
    /// The executive summary must be non-empty (category label required).
    pub require_summary: bool,
    /// The proposal must be in at least this OODA phase.
    pub min_phase: OodaPhase,
}

impl Default for DaredAcceptanceCriteria {
    fn default() -> Self {
        Self {
            min_alternatives: 1,
            require_mitigations: true,
            require_artifacts: true,
            require_summary: true,
            min_phase: OodaPhase::Decide,
        }
    }
}

impl IsoAuditable for DaredAcceptanceCriteria {
    fn iso_standard_ids(&self) -> Vec<String> {
        vec!["ISO 31000:2018".into(), "ISO 9001:2015".into()]
    }
}

impl Satisfies<DaredAcceptanceCriteria> for DaredProposal {
    fn satisfies(&self, criteria: &DaredAcceptanceCriteria) -> SatisfiesResult {
        let mut issues = Vec::new();
        let mut score = 0.0_f64;
        let total = 5.0_f64;

        if self.alternatives.len() >= criteria.min_alternatives {
            score += 1.0;
        } else {
            issues.push(format!(
                "only {} alternatives (need {})",
                self.alternatives.len(),
                criteria.min_alternatives
            ));
        }

        if !criteria.require_mitigations
            || self.risks.iter().all(|r| !r.mitigation.is_empty())
        {
            score += 1.0;
        } else {
            let unmitigated: Vec<_> = self
                .risks
                .iter()
                .filter(|r| r.mitigation.is_empty())
                .map(|r| r.name.clone())
                .collect();
            issues.push(format!("unmitigated risks: {}", unmitigated.join(", ")));
        }

        if !criteria.require_artifacts
            || !(self.executive_decision.artifacts.is_empty()
                && self.executive_decision.removed.is_empty())
        {
            score += 1.0;
        } else {
            issues.push("no executive decision artifacts declared".into());
        }

        if !criteria.require_summary || !self.executive_decision.summary.is_empty() {
            score += 1.0;
        } else {
            issues.push("executive summary is empty".into());
        }

        if self.phase >= criteria.min_phase {
            score += 1.0;
        } else {
            issues.push(format!(
                "phase is {:?} (need at least {:?})",
                self.phase, criteria.min_phase
            ));
        }

        let confidence = score / total;

        if (confidence - 1.0_f64).abs() < f64::EPSILON {
            SatisfiesResult::satisfied(confidence)
        } else if confidence > 0.0 {
            SatisfiesResult::violated(issues.join("; "), confidence)
        } else {
            SatisfiesResult::violated("all checks failed", confidence)
        }
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_decision() -> Decision {
        Decision {
            what: "Adopt josh for vendor submodules".into(),
            who: "b00t-core".into(),
            when: "Q3 2026".into(),
            scope: vec!["vendor/".into()],
            explicitly_out_of_scope: vec!["crates/".into()],
        }
    }

    fn sample_alternative(name: &str, viable: &str, rejected: &str) -> Alternative {
        Alternative {
            name: name.into(),
            viable: viable.into(),
            rejected_because: rejected.into(),
        }
    }

    fn sample_risk(name: &str, severity: RiskSeverity, mitigation: &str) -> Risk {
        Risk {
            name: name.into(),
            severity,
            description: format!("Risk: {name}"),
            mitigation: mitigation.into(),
        }
    }

    fn sample_executive_decision() -> ExecutiveDecision {
        ExecutiveDecision {
            summary: "SubmoduleElimination ⊆ VendorGovernance ⊆ ArchitectureDecision".into(),
            artifacts: vec!["vendor.josh".into()],
            removed: vec![".gitmodules vendor entries".into()],
            acceptance_criteria: vec!["vendor.josh parseable".into()],
        }
    }

    fn valid_proposal() -> DaredProposal {
        DaredProposal::new(
            "DARED-001",
            "Eliminate vendor submodules",
            sample_decision(),
            sample_executive_decision(),
        )
        .with_alternative(sample_alternative(
            "do-nothing",
            "zero migration cost",
            "doesn't solve the problem",
        ))
        .with_risk(sample_risk("josh maturity", RiskSeverity::High, "pin to known-good SHA"))
    }

    // ── UFO grounding tests ──

    #[test]
    fn decision_is_endurant_kind() {
        let d = sample_decision();
        assert_eq!(d.ufo_stereotype().to_string(), "Kind:Decision");
    }

    #[test]
    fn alternative_is_endurant_kind() {
        let a = sample_alternative("do-nothing", "zero cost", "doesn't solve");
        assert_eq!(a.ufo_stereotype().to_string(), "Kind:Alternative");
    }

    #[test]
    fn risk_is_mode() {
        let r = sample_risk("test", RiskSeverity::Low, "mitigation");
        assert_eq!(r.ufo_stereotype().to_string(), "Mode:Risk");
    }

    #[test]
    fn executive_decision_is_relator() {
        let ed = sample_executive_decision();
        assert_eq!(ed.ufo_stereotype().to_string(), "Relator:ExecutiveDecision");
    }

    #[test]
    fn proposal_is_relator() {
        let p = valid_proposal();
        assert_eq!(p.ufo_stereotype().to_string(), "Relator:DaredProposal");
    }

    #[test]
    fn executive_decision_summary_is_set_category_label() {
        let ed = sample_executive_decision();
        assert!(ed.summary.contains("⊆"));
        assert!(ed.summary.contains("VendorGovernance"));
        assert!(ed.summary.contains("ArchitectureDecision"));
    }

    // ── OODA phase tests ──

    #[test]
    fn ooda_phase_ordering() {
        assert_eq!(OodaPhase::Observe.next(), Some(OodaPhase::Orient));
        assert_eq!(OodaPhase::Orient.next(), Some(OodaPhase::Decide));
        assert_eq!(OodaPhase::Decide.next(), Some(OodaPhase::Act));
        assert_eq!(OodaPhase::Act.next(), Some(OodaPhase::Verify));
        assert_eq!(OodaPhase::Verify.next(), None);
    }

    #[test]
    fn observe_precedes_all_others() {
        assert!(OodaPhase::Observe.precedes(OodaPhase::Orient));
        assert!(OodaPhase::Observe.precedes(OodaPhase::Decide));
        assert!(OodaPhase::Observe.precedes(OodaPhase::Verify));
    }

    #[test]
    fn verify_precedes_nothing() {
        assert!(!OodaPhase::Verify.precedes(OodaPhase::Observe));
        assert!(!OodaPhase::Verify.precedes(OodaPhase::Decide));
    }

    #[test]
    fn phase_partial_ord_matches_semantics() {
        assert!(OodaPhase::Decide > OodaPhase::Observe);
        assert!(OodaPhase::Act > OodaPhase::Decide);
    }

    #[test]
    fn phase_display_is_human_readable() {
        assert_eq!(OodaPhase::Observe.to_string(), "Observe");
        assert_eq!(OodaPhase::Act.to_string(), "Act");
    }

    // ── Validation tests ──

    #[test]
    fn valid_proposal_passes_validation() {
        let p = valid_proposal();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn missing_decision_what_fails() {
        let mut p = valid_proposal();
        p.decision.what = String::new();
        let errs = p.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, DaredValidationError::MissingSection(_))));
    }

    #[test]
    fn no_alternatives_fails() {
        let p = DaredProposal::new(
            "DARED-002",
            "no alts",
            sample_decision(),
            sample_executive_decision(),
        );
        let errs = p.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, DaredValidationError::NoAlternatives)));
    }

    #[test]
    fn unmitigated_risk_fails() {
        let mut p = valid_proposal();
        p.risks[0].mitigation = String::new();
        let errs = p.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, DaredValidationError::UnmitigatedRisk { .. })));
    }

    #[test]
    fn no_executive_decision_artifacts_fails() {
        let mut p = valid_proposal();
        p.executive_decision.artifacts.clear();
        p.executive_decision.removed.clear();
        let errs = p.validate().unwrap_err();
        assert!(errs
            .iter()
            .any(|e| matches!(e, DaredValidationError::NoExecutiveDecisionArtifacts)));
    }

    #[test]
    fn empty_summary_fails() {
        let mut p = valid_proposal();
        p.executive_decision.summary = String::new();
        let errs = p.validate().unwrap_err();
        assert!(errs.iter().any(|e| matches!(e, DaredValidationError::EmptySummary)));
    }

    #[test]
    fn empty_acceptance_criterion_fails() {
        let mut p = valid_proposal();
        p.executive_decision.acceptance_criteria = vec!["".into(), "  ".into()];
        let errs = p.validate().unwrap_err();
        assert_eq!(errs.len(), 2);
    }

    // ── Satisfies<DaredAcceptanceCriteria> tests ──

    #[test]
    fn full_proposal_satisfies_default_criteria() {
        let p = valid_proposal();
        let criteria = DaredAcceptanceCriteria::default();
        let result = p.satisfies(&criteria);
        assert!(result.is_satisfied(), "{result:?}");
    }

    #[test]
    fn missing_alternatives_violates_criteria() {
        let p = DaredProposal::new(
            "DARED-003",
            "no alts",
            sample_decision(),
            sample_executive_decision(),
        );
        let criteria = DaredAcceptanceCriteria::default();
        let result = p.satisfies(&criteria);
        assert!(result.is_violated());
    }

    #[test]
    fn wrong_phase_violates_criteria() {
        let mut p = valid_proposal();
        p.phase = OodaPhase::Observe;
        let criteria = DaredAcceptanceCriteria::default();
        let result = p.satisfies(&criteria);
        assert!(result.is_violated());
    }

    #[test]
    fn empty_summary_violates_criteria() {
        let mut p = valid_proposal();
        p.executive_decision.summary = String::new();
        let criteria = DaredAcceptanceCriteria::default();
        let result = p.satisfies(&criteria);
        assert!(result.is_violated());
    }

    // ── state machine tests ──

    #[test]
    fn advance_phase_through_full_cycle() {
        let mut p = valid_proposal();
        assert_eq!(p.phase, OodaPhase::Decide);

        assert!(p.advance_phase());
        assert_eq!(p.phase, OodaPhase::Act);

        assert!(p.advance_phase());
        assert_eq!(p.phase, OodaPhase::Verify);

        assert!(!p.advance_phase());
        assert_eq!(p.phase, OodaPhase::Verify);
    }

    #[test]
    fn is_ready_to_act_checks_phase_and_validation() {
        let p = valid_proposal();
        assert!(p.is_ready_to_act());

        let mut bad = valid_proposal();
        bad.phase = OodaPhase::Observe;
        assert!(!bad.is_ready_to_act());
    }

    // ── Serialization tests ──

    #[test]
    fn dared_proposal_roundtrips_json() {
        let p = valid_proposal();
        let json = serde_json::to_string_pretty(&p).unwrap();
        let p2: DaredProposal = serde_json::from_str(&json).unwrap();
        assert_eq!(p, p2);
    }

    #[test]
    fn risk_severity_ordering() {
        assert!(RiskSeverity::Critical > RiskSeverity::High);
        assert!(RiskSeverity::High > RiskSeverity::Medium);
        assert!(RiskSeverity::Medium > RiskSeverity::Low);
        assert_eq!(RiskSeverity::Low, RiskSeverity::Low);
    }

    #[test]
    fn risk_is_iso_auditable() {
        let r = sample_risk("test", RiskSeverity::Low, "mitigation");
        let ids = r.iso_standard_ids();
        assert!(ids.contains(&"ISO 31000:2018".to_string()));
    }

    #[test]
    fn acceptance_criteria_iso_auditable() {
        let c = DaredAcceptanceCriteria::default();
        let ids = c.iso_standard_ids();
        assert!(ids.contains(&"ISO 31000:2018".to_string()));
        assert!(ids.contains(&"ISO 9001:2015".to_string()));
    }
}
