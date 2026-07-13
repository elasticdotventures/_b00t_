//! Maximized OODA (Observe-Orient-Decide-Act) decision framework.
//!
//! Provides a state-machine-driven OODA loop with:
//! - `OodaPhase` state machine transitions (Idle -> Observe -> Orient -> Decide -> Act -> Review)
//! - `OodaGuardRails` for iteration/failure/duration limits
//! - `OodaConfig` with autoresearch flag for complex observations
//! - `check_peer_handshake()` for ledgrrr/b00t peer integration
//!
//! # State Machine
//!
//! ```text
//! Idle -> Observing -> Orienting -> Deciding -> Acting -> Reviewing -> Complete
//!                            ^                                      |
//!                            +--- (loop back on re-observe) --------+
//! ```
//!
//! Any phase can transition directly to `Failed(reason)`.

use serde::{Deserialize, Serialize};
use statig::prelude::*;
use std::path::PathBuf;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Part 1: OODA phase states with validated transitions
// ---------------------------------------------------------------------------

/// OODA phase states for state machine transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OodaPhase {
    Idle,
    Observing,
    Orienting,
    Deciding,
    Acting,
    Reviewing,
    Complete,
    Failed(String),
}

impl OodaPhase {
    /// Returns `true` if a transition from `self` to `next` is valid.
    ///
    /// Valid transitions:
    /// - `Idle`        → `Observing`
    /// - `Observing`   → `Orienting`
    /// - `Orienting`   → `Deciding`
    /// - `Deciding`    → `Acting`
    /// - `Acting`      → `Reviewing`
    /// - `Reviewing`   → `Complete`  (successful finish)
    /// - `Reviewing`   → `Observing` (loop back for another cycle)
    /// - Any phase     → `Failed(_)` (abort with reason)
    pub fn can_transition_to(&self, next: &OodaPhase) -> bool {
        matches!(
            (self, next),
            (OodaPhase::Idle, OodaPhase::Observing)
                | (OodaPhase::Observing, OodaPhase::Orienting)
                | (OodaPhase::Orienting, OodaPhase::Deciding)
                | (OodaPhase::Deciding, OodaPhase::Acting)
                | (OodaPhase::Acting, OodaPhase::Reviewing)
                | (OodaPhase::Reviewing, OodaPhase::Complete)
                | (OodaPhase::Reviewing, OodaPhase::Observing) // loop back
                | (_, OodaPhase::Failed(_)) // any phase can fail
        )
    }

    /// Returns the next phase in a standard forward progression, ignoring failures.
    /// Useful for iterating through a clean cycle.
    pub fn next_forward(&self) -> Option<OodaPhase> {
        match self {
            OodaPhase::Idle => Some(OodaPhase::Observing),
            OodaPhase::Observing => Some(OodaPhase::Orienting),
            OodaPhase::Orienting => Some(OodaPhase::Deciding),
            OodaPhase::Deciding => Some(OodaPhase::Acting),
            OodaPhase::Acting => Some(OodaPhase::Reviewing),
            OodaPhase::Reviewing => Some(OodaPhase::Complete),
            OodaPhase::Complete | OodaPhase::Failed(_) => None,
        }
    }
}

fn ooda_phase_can_transition(source: &OodaPhase, target: &OodaPhase) -> bool {
    source.can_transition_to(target)
}

fn ooda_phase_event_for_target(target: &OodaPhase) -> Option<&'static str> {
    match target {
        OodaPhase::Idle => None,
        OodaPhase::Observing => Some("GoToObserving"),
        OodaPhase::Orienting => Some("GoToOrienting"),
        OodaPhase::Deciding => Some("GoToDeciding"),
        OodaPhase::Acting => Some("GoToActing"),
        OodaPhase::Reviewing => Some("GoToReviewing"),
        OodaPhase::Complete => Some("GoToComplete"),
        OodaPhase::Failed(_) => Some("GoToFailed"),
    }
}

crate::impl_state_machine_introspection! {
    impl OodaPhase {
        machine_id: "OodaPhase",
        initial: "Idle",
        states: [
            OodaPhase::Idle => "Idle",
            OodaPhase::Observing => "Observing",
            OodaPhase::Orienting => "Orienting",
            OodaPhase::Deciding => "Deciding",
            OodaPhase::Acting => "Acting",
            OodaPhase::Reviewing => "Reviewing",
            OodaPhase::Complete => "Complete",
            OodaPhase::Failed(String::new()) => "Failed",
        ],
        finals: ["Complete", "Failed"],
        can_transition: ooda_phase_can_transition,
        event_for_target: ooda_phase_event_for_target,
    }
}

pub fn ooda_phase_mermaid_state_diagram() -> String {
    <OodaPhase as crate::state_introspection::StateMachineIntrospection>::render_mermaid_state_diagram()
}

pub fn ooda_phase_s5() -> String {
    <OodaPhase as crate::state_introspection::StateMachineIntrospection>::render_s5()
}

// ---------------------------------------------------------------------------
// Part 2: Guard rails — replace hardcoded limits
// ---------------------------------------------------------------------------

/// Guard-rail and policy configuration for an OODA loop.
///
/// This struct carries execution limits and higher-level policy metadata.
/// In this module, callers MUST NOT assume every field is enforced by
/// `run_phases()`: duration and approval settings are declarative unless an
/// external orchestrator or future implementation explicitly applies them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OodaGuardRails {
    /// Maximum number of OODA iterations before the loop terminates.
    pub max_iterations: u32,
    /// Maximum number of failed iterations before the loop aborts.
    pub max_failures: u32,
    /// Declarative maximum wall-clock time in seconds for integrations that
    /// enforce runtime duration limits; not enforced by `run_phases()`.
    pub max_duration_secs: u64,
    /// Declarative approval-gate requirement for integrations that enforce
    /// human review before `Acting`; not enforced by `run_phases()`.
    pub require_approval: bool,
}

impl Default for OodaGuardRails {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_failures: 3,
            max_duration_secs: 3600,
            require_approval: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Part 4: OODA configuration (includes autoresearch flag)
// ---------------------------------------------------------------------------

/// High-level configuration for an OODA loop session.
///
/// Bundles the guard rails together with behavioural flags such
/// as the autoresearch toggle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OodaConfig {
    /// Safety and limit constraints.
    pub guard_rails: OodaGuardRails,
    /// When `true`, the Orient phase will automatically trigger a research
    /// sub-cycle when it encounters a complex observation (>100 characters).
    pub enable_autoresearch: bool,
}

impl Default for OodaConfig {
    fn default() -> Self {
        Self {
            guard_rails: OodaGuardRails::default(),
            enable_autoresearch: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal statig HSM — OodaLoop state machine internals
// ---------------------------------------------------------------------------

/// Internal event type dispatched from transition_to() into the statig machine.
#[derive(Debug, Clone)]
enum OodaDispatch {
    GoToObserving,
    GoToOrienting,
    GoToDeciding,
    GoToActing,
    GoToReviewing,
    GoToComplete,
    #[allow(dead_code)]
    GoToFailed(String),
}

fn phase_to_dispatch(phase: &OodaPhase) -> OodaDispatch {
    match phase {
        OodaPhase::Observing      => OodaDispatch::GoToObserving,
        OodaPhase::Orienting      => OodaDispatch::GoToOrienting,
        OodaPhase::Deciding       => OodaDispatch::GoToDeciding,
        OodaPhase::Acting         => OodaDispatch::GoToActing,
        OodaPhase::Reviewing      => OodaDispatch::GoToReviewing,
        OodaPhase::Complete       => OodaDispatch::GoToComplete,
        OodaPhase::Failed(r)      => OodaDispatch::GoToFailed(r.clone()),
        OodaPhase::Idle           => unreachable!("no dispatch for Idle (initial state)"),
    }
}

/// Minimal context for the statig state machine. All side effects (json write)
/// are handled by OodaLoop::transition_to() so this struct is intentionally empty.
#[derive(Default)]
struct OodaCtx;

/// Write `~/.b00t/ooda-state.json` with the current OodaPhase. Best-effort; ignores errors.
fn write_ooda_state_json(phase: &OodaPhase) {
    let Some(home) = dirs::home_dir() else { return };
    let path = home.join(".b00t/ooda-state.json");
    let json = match phase {
        OodaPhase::Failed(r) => format!(
            r#"{{"phase":"Failed","reason":{}}}"#,
            serde_json::to_string(r).unwrap_or_else(|_| r#""""#.into())
        ),
        other => format!(r#"{{"phase":"{:?}"}}"#, other),
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, json);
}

type OodaOutcome = statig::Outcome<OodaState>;

#[state_machine(
    initial = "OodaState::idle()",
    state(name = "OodaState"),
    event_identifier = "event"
)]
impl OodaCtx {
    #[state]
    fn idle(&mut self, event: &OodaDispatch) -> OodaOutcome {
        match event {
            OodaDispatch::GoToObserving  => Transition(OodaState::observing()),
            OodaDispatch::GoToFailed(_)  => Transition(OodaState::failed()),
            _                            => Super,
        }
    }

    #[state]
    fn observing(&mut self, event: &OodaDispatch) -> OodaOutcome {
        match event {
            OodaDispatch::GoToOrienting  => Transition(OodaState::orienting()),
            OodaDispatch::GoToFailed(_)  => Transition(OodaState::failed()),
            _                            => Super,
        }
    }

    #[state]
    fn orienting(&mut self, event: &OodaDispatch) -> OodaOutcome {
        match event {
            OodaDispatch::GoToDeciding   => Transition(OodaState::deciding()),
            OodaDispatch::GoToFailed(_)  => Transition(OodaState::failed()),
            _                            => Super,
        }
    }

    #[state]
    fn deciding(&mut self, event: &OodaDispatch) -> OodaOutcome {
        match event {
            OodaDispatch::GoToActing     => Transition(OodaState::acting()),
            OodaDispatch::GoToFailed(_)  => Transition(OodaState::failed()),
            _                            => Super,
        }
    }

    #[state]
    fn acting(&mut self, event: &OodaDispatch) -> OodaOutcome {
        match event {
            OodaDispatch::GoToReviewing  => Transition(OodaState::reviewing()),
            OodaDispatch::GoToFailed(_)  => Transition(OodaState::failed()),
            _                            => Super,
        }
    }

    #[state]
    fn reviewing(&mut self, event: &OodaDispatch) -> OodaOutcome {
        match event {
            OodaDispatch::GoToComplete   => Transition(OodaState::complete()),
            OodaDispatch::GoToObserving  => Transition(OodaState::observing()),
            OodaDispatch::GoToFailed(_)  => Transition(OodaState::failed()),
            _                            => Super,
        }
    }

    #[state]
    #[state]
    fn complete(&mut self, event: &OodaDispatch) -> OodaOutcome {
        let _ = event;
        Handled
    }

    #[state]
    fn failed(&mut self, event: &OodaDispatch) -> OodaOutcome {
        let _ = event;
        Handled
    }
}

// ---------------------------------------------------------------------------
// Part 3: Handshake peer check (ledgrrr / b00t mesh)
// ---------------------------------------------------------------------------

/// Check whether a peer handshake document exists from ledgrrr or another
/// b00t instance.
///
/// Looks in two places:
/// 1. `~/.b00t/mesh/l3dg3rr.handshake` (ledgrrr's handshake surface output)
/// 2. `_b00t_/handshake/l3dg3rr.json`  (project-local handshake)
///
/// Returns the `variant_id` string extracted from the first found document,
/// or `None` when no handshake is present or the document cannot be parsed.
pub fn check_peer_handshake() -> Option<String> {
    let paths: Vec<PathBuf> = [
        dirs::home_dir().map(|h| h.join(".b00t").join("mesh").join("l3dg3rr.handshake")),
        Some(PathBuf::from("_b00t_/handshake/l3dg3rr.json")),
    ]
    .into_iter()
    .flatten()
    .collect();

    check_peer_handshake_inner(&paths)
}

/// Inner implementation that accepts an injectable path list.
/// Used by `check_peer_handshake()` and tests.
fn check_peer_handshake_inner(paths: &[PathBuf]) -> Option<String> {
    for path in paths {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(path) {
                // Try JSON first
                if let Ok(document) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(variant) = document.get("variant_id").and_then(|v| v.as_str()) {
                        return Some(variant.to_string());
                    }
                }
                // Fallback: plaintext `key: value` per line
                for line in content.lines() {
                    if let Some(rest) = line.strip_prefix("variant_id:") {
                        let val = rest.trim().trim_matches('"');
                        if !val.is_empty() {
                            return Some(val.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Core data types
// ---------------------------------------------------------------------------

/// Result of a single OODA cycle phase.
#[derive(Debug, Clone, PartialEq)]
pub enum PhaseResult {
    Continue,
    Repeat,
    Terminate(String),
}

/// One OODA cycle iteration.
#[derive(Debug, Clone)]
pub struct OodaIteration {
    pub phase: String,
    pub observation: String,
    pub orientation: String,
    pub decision: String,
    pub action: String,
    pub success: bool,
}

impl OodaIteration {
    /// Build an iteration that records which OodaPhase was active.
    pub fn from_phase(phase: &OodaPhase, observation: &str, success: bool) -> Self {
        Self {
            phase: format!("{:?}", phase),
            observation: observation.to_string(),
            orientation: String::new(),
            decision: String::new(),
            action: String::new(),
            success,
        }
    }
}

// ---------------------------------------------------------------------------
// OODA loop executor with state machine integration
// ---------------------------------------------------------------------------

/// Maximized OODA loop executor.
///
/// Drives the Observe–Orient–Decide–Act cycle as a validated state machine
/// with guard rails, optional handshake peer checks, and autoresearch.
pub struct OodaLoop {
    /// Safety constraints applied to every run.
    pub guard_rails: OodaGuardRails,
    iterations: Vec<OodaIteration>,
    /// Current phase in the state machine (mirrors inner statig state).
    current_phase: OodaPhase,
    /// Running failure count across iterations.
    failure_count: u32,
    /// When `enable_autoresearch` is true, complex observations trigger a
    /// research sub-cycle during the Orient phase.
    pub enable_autoresearch: bool,
    /// Internal statig HSM — validates phase transitions.
    sm: statig::blocking::StateMachine<OodaCtx>,
}

impl OodaLoop {
    /// Create a new loop with default guard rails and a custom iteration cap.
    ///
    /// This constructor preserves backward compatibility with the previous
    /// `OodaLoop::new(max_iterations)` signature.
    pub fn new(max_iterations: u32) -> Self {
        Self {
            guard_rails: OodaGuardRails {
                max_iterations,
                ..OodaGuardRails::default()
            },
            iterations: Vec::new(),
            current_phase: OodaPhase::Idle,
            failure_count: 0,
            enable_autoresearch: false,
            sm: OodaCtx::default().state_machine(),
        }
    }

    /// Create a new loop from a full `OodaConfig`.
    pub fn from_config(config: OodaConfig) -> Self {
        Self {
            guard_rails: config.guard_rails,
            iterations: Vec::new(),
            current_phase: OodaPhase::Idle,
            failure_count: 0,
            enable_autoresearch: config.enable_autoresearch,
            sm: OodaCtx::default().state_machine(),
        }
    }

    /// Create a new loop with custom guard rails.
    pub fn with_guard_rails(guard_rails: OodaGuardRails) -> Self {
        Self {
            guard_rails,
            iterations: Vec::new(),
            current_phase: OodaPhase::Idle,
            failure_count: 0,
            enable_autoresearch: false,
            sm: OodaCtx::default().state_machine(),
        }
    }

    // ---- Legacy flat-run interface ----------------------------------------

    /// Run the OODA cycle up to `guard_rails.max_iterations` times.
    ///
    /// Each call to `cycle` receives the current zero-based iteration index.
    /// Returns the last iteration produced, or a default iteration if zero runs.
    ///
    /// This method is the **legacy flat interface** — it does NOT enforce
    /// the `OodaPhase` state machine.  Use `run_phases` for state-machine
    /// driven execution.
    pub fn run<F>(&mut self, mut cycle: F) -> OodaIteration
    where
        F: FnMut(u32) -> OodaIteration,
    {
        let mut last = OodaIteration {
            phase: String::new(),
            observation: String::new(),
            orientation: String::new(),
            decision: String::new(),
            action: String::new(),
            success: true,
        };
        let limit = self.guard_rails.max_iterations;
        for i in 0..limit {
            let iteration = cycle(i);
            if !iteration.success {
                self.failure_count += 1;
                if self.failure_count >= self.guard_rails.max_failures {
                    let mut failed = iteration.clone();
                    failed.phase = format!("{:?}", OodaPhase::Failed(
                        "max_failures exceeded".into()
                    ));
                    self.iterations.push(failed.clone());
                    let _ = self.transition_to(OodaPhase::Failed("max_failures exceeded".into()));
                    return failed;
                }
            }
            self.iterations.push(iteration.clone());
            last = iteration;
        }
        last
    }

    // ---- State-machine phase interface ------------------------------------

    /// Advance the state machine to `next` phase.
    ///
    /// Returns `Ok(())` on success, or `Err` with a message describing the
    /// invalid transition.
    pub fn transition_to(&mut self, next: OodaPhase) -> Result<(), String> {
        if !self.current_phase.can_transition_to(&next) {
            return Err(format!(
                "Invalid OODA transition: {:?} -> {:?}",
                self.current_phase, next
            ));
        }
        // Dispatch through statig HSM
        let dispatch = phase_to_dispatch(&next);
        self.sm.handle(&dispatch);
        // Update current_phase and write ooda-state.json on each transition
        self.current_phase = next.clone();
        write_ooda_state_json(&next);
        Ok(())
    }

    /// Return the current phase of the state machine.
    pub fn phase(&self) -> &OodaPhase {
        &self.current_phase
    }

    /// Run the OODA cycle with state-machine phase transitions.
    ///
    /// The provided `cycle` closure receives:
    /// - the current `OodaPhase` (guaranteed valid)
    /// - the zero-based iteration index
    ///
    /// It should return an `OodaIteration` along with the next desired phase.
    /// The runner validates every transition, enforces guard rails, and
    /// handles autoresearch logic during the Orient phase.
    ///
    /// Returns `None` if the loop halts before producing any iterations
    /// (e.g. if `max_iterations` is 0 and no runs are made).
    pub fn run_phases<F>(&mut self, mut cycle: F) -> Option<OodaIteration>
    where
        F: FnMut(&OodaPhase, u32) -> (OodaIteration, OodaPhase),
    {
        let mut last: Option<OodaIteration> = None;
        let limit = self.guard_rails.max_iterations;
        let start = Instant::now();

        // Start with Idle -> Observing
        if let Err(e) = self.transition_to(OodaPhase::Observing) {
            let failed = OodaIteration {
                phase: format!("{:?}", OodaPhase::Failed(e.clone())),
                observation: e,
                orientation: String::new(),
                decision: String::new(),
                action: String::new(),
                success: false,
            };
            self.iterations.push(failed.clone());
            let _ = self.transition_to(OodaPhase::Failed("initial transition failed".into()));
            return Some(failed);
        }

        for i in 0..limit {
            // --- Guard: max_duration_secs ---
            if start.elapsed().as_secs() >= self.guard_rails.max_duration_secs {
                let failed = OodaIteration {
                    phase: format!("{:?}", OodaPhase::Failed("max_duration_secs exceeded".into())),
                    observation: "max_duration_secs exceeded".into(),
                    orientation: String::new(),
                    decision: String::new(),
                    action: String::new(),
                    success: false,
                };
                self.iterations.push(failed.clone());
                let _ = self.transition_to(OodaPhase::Failed("max_duration_secs exceeded".into()));
                return Some(failed);
            }

            // Invoke the user callback with the current phase.
            let (mut iteration, next_phase) = cycle(&self.current_phase, i);

            // --- Part 4: Autoresearch integration ---
            // If we're in the Orient phase and the observation is complex
            // (heuristic: >100 chars), and autoresearch is enabled, flag it.
            if self.enable_autoresearch
                && self.current_phase == OodaPhase::Orienting
                && iteration.observation.len() > 100
            {
                // Tag the orientation field to indicate a research sub-cycle
                // was triggered. The caller can inspect this to drive an
                // actual research workflow.
                if iteration.orientation.is_empty() {
                    iteration.orientation =
                        "autoresearch: complex observation triggered research sub-cycle".to_string();
                } else {
                    iteration.orientation = format!(
                        "{}; autoresearch: complex observation triggered research sub-cycle",
                        iteration.orientation
                    );
                }
            }

            iteration.phase = format!("{:?}", self.current_phase);

            // Record the iteration.
            if !iteration.success {
                self.failure_count += 1;
            }

            self.iterations.push(iteration.clone());
            last = Some(iteration.clone());

            // --- Guard: max_failures ---
            if self.failure_count >= self.guard_rails.max_failures {
                let failed = OodaIteration {
                    phase: format!("{:?}", OodaPhase::Failed("max_failures exceeded".into())),
                    ..iteration
                };
                self.iterations.push(failed.clone());
                let _ = self.transition_to(OodaPhase::Failed("max_failures exceeded".into()));
                return Some(failed);
            }

            // --- Guard: Complete terminates the loop ---
            if next_phase == OodaPhase::Complete {
                let _ = self.transition_to(OodaPhase::Complete);
                return last;
            }

            // --- Guard: Failed terminates the loop ---
            if matches!(&next_phase, OodaPhase::Failed(_)) {
                let failed = OodaIteration {
                    phase: format!("{:?}", next_phase),
                    ..iteration
                };
                self.iterations.push(failed.clone());
                let _ = self.transition_to(next_phase);
                return Some(failed);
            }

            // Validate and apply the phase transition.
            if let Err(e) = self.transition_to(next_phase) {
                let failed = OodaIteration {
                    phase: format!("{:?}", OodaPhase::Failed(e.clone())),
                    observation: e,
                    ..iteration
                };
                self.iterations.push(failed.clone());
                let _ = self.transition_to(OodaPhase::Failed("invalid phase transition".into()));
                return Some(failed);
            }
        }

        last
    }

    /// Return all iterations recorded so far.
    pub fn iterations(&self) -> &[OodaIteration] {
        &self.iterations
    }

    /// Fraction of iterations where `success == true` (0.0 ..= 1.0).
    /// Returns 0.0 when there are no iterations.
    pub fn success_rate(&self) -> f64 {
        if self.iterations.is_empty() {
            return 0.0;
        }
        let ok = self.iterations.iter().filter(|i| i.success).count();
        ok as f64 / self.iterations.len() as f64
    }

    /// Reset the loop state (clears iterations, failures, resets phase to Idle).
    pub fn reset(&mut self) {
        self.iterations.clear();
        self.current_phase = OodaPhase::Idle;
        self.failure_count = 0;
        self.sm = OodaCtx::default().state_machine();
    }
}

// ---------------------------------------------------------------------------
// PolyConnector — typed phase wrappers
// ---------------------------------------------------------------------------
//
// OODA phases are a natural PolyConnector chain:
//
//   ObserveNode  (env → Observation)
//     .then(OrientNode)   (Observation → Orientation)
//     .then(DecideNode)   (Orientation → Decision)
//     .then(ActNode)      (Decision    → ActionResult)
//
// `OodaLoop::run_typed()` enforces this type chain at compile time.
// Each node is a thin wrapper around the user-supplied closure.
// The existing `run_phases()` API remains unchanged — this is additive.

/// Phase-typed newtype for the Observe output.
#[derive(Debug, Clone)]
pub struct Observation(pub String);

/// Phase-typed newtype for the Orient output.
#[derive(Debug, Clone)]
pub struct Orientation(pub String);

/// Phase-typed newtype for the Decide output.
#[derive(Debug, Clone)]
pub struct Decision(pub String);

/// Phase-typed newtype for the Act output.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub summary: String,
    pub success: bool,
}

/// Input to the Observe phase — carries the loop iteration number.
#[derive(Debug, Clone)]
pub struct ObserveInput(pub u32);

// ── Phase node wrappers ────────────────────────────────────────────────────

use crate::pipeline_nodes::{NodeCategory, NodeShape, NodeStyle, PipelineNode, PortDef, PortDirection, StateMachine};
use crate::doc_pipeline::SerializableFOLFormula;

/// Typed Observe phase node.  `ObserveInput → Observation`.
pub struct ObserveNode<F>(pub F);
/// Typed Orient phase node.  `Observation → Orientation`.
pub struct OrientNode<F>(pub F);
/// Typed Decide phase node.  `Orientation → Decision`.
pub struct DecideNode<F>(pub F);
/// Typed Act phase node.  `Decision → ActionResult`.
pub struct ActNode<F>(pub F);

impl<F> std::fmt::Debug for ObserveNode<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "ObserveNode") }
}
impl<F> std::fmt::Debug for OrientNode<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "OrientNode") }
}
impl<F> std::fmt::Debug for DecideNode<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "DecideNode") }
}
impl<F> std::fmt::Debug for ActNode<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "ActNode") }
}

macro_rules! ooda_phase_node {
    ($node:ident, $id:literal, $label:literal, $input:ty, $output:ty) => {
        impl<F> PipelineNode for $node<F>
        where
            F: Fn($input) -> $output + Send + Sync,
        {
            type Input  = $input;
            type Output = $output;
            fn node_id(&self)    -> &str { $id }
            fn node_label(&self) -> &str { $label }
            fn node_category(&self) -> NodeCategory { NodeCategory::Transform }
            fn preconditions(&self)  -> Vec<SerializableFOLFormula> { vec![] }
            fn postconditions(&self) -> Vec<SerializableFOLFormula> { vec![] }
            fn invariants(&self)     -> Vec<SerializableFOLFormula> { vec![] }
            fn execute(&self, input: $input) -> $output { (self.0)(input) }
            fn state_machine(&self) -> StateMachine { StateMachine::idle_run_cycle($id) }
            fn input_ports(&self)  -> Vec<PortDef> {
                vec![PortDef { name: "in".into(), port_type: stringify!($input).into(), direction: PortDirection::Input }]
            }
            fn output_ports(&self) -> Vec<PortDef> {
                vec![PortDef { name: "out".into(), port_type: stringify!($output).into(), direction: PortDirection::Output }]
            }
            fn visual_style(&self) -> NodeStyle {
                NodeStyle { fill: "#0f172a".into(), stroke: "#60a5fa".into(), shape: NodeShape::RoundedBox }
            }
        }
    };
}

ooda_phase_node!(ObserveNode, "ooda.observe", "Observe", ObserveInput, Observation);
ooda_phase_node!(OrientNode,  "ooda.orient",  "Orient",  Observation,  Orientation);
ooda_phase_node!(DecideNode,  "ooda.decide",  "Decide",  Orientation,  Decision);
ooda_phase_node!(ActNode,     "ooda.act",     "Act",     Decision,     ActionResult);

impl OodaLoop {
    /// Run the OODA loop using typed PolyConnector phase nodes.
    ///
    /// Type-checks the full Observe→Orient→Decide→Act chain at compile time.
    /// Runs exactly once; use `run_phases` for multi-iteration guard-railed loops.
    ///
    /// # Example
    /// ```rust,ignore
    /// loop_.run_typed(
    ///     ObserveNode(|i: ObserveInput| Observation(format!("env at iter {}", i.0))),
    ///     OrientNode(|obs: Observation| Orientation(format!("analysis of: {}", obs.0))),
    ///     DecideNode(|ori: Orientation| Decision(format!("plan: {}", ori.0))),
    ///     ActNode(|dec: Decision| ActionResult { summary: dec.0, success: true }),
    /// );
    /// ```
    pub fn run_typed<Obs, Ori, Dec, Act>(
        &mut self,
        observe: Obs,
        orient: Ori,
        decide: Dec,
        act: Act,
    ) -> Option<OodaIteration>
    where
        Obs: PipelineNode<Input = ObserveInput, Output = Observation>,
        Ori: PipelineNode<Input = Observation, Output = Orientation>,
        Dec: PipelineNode<Input = Orientation, Output = Decision>,
        Act: PipelineNode<Input = Decision, Output = ActionResult>,
    {
        // Build the PolyConnector chain — type-checked by the compiler.
        let chain = observe.then(orient).then(decide).then(act);

        self.run_phases(|_phase, i| {
            let result = chain.execute(ObserveInput(i));
            let iteration = OodaIteration {
                phase: "typed".into(),
                observation: String::new(),
                orientation: String::new(),
                decision: String::new(),
                action: result.summary.clone(),
                success: result.success,
            };
            let next = if result.success { OodaPhase::Complete } else { OodaPhase::Failed("act failed".into()) };
            (iteration, next)
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_iter(i: u32, success: bool) -> OodaIteration {
        OodaIteration {
            phase: format!("phase-{}", i),
            observation: format!("obs-{}", i),
            orientation: format!("ori-{}", i),
            decision: format!("dec-{}", i),
            action: format!("act-{}", i),
            success,
        }
    }

    // --- Existing tests (must still pass) ---

    #[test]
    fn basic_cycle_execution() {
        let mut loop_ = OodaLoop::new(3);
        let last = loop_.run(|i| make_iter(i, true));
        assert_eq!(loop_.iterations().len(), 3);
        assert!(last.success);
        assert_eq!(last.phase, "phase-2");
    }

    #[test]
    fn max_iteration_enforcement() {
        let mut loop_ = OodaLoop::new(2);
        loop_.run(|i| make_iter(i, true));
        assert_eq!(loop_.iterations().len(), 2);
    }

    #[test]
    fn success_rate_calculation() {
        let mut loop_ = OodaLoop::new(5);
        loop_.run(|i| make_iter(i, i % 2 == 0)); // 3 successes, 2 failures
        assert!((loop_.success_rate() - 0.6).abs() < 1e-9);
    }

    #[test]
    fn zero_iterations() {
        let mut loop_ = OodaLoop::new(0);
        let last = loop_.run(|_| make_iter(0, true));
        assert!(loop_.iterations().is_empty());
        assert_eq!(loop_.success_rate(), 0.0);
        // last should be the default (empty fields, success=true)
        assert!(last.success);
        assert!(last.phase.is_empty());
    }

    #[test]
    fn phase_result_variants() {
        assert_eq!(PhaseResult::Continue, PhaseResult::Continue);
        assert_eq!(PhaseResult::Repeat, PhaseResult::Repeat);
        match PhaseResult::Terminate("done".into()) {
            PhaseResult::Terminate(msg) => assert_eq!(msg, "done"),
            _ => panic!("expected Terminate"),
        }
    }

    // --- New tests for the enhanced OODA ---

    #[test]
    fn ooda_phase_transitions_valid() {
        let idle = OodaPhase::Idle;
        assert!(idle.can_transition_to(&OodaPhase::Observing));
        assert!(!idle.can_transition_to(&OodaPhase::Deciding));

        let observing = OodaPhase::Observing;
        assert!(observing.can_transition_to(&OodaPhase::Orienting));
        assert!(!observing.can_transition_to(&OodaPhase::Acting));

        let acting = OodaPhase::Acting;
        assert!(acting.can_transition_to(&OodaPhase::Reviewing));

        let reviewing = OodaPhase::Reviewing;
        assert!(reviewing.can_transition_to(&OodaPhase::Complete));
        assert!(reviewing.can_transition_to(&OodaPhase::Observing)); // loop back
    }

    #[test]
    fn any_phase_can_fail() {
        let phases = [
            OodaPhase::Idle,
            OodaPhase::Observing,
            OodaPhase::Orienting,
            OodaPhase::Deciding,
            OodaPhase::Acting,
            OodaPhase::Reviewing,
            OodaPhase::Complete,
        ];
        for phase in &phases {
            assert!(
                phase.can_transition_to(&OodaPhase::Failed("reason".into())),
                "{:?} should be able to transition to Failed",
                phase
            );
        }
    }

    #[test]
    fn ooda_phase_introspection_derives_existing_transitions() {
        let transitions =
            <OodaPhase as crate::state_introspection::StateMachineIntrospection>::transition_descriptors();

        assert!(transitions.iter().any(|transition| {
            transition.source == "Idle"
                && transition.event == "GoToObserving"
                && transition.target == "Observing"
        }));
        assert!(transitions.iter().any(|transition| {
            transition.source == "Reviewing"
                && transition.event == "GoToObserving"
                && transition.target == "Observing"
        }));
        assert!(transitions.iter().any(|transition| {
            transition.source == "Complete"
                && transition.event == "GoToFailed"
                && transition.target == "Failed"
        }));
    }

    #[test]
    fn ooda_phase_renders_mermaid_and_s5_from_introspection() {
        let mermaid = ooda_phase_mermaid_state_diagram();
        let s5 = ooda_phase_s5();

        assert!(mermaid.starts_with("stateDiagram-v2\n"));
        assert!(mermaid.contains("[*] --> Idle"));
        assert!(mermaid.contains("Idle --> Observing: GoToObserving"));
        assert!(mermaid.contains("Complete --> [*]"));
        assert!(s5.starts_with("@machine id=OodaPhase initial=Idle datamodel=rust\n"));
        assert!(s5.contains("@state Reviewing:"));
        assert!(s5.contains("  -GoToObserving-> Observing"));
    }

    #[test]
    fn ooda_phase_next_forward() {
        assert_eq!(OodaPhase::Idle.next_forward(), Some(OodaPhase::Observing));
        assert_eq!(OodaPhase::Observing.next_forward(), Some(OodaPhase::Orienting));
        assert_eq!(OodaPhase::Orienting.next_forward(), Some(OodaPhase::Deciding));
        assert_eq!(OodaPhase::Deciding.next_forward(), Some(OodaPhase::Acting));
        assert_eq!(OodaPhase::Acting.next_forward(), Some(OodaPhase::Reviewing));
        assert_eq!(OodaPhase::Reviewing.next_forward(), Some(OodaPhase::Complete));
        assert_eq!(OodaPhase::Complete.next_forward(), None);
        assert_eq!(OodaPhase::Failed("x".into()).next_forward(), None);
    }

    #[test]
    fn guard_rails_defaults() {
        let rails = OodaGuardRails::default();
        assert_eq!(rails.max_iterations, 10);
        assert_eq!(rails.max_failures, 3);
        assert_eq!(rails.max_duration_secs, 3600);
        assert!(!rails.require_approval);
    }

    #[test]
    fn ooda_config_default() {
        let config = OodaConfig::default();
        assert!(!config.enable_autoresearch);
        assert_eq!(config.guard_rails.max_iterations, 10);
    }

    #[test]
    fn transition_to_valid() {
        let mut loop_ = OodaLoop::new(5);
        assert_eq!(*loop_.phase(), OodaPhase::Idle);
        assert!(loop_.transition_to(OodaPhase::Observing).is_ok());
        assert_eq!(*loop_.phase(), OodaPhase::Observing);
        assert!(loop_.transition_to(OodaPhase::Orienting).is_ok());
        assert!(loop_.transition_to(OodaPhase::Deciding).is_ok());
        assert!(loop_.transition_to(OodaPhase::Acting).is_ok());
        assert!(loop_.transition_to(OodaPhase::Reviewing).is_ok());
        assert!(loop_.transition_to(OodaPhase::Complete).is_ok());
    }

    #[test]
    fn transition_to_invalid() {
        let mut loop_ = OodaLoop::new(5);
        // Idle -> Deciding is invalid
        assert!(loop_.transition_to(OodaPhase::Deciding).is_err());
    }

    #[test]
    fn transition_to_failed() {
        let mut loop_ = OodaLoop::new(5);
        assert!(loop_.transition_to(OodaPhase::Failed("test".into())).is_ok());
        assert_eq!(*loop_.phase(), OodaPhase::Failed("test".into()));
    }

    #[test]
    fn run_phases_basic_cycle() {
        // Run enough iterations to complete a full Observe → Orient → Decide → Act → Review cycle
        let mut loop_ = OodaLoop::new(5);
        let result = loop_.run_phases(|phase, i| {
            let iter = OodaIteration {
                phase: format!("{:?}", phase),
                observation: format!("obs-{}", i),
                orientation: format!("ori-{}", i),
                decision: format!("dec-{}", i),
                action: format!("act-{}", i),
                success: true,
            };
            // Advance through the standard cycle
            let next = match phase {
                OodaPhase::Observing => OodaPhase::Orienting,
                OodaPhase::Orienting => OodaPhase::Deciding,
                OodaPhase::Deciding => OodaPhase::Acting,
                OodaPhase::Acting => OodaPhase::Reviewing,
                OodaPhase::Reviewing if i < 4 => OodaPhase::Observing,
                OodaPhase::Reviewing => OodaPhase::Complete,
                _ => OodaPhase::Failed("unexpected".into()),
            };
            (iter, next)
        });
        assert!(result.is_some());
        let last = result.unwrap();
        // After 5 iterations we should be in Reviewing (the last phase before Complete)
        assert_eq!(last.phase, format!("{:?}", OodaPhase::Reviewing));
    }

    #[test]
    fn run_phases_max_failures_guard() {
        let guard = OodaGuardRails {
            max_iterations: 10,
            max_failures: 2,
            ..OodaGuardRails::default()
        };
        let mut loop_ = OodaLoop::with_guard_rails(guard);
        let result = loop_.run_phases(|phase, i| {
            let iter = OodaIteration {
                phase: format!("{:?}", phase),
                observation: format!("obs-{}", i),
                orientation: String::new(),
                decision: String::new(),
                action: String::new(),
                success: false, // all fail
            };
            let next = phase.next_forward().unwrap_or(OodaPhase::Complete);
            (iter, next)
        });
        // Should terminate due to max_failures after 2 failures.
        // Each iteration is recorded (2 failed iters) plus one failure-record
        // iteration = 3 total.
        assert!(result.is_some());
        // 2 real failures + 1 failure-termination record
        assert_eq!(loop_.iterations().len(), 3);
        assert!(loop_.success_rate() < 0.5);
    }

    #[test]
    fn run_phases_invalid_transition_rejected() {
        let mut loop_ = OodaLoop::new(5);
        loop_.run_phases(|_phase, i| {
            let iter = OodaIteration {
                phase: String::new(),
                observation: format!("obs-{}", i),
                orientation: String::new(),
                decision: String::new(),
                action: String::new(),
                success: true,
            };
            // Try an invalid transition: first phase (Observing) -> Acting is invalid
            (iter, OodaPhase::Acting)
        });
        // First iteration is recorded, then the invalid transition appends a
        // failure-record iteration = 2 total in the history.
        assert_eq!(loop_.iterations().len(), 2);
        // The last phase should be Failed
        assert!(matches!(*loop_.phase(), OodaPhase::Failed(_)));
    }

    #[test]
    fn autoresearch_triggers_on_complex_observation() {
        let config = OodaConfig {
            enable_autoresearch: true,
            ..OodaConfig::default()
        };
        let mut loop_ = OodaLoop::from_config(config);

        let _ = loop_.run_phases(|phase, i| {
            let observation = if *phase == OodaPhase::Orienting {
                // Complex observation: >100 chars
                "A".repeat(120)
            } else {
                format!("obs-{}", i)
            };
            let iter = OodaIteration {
                phase: format!("{:?}", phase),
                observation,
                orientation: String::new(),
                decision: String::new(),
                action: String::new(),
                success: true,
            };
            let next = phase.next_forward().unwrap_or(OodaPhase::Complete);
            (iter, next)
        });

        // Check that at least one orientation entry contains the autoresearch marker
        let has_autoresearch = loop_
            .iterations()
            .iter()
            .any(|i| i.orientation.contains("autoresearch"));
        assert!(has_autoresearch, "Autoresearch should have been triggered");
    }

    #[test]
    fn autoresearch_not_triggered_for_short_observation() {
        let config = OodaConfig {
            enable_autoresearch: true,
            ..OodaConfig::default()
        };
        let mut loop_ = OodaLoop::from_config(config);

        let _ = loop_.run_phases(|phase, i| {
            let observation = if *phase == OodaPhase::Orienting {
                "short obs".to_string() // < 100 chars
            } else {
                format!("obs-{}", i)
            };
            let iter = OodaIteration {
                phase: format!("{:?}", phase),
                observation,
                orientation: String::new(),
                decision: String::new(),
                action: String::new(),
                success: true,
            };
            let next = phase.next_forward().unwrap_or(OodaPhase::Complete);
            (iter, next)
        });

        let has_autoresearch = loop_
            .iterations()
            .iter()
            .any(|i| i.orientation.contains("autoresearch"));
        assert!(!has_autoresearch, "Autoresearch should NOT have been triggered");
    }

    #[test]
    fn autoresearch_disabled_does_not_trigger() {
        let config = OodaConfig {
            enable_autoresearch: false, // disabled
            ..OodaConfig::default()
        };
        let mut loop_ = OodaLoop::from_config(config);

        let _ = loop_.run_phases(|phase, i| {
            let observation = if *phase == OodaPhase::Orienting {
                "A".repeat(120) // complex but autoresearch disabled
            } else {
                format!("obs-{}", i)
            };
            let iter = OodaIteration {
                phase: format!("{:?}", phase),
                observation,
                orientation: String::new(),
                decision: String::new(),
                action: String::new(),
                success: true,
            };
            let next = phase.next_forward().unwrap_or(OodaPhase::Complete);
            (iter, next)
        });

        let has_autoresearch = loop_
            .iterations()
            .iter()
            .any(|i| i.orientation.contains("autoresearch"));
        assert!(!has_autoresearch, "Autoresearch should NOT trigger when disabled");
    }

    #[test]
    fn from_config_honors_autoresearch() {
        let config = OodaConfig {
            enable_autoresearch: true,
            guard_rails: OodaGuardRails {
                max_iterations: 5,
                max_failures: 1,
                max_duration_secs: 60,
                require_approval: true,
            },
        };
        let loop_ = OodaLoop::from_config(config);
        assert!(loop_.enable_autoresearch);
        assert_eq!(loop_.guard_rails.max_iterations, 5);
        assert_eq!(loop_.guard_rails.max_failures, 1);
        assert_eq!(loop_.guard_rails.max_duration_secs, 60);
        assert!(loop_.guard_rails.require_approval);
    }

    #[test]
    fn reset_clears_state() {
        let mut loop_ = OodaLoop::new(3);
        loop_.run(|i| make_iter(i, true));
        assert_eq!(loop_.iterations().len(), 3);
        loop_.reset();
        assert_eq!(loop_.iterations().len(), 0);
        assert_eq!(*loop_.phase(), OodaPhase::Idle);
    }

    struct TestHomeDir {
        path: std::path::PathBuf,
    }

    impl TestHomeDir {
        fn new() -> Self {
            let mut path = std::env::temp_dir();
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after UNIX_EPOCH")
                .as_nanos();
            path.push(format!("b00t-ooda-test-{}-{}", std::process::id(), unique));
            std::fs::create_dir_all(&path).expect("temp home dir should be created");
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestHomeDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set_path(key: &'static str, value: &std::path::Path) -> Self {
            let original = std::env::var_os(key);
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.original {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    #[test]
    fn check_peer_handshake_no_file() {
        // When no handshake file exists, should return None.
        // 🤓 Force home resolution into a fresh temp dir so this test cannot observe a real ~/.b00t handshake.
        let test_home = TestHomeDir::new();
        let _home_guard = EnvVarGuard::set_path("HOME", test_home.path());
        let _userprofile_guard = EnvVarGuard::set_path("USERPROFILE", test_home.path());

        let result = check_peer_handshake();
        assert!(result.is_none());
    }

    #[test]
    fn check_peer_handshake_json_parsing() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("handshake.json");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, r#"{{"variant_id": "test-variant-42", "other": "data"}}"#).unwrap();
        let result = check_peer_handshake_inner(&[path]);
        assert_eq!(result.as_deref(), Some("test-variant-42"));
    }

    #[test]
    fn check_peer_handshake_plaintext_fallback() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("handshake.txt");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "variant_id: plain-text-variant").unwrap();
        let result = check_peer_handshake_inner(&[path]);
        assert_eq!(result.as_deref(), Some("plain-text-variant"));
    }

    #[test]
    fn ooda_iteration_from_phase() {
        let iter = OodaIteration::from_phase(&OodaPhase::Observing, "test obs", true);
        assert_eq!(iter.phase, "Observing");
        assert_eq!(iter.observation, "test obs");
        assert!(iter.success);
    }

    /// PolyConnector: run_typed() enforces Observe→Orient→Decide→Act type chain.
    #[test]
    fn run_typed_poly_connector_chain() {
        let mut loop_ = OodaLoop::new(3);

        let result = loop_.run_typed(
            ObserveNode(|i: ObserveInput| Observation(format!("obs-{}", i.0))),
            OrientNode(|obs: Observation| Orientation(format!("ori({})", obs.0))),
            DecideNode(|ori: Orientation| Decision(format!("dec({})", ori.0))),
            ActNode(|dec: Decision| ActionResult { summary: dec.0.clone(), success: true }),
        );

        assert!(result.is_some());
        let iter = result.unwrap();
        assert!(iter.success);
        assert!(iter.action.contains("dec(ori(obs-"));
    }
}
