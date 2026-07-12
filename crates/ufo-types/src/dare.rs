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
// OODA State Machine — transition system with guards + history
// ══════════════════════════════════════════════════════════════════════════════

/// Events that can trigger OODA phase transitions. Unlike `phase.next()`
/// (which is purely linear), events allow skipping, revisiting, and
/// edge-case transitions with guard enforcement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum OodaEvent {
    /// New information discovered; triggers Observe or re-Observe.
    Discover,
    /// Analysis complete; alternatives mapped.
    AnalyzeComplete,
    /// Decision committed; proposal drafted.
    Decide,
    /// Implementation or migration started.
    Execute,
    /// Acceptance criteria checked.
    VerifyComplete,
    /// Rollback to previous phase (e.g., Verify failed → re-Decide).
    Reject,
    /// Retry current phase (re-evaluate without changing state).
    Retry,
    /// Cancel the OODA loop entirely.
    Cancel,
}

impl OodaEvent {
    /// The target phase for this event given the current phase.
    /// Returns `None` if the event is not valid from the current phase.
    pub fn target(&self, current: OodaPhase) -> Option<OodaPhase> {
        match (self, current) {
            (OodaEvent::Discover, _) => Some(OodaPhase::Observe),
            (OodaEvent::AnalyzeComplete, OodaPhase::Observe) => Some(OodaPhase::Orient),
            (OodaEvent::Decide, OodaPhase::Orient) => Some(OodaPhase::Decide),
            (OodaEvent::Execute, OodaPhase::Decide) => Some(OodaPhase::Act),
            (OodaEvent::VerifyComplete, OodaPhase::Act) => Some(OodaPhase::Verify),
            (OodaEvent::Reject, OodaPhase::Verify) => Some(OodaPhase::Decide),
            (OodaEvent::Reject, OodaPhase::Act) => Some(OodaPhase::Decide),
            (OodaEvent::Retry, phase) => Some(phase),
            (OodaEvent::Cancel, _) => None,
            _ => None,
        }
    }
}

impl std::fmt::Display for OodaEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OodaEvent::Discover => write!(f, "Discover"),
            OodaEvent::AnalyzeComplete => write!(f, "AnalyzeComplete"),
            OodaEvent::Decide => write!(f, "Decide"),
            OodaEvent::Execute => write!(f, "Execute"),
            OodaEvent::VerifyComplete => write!(f, "VerifyComplete"),
            OodaEvent::Reject => write!(f, "Reject"),
            OodaEvent::Retry => write!(f, "Retry"),
            OodaEvent::Cancel => write!(f, "Cancel"),
        }
    }
}

/// Result of attempting an OODA state machine transition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OodaTransition {
    /// The event that triggered this transition.
    pub event: OodaEvent,
    /// The phase before the transition.
    pub from: OodaPhase,
    /// The phase after the transition (same as from if guard rejected).
    pub to: OodaPhase,
    /// Whether the transition was accepted.
    pub accepted: bool,
    /// Reason if rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rejection_reason: Option<String>,
}

impl OodaTransition {
    /// Create an accepted transition.
    pub fn accepted(event: OodaEvent, from: OodaPhase, to: OodaPhase) -> Self {
        Self {
            event,
            from,
            to,
            accepted: true,
            rejection_reason: None,
        }
    }

    /// Create a rejected transition.
    pub fn rejected(event: OodaEvent, from: OodaPhase, to: OodaPhase, reason: impl Into<String>) -> Self {
        Self {
            event,
            from,
            to,
            accepted: false,
            rejection_reason: Some(reason.into()),
        }
    }
}

/// Error type for OODA state machine operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
pub enum OodaStateMachineError {
    /// The requested transition is not valid from the current phase.
    #[error("invalid transition: {event} not valid from {current}")]
    InvalidTransition { event: OodaEvent, current: OodaPhase },

    /// A guard condition blocked the transition.
    #[error("guard blocked {event} from {from} → {to}: {reason}")]
    GuardBlocked {
        event: OodaEvent,
        from: OodaPhase,
        to: OodaPhase,
        reason: String,
    },

    /// Required phase data is missing (e.g., no Decision when entering Decide).
    #[error("missing phase data for {phase}: {detail}")]
    MissingPhaseData { phase: OodaPhase, detail: String },
}

/// Guards check whether a transition is permitted.
///
/// Each phase may have entry guards (must hold before entering) and
/// exit guards (must hold before leaving). Guards are pure functions
/// of the proposal state — no side effects.
#[derive(Debug, Clone)]
pub struct OodaGuards {
    /// Guards that must pass before exiting a phase.
    pub exit_guards: Vec<(OodaPhase, String)>,
    /// Guards that must pass before entering a phase.
    pub entry_guards: Vec<(OodaPhase, String)>,
}

impl Default for OodaGuards {
    fn default() -> Self {
        Self {
            exit_guards: Vec::new(),
            entry_guards: Vec::new(),
        }
    }
}

/// The OODA state machine — wraps a DaredDocument and enforces
/// valid transitions via events and guards.
///
/// # State machine diagram
///
/// ```text
///                    ┌──────────────────────────────────────┐
///                    │            ┌─────────┐               │
///   Discover ──────→ │ Observe ──→│  Orient  │              │
///   (anywhere)       │   │        │          │              │
///                    │   │        └────┬─────┘              │
///                    │   │   Analyze   │                    │
///                    │   │   Complete  │                    │
///                    │   │             ▼                    │
///                    │   │        ┌─────────┐              │
///                    │   └───────→│ Decide   │←─────────┐   │
///                    │            │          │          │   │
///                    │            └────┬─────┘    Reject│   │
///                    │                 │                 │   │
///                    │            Execute                │   │
///                    │                 │                 │   │
///                    │                 ▼                 │   │
///                    │            ┌─────────┐           │   │
///                    │            │   Act    │──────────┘   │
///                    │            │          │              │
///                    │            └────┬─────┘              │
///                    │                 │                    │
///                    │            VerifyComplete            │
///                    │                 │                    │
///                    │                 ▼                    │
///                    │            ┌─────────┐              │
///                    │            │ Verify   │── Cancel ──→ ∅
///                    │            └─────────┘              │
///                    └──────────────────────────────────────┘
/// ```
#[derive(Debug, Clone)]
pub struct OodaStateMachine<D: DaredDocument> {
    /// The proposal being tracked.
    pub proposal: D,
    /// History of accepted transitions for audit trail.
    pub history: Vec<OodaTransition>,
    /// Guards that gate transitions.
    pub guards: OodaGuards,
}

impl<D: DaredDocument> OodaStateMachine<D> {
    /// Create a new state machine wrapping a proposal.
    pub fn new(proposal: D) -> Self {
        Self {
            proposal,
            history: Vec::new(),
            guards: OodaGuards::default(),
        }
    }

    /// Current phase of the proposal.
    pub fn current_phase(&self) -> OodaPhase {
        self.proposal.phase()
    }

    /// Render the state machine as a compact ASCII diagram.
    /// Token-efficient: ~30 lines, scannable in <5s by human and LLM.
    /// Active phase is marked with `●`, inactive with `○`.
    pub fn render(&self) -> String {
        let p = self.current_phase();
        let dot = |ph: OodaPhase| if ph == p { "●" } else { "○" };

        let ev = |ph: OodaPhase| match ph {
            OodaPhase::Observe => "Discover",
            OodaPhase::Orient => "AnalyzeComplete",
            OodaPhase::Decide => "Decide",
            OodaPhase::Act => "Execute",
            OodaPhase::Verify => "VerifyComplete",
        };

        let title = self.proposal.title();

        format!(
            "\
╔══ OODA :: {title} ═══════════════════════════════════════╗
║                                                           ║
║   {o1} Observe ──{e1}──→ {o2} Orient ──{e2}──→ {o3} Decide ──{e3}──→ {o4} Act ──{e4}──→ {o5} Verify
║    │  ▲                     │  ▲                     │  ▲
║    │  └─── Discover ────────┘  └─── Reject ──────────┘  └─ Cancel
║    │                         │
║    └─── phase skip ──────────┘  (if preconditions met)
║
╠══ guards ─────────────────────────────────────────────────╣
║   Observe → Orient: │alternatives│ ≥ 1                    ║
║   Orient → Decide:  Decision.what ≠ \"\", summary ≠ \"\"     ║
║   Decide → Act:     validate() = PASS                    ║
║   Act → Verify:     │acceptance_criteria│ ≥ 1             ║
╠══ events ─────────────────────────────────────────────────╣
║   Discover   → Observe (from any phase)                   ║
║   Reject     → Decide  (from Act or Verify)               ║
║   Retry      → same   (re-evaluate current phase)         ║
║   Cancel     → ∅      (terminate)                         ║
╠══ trace ──────────────────────────────────────────────────╣
║   {trace}
╚═══════════════════════════════════════════════════════════╝",
            o1 = dot(OodaPhase::Observe),
            o2 = dot(OodaPhase::Orient),
            o3 = dot(OodaPhase::Decide),
            o4 = dot(OodaPhase::Act),
            o5 = dot(OodaPhase::Verify),
            e1 = ev(OodaPhase::Observe),
            e2 = ev(OodaPhase::Orient),
            e3 = ev(OodaPhase::Decide),
            e4 = ev(OodaPhase::Act),
            trace = self.audit_trail(),
        )
    }

    /// One-line status for log/JSONL evidence.
    pub fn status_line(&self) -> String {
        let p = self.current_phase();
        let id = self.proposal.proposal_id();
        let hist = self.history.len();
        let summary = self.proposal.executive_decision().summary.clone();
        format!("[{id}] phase={p} transitions={hist} summary=\"{summary}\"")
    }

    /// Check if the proposal is valid in its current phase.
    pub fn check_phase_invariants(&self) -> Result<(), OodaStateMachineError> {
        let phase = self.current_phase();
        match phase {
            OodaPhase::Observe => {
                // Observe: problem scoped but may be undefined
                Ok(())
            }
            OodaPhase::Orient => {
                // Orient: alternatives researched
                if self.proposal.alternatives().is_empty() {
                    return Err(OodaStateMachineError::MissingPhaseData {
                        phase,
                        detail: "at least one alternative must be researched in Orient".into(),
                    });
                }
                Ok(())
            }
            OodaPhase::Decide => {
                // Decide: decision committed, risks assessed
                if self.proposal.decision().what.is_empty() {
                    return Err(OodaStateMachineError::MissingPhaseData {
                        phase,
                        detail: "Decision.what must be non-empty in Decide".into(),
                    });
                }
                if self.proposal.executive_decision().summary.is_empty() {
                    return Err(OodaStateMachineError::MissingPhaseData {
                        phase,
                        detail: "ExecutiveDecision.summary must be non-empty in Decide".into(),
                    });
                }
                Ok(())
            }
            OodaPhase::Act => {
                // Act: ready to execute — validated DARED
                if self.proposal.validate().is_err() {
                    return Err(OodaStateMachineError::MissingPhaseData {
                        phase,
                        detail: "proposal must pass validation before Act".into(),
                    });
                }
                Ok(())
            }
            OodaPhase::Verify => {
                // Verify: artifacts produced, criteria checked
                let ed = self.proposal.executive_decision();
                if ed.acceptance_criteria.is_empty() {
                    return Err(OodaStateMachineError::MissingPhaseData {
                        phase,
                        detail: "acceptance criteria must be defined for Verify".into(),
                    });
                }
                Ok(())
            }
        }
    }

    /// Attempt to dispatch an event. Returns the transition result.
    /// If the transition is accepted, the proposal's phase is updated
    /// and the transition is recorded in history.
    pub fn dispatch(&mut self, event: OodaEvent) -> Result<OodaTransition, OodaStateMachineError> {
        let current = self.current_phase();

        // Cancel terminates the machine.
        if event == OodaEvent::Cancel {
            let t = OodaTransition {
                event,
                from: current,
                to: current,
                accepted: true,
                rejection_reason: None,
            };
            self.history.push(t.clone());
            return Ok(t);
        }

        // Retry stays in current phase.
        if event == OodaEvent::Retry {
            let t = OodaTransition {
                event,
                from: current,
                to: current,
                accepted: true,
                rejection_reason: None,
            };
            self.history.push(t.clone());
            return Ok(t);
        }

        // Resolve target phase.
        let target = event
            .target(current)
            .ok_or(OodaStateMachineError::InvalidTransition { event, current })?;

        // Check exit guards for current phase.
        for (phase, guard) in &self.guards.exit_guards {
            if *phase == current {
                return Ok(OodaTransition::rejected(
                    event,
                    current,
                    target,
                    format!("exit guard: {guard}"),
                ));
            }
        }

        // Check entry guards for target phase.
        for (phase, guard) in &self.guards.entry_guards {
            if *phase == target {
                return Ok(OodaTransition::rejected(
                    event,
                    current,
                    target,
                    format!("entry guard: {guard}"),
                ));
            }
        }

        // Check phase invariants before transition.
        if let Err(e) = self.check_phase_invariants() {
            return Ok(OodaTransition::rejected(
                event,
                current,
                target,
                e.to_string(),
            ));
        }

        // Accepted — apply transition.
        self.proposal.set_phase(target);
        let t = OodaTransition::accepted(event, current, target);
        self.history.push(t.clone());
        Ok(t)
    }

    /// Returns true if the state machine has reached the Verify phase
    /// and all acceptance criteria pass.
    pub fn is_complete(&self) -> bool {
        self.current_phase() == OodaPhase::Verify
            && self.proposal.validate().is_ok()
    }

    /// Rollback to a previous phase in history.
    pub fn rollback(&mut self, target: OodaPhase) -> Result<OodaTransition, OodaStateMachineError> {
        let current = self.current_phase();
        if !target.precedes(current) {
            return Err(OodaStateMachineError::InvalidTransition {
                event: OodaEvent::Reject,
                current,
            });
        }
        self.proposal.set_phase(target);
        let t = OodaTransition::accepted(OodaEvent::Reject, current, target);
        self.history.push(t.clone());
        Ok(t)
    }

    /// Walk the history and produce a summary of phase transitions.
    pub fn audit_trail(&self) -> String {
        if self.history.is_empty() {
            return format!("phase: {} (no transitions)", self.current_phase());
        }
        let steps: Vec<String> = self
            .history
            .iter()
            .map(|t| {
                if t.accepted {
                    format!("{}: {} → {}", t.event, t.from, t.to)
                } else {
                    format!("{}: {} → {} [BLOCKED: {}]", t.event, t.from, t.to,
                        t.rejection_reason.as_deref().unwrap_or("unknown"))
                }
            })
            .collect();
        format!("{} | current: {}", steps.join(" | "), self.current_phase())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// DaredAcceptanceCriteria — constraint type
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

    /// Human-readable title.
    fn title(&self) -> &str;

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

    /// Set the current phase (mutable access — used by state machine).
    fn set_phase(&mut self, phase: OodaPhase);
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

    fn title(&self) -> &str {
        &self.title
    }

    fn phase(&self) -> OodaPhase {
        self.phase
    }

    fn set_phase(&mut self, phase: OodaPhase) {
        self.phase = phase;
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

    // ── State machine tests ──

    #[test]
    fn state_machine_dispatch_observe_to_orient() {
        let mut sm = OodaStateMachine::new(valid_proposal());
        assert_eq!(sm.current_phase(), OodaPhase::Decide);

        let t = sm.dispatch(OodaEvent::Discover).unwrap();
        assert!(t.accepted);
        assert_eq!(sm.current_phase(), OodaPhase::Observe);

        let t = sm.dispatch(OodaEvent::AnalyzeComplete).unwrap();
        assert!(t.accepted);
        assert_eq!(sm.current_phase(), OodaPhase::Orient);
    }

    #[test]
    fn state_machine_blocked_skip_orient_without_alternatives() {
        let p = DaredProposal::new(
            "DARED-004",
            "skip test",
            sample_decision(),
            sample_executive_decision(),
        );
        let mut sm = OodaStateMachine::new(p);
        // Manually set to Orient (bypassing guards for test)
        sm.proposal.phase = OodaPhase::Orient;

        // Try to go Orient → Decide without alternatives
        let t = sm.dispatch(OodaEvent::Decide).unwrap();
        assert!(!t.accepted, "should block: no alternatives in Orient");
    }

    #[test]
    fn state_machine_reject_verify_to_decide() {
        let mut sm = OodaStateMachine::new(valid_proposal());
        sm.proposal.phase = OodaPhase::Verify;

        let t = sm.dispatch(OodaEvent::Reject).unwrap();
        assert!(t.accepted);
        assert_eq!(sm.current_phase(), OodaPhase::Decide);
    }

    #[test]
    fn state_machine_cancel_terminates() {
        let mut sm = OodaStateMachine::new(valid_proposal());
        let t = sm.dispatch(OodaEvent::Cancel).unwrap();
        assert!(t.accepted);
        assert_eq!(sm.current_phase(), OodaPhase::Decide); // phase unchanged
        assert_eq!(sm.history.len(), 1);
    }

    #[test]
    fn state_machine_history_accumulates() {
        let mut sm = OodaStateMachine::new(valid_proposal());
        sm.dispatch(OodaEvent::Discover).unwrap(); // Decide → Observe
        sm.dispatch(OodaEvent::AnalyzeComplete).unwrap(); // Observe → Orient
        assert_eq!(sm.history.len(), 2);
        assert!(sm.audit_trail().contains("Discover"));
        assert!(sm.audit_trail().contains("AnalyzeComplete"));
    }

    #[test]
    fn state_machine_render_shows_active_phase() {
        let sm = OodaStateMachine::new(valid_proposal());
        let r = sm.render();
        assert!(r.contains("●"));
        assert!(r.contains("Decide"));
        assert!(r.contains("Eliminate vendor submodules"));
    }

    #[test]
    fn state_machine_status_line_is_compact() {
        let sm = OodaStateMachine::new(valid_proposal());
        let s = sm.status_line();
        assert!(s.contains("DARED-001"));
        assert!(s.contains("phase=Decide"));
        assert!(s.contains("SubmoduleElimination"));
    }

    #[test]
    fn ooda_event_target_mapping() {
        assert_eq!(OodaEvent::Discover.target(OodaPhase::Decide), Some(OodaPhase::Observe));
        assert_eq!(OodaEvent::Discover.target(OodaPhase::Act), Some(OodaPhase::Observe));
        assert_eq!(OodaEvent::Decide.target(OodaPhase::Orient), Some(OodaPhase::Decide));
        assert_eq!(OodaEvent::Reject.target(OodaPhase::Verify), Some(OodaPhase::Decide));
        assert_eq!(OodaEvent::Execute.target(OodaPhase::Orient), None); // can't Execute from Orient
        assert_eq!(OodaEvent::Retry.target(OodaPhase::Act), Some(OodaPhase::Act));
        assert_eq!(OodaEvent::Cancel.target(OodaPhase::Observe), None);
    }

    #[test]
    fn state_machine_retry_stays_in_phase() {
        let mut sm = OodaStateMachine::new(valid_proposal());
        let t = sm.dispatch(OodaEvent::Retry).unwrap();
        assert!(t.accepted);
        assert_eq!(t.from, OodaPhase::Decide);
        assert_eq!(t.to, OodaPhase::Decide);
    }

    #[test]
    fn state_machine_invalid_transition_rejected() {
        let mut sm = OodaStateMachine::new(valid_proposal());
        // Can't go Decide → VerifyComplete (must Execute first)
        let result = sm.dispatch(OodaEvent::VerifyComplete);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            OodaStateMachineError::InvalidTransition { .. }
        ));
    }
}
