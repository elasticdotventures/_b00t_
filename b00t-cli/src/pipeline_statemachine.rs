//! Formally verifiable, version-controllable, serializable pipeline state machine.
//!
//! Models the pipeline lifecycle as a finite state machine with explicit,
//! auditable transitions. Every state change is recorded in an append-only
//! history for traceability, replay, and post-mortem analysis.
//!
//! # State diagram
//!
//! ```text
//!                ┌─────────────────────────────────────┐
//!                │            Validate (restart)        │
//!                v                                     │
//! ┌──────┐  Validate  ┌────────────┐  Schedule  ┌────────────┐
//! │ Idle │ ────────→  │ Validating │ ────────→  │ Scheduling │
//! └──────┘            └────────────┘            └────────────┘
//!   ↑                    │  │                       │       │
//!   │                    │  │ (StageFailed)         │       │
//!   │                    │  v                       │       │
//!   │  Validate          │ ┌────────┐               │       │
//!   │  (restart)         │ │ Failed │ ←──────────── │       │
//!   │                    │ └────────┘               │       │
//!   │                    │     ↑                    │       │
//!   │                    │     │ (Cancel)            │       │
//!   │                    │     │                    │       │
//!   │                    │  ┌────────┐              │       │
//!   │                    │  │ Paused │ ←── (Pause)  │       │
//!   │                    │  └────────┘              │       │
//!   │                    │     │                     │       │
//!   │   ┌───────────┐    │     │ (Resume)            │       │
//!   │   │ Completed │    │     v                     │       │
//!   │   └───────────┘    │ ┌──────────┐             │       │
//!   │         ↑          │ │ Running  │ ←── Execute  │       │
//!   │         │          │ └──────────┘             │       │
//!   │         │          │    │    ↑                │       │
//!   │         │          │    │    │                │       │
//!   │         └──────────│────┘    └────────────────│       │
//!   │      StageComplete │      (StageComplete)     │       │
//!   │      (last stage)  │                          │       │
//!   └────────────────────┘                          │       │
//!               (StageFailed) ──────────────────────┘       │
//!               (StageFailed) ──────────────────────────────┘
//! ```

use crate::pipeline_types::{PipelineDag, PipelineError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── GH #743: Pipeline state machine ──

/// All possible states of a pipeline execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineState {
    /// Initial state — awaiting validation trigger.
    Idle,
    /// Input validation in progress.
    Validating,
    /// Resource scheduling / DAG wiring in progress.
    Scheduling,
    /// Execution active on stage `n` (0-indexed stage counter).
    Running(u32),
    /// Execution paused at stage `n`.
    Paused(u32),
    /// Terminal failure with diagnostic error.
    Failed(PipelineError),
    /// Successful completion — all stages finished.
    Completed,
}

/// Events that drive state transitions within the pipeline state machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PipelineEvent {
    /// Begin validation; also resets from `Failed` or `Completed` for a re-run.
    Validate,
    /// Validation passed; proceed to resource scheduling.
    Schedule,
    /// Execute the scheduled pipeline (stages at index 0).
    Execute,
    /// Request a pause at the current stage.
    Pause,
    /// Resume execution from a paused state.
    Resume,
    /// Cancel execution — transitions `Paused` → `Failed`.
    Cancel,
    /// Stage `n` completed successfully.  When `n` is the final stage,
    /// the machine transitions to `Completed`.
    StageComplete(u32),
    /// Stage failed with the given pipeline error.
    StageFailed(PipelineError),
    /// Request retry of stage `n`.
    Retry(u32),
}

/// A finite state machine for pipeline lifecycle management.
///
/// Wraps the current state, an append-only transition history (every
/// transition is recorded with a [`DateTime<Utc>`] timestamp), the
/// pipeline [`PipelineDag`] (used for stage-count awareness), and an
/// optional run identifier.
///
/// # State transitions
///
/// | Current State | Event | Next State |
/// |---|---|---|
/// | `Idle` | `Validate` | `Validating` |
/// | `Validating` | `Schedule` | `Scheduling` |
/// | `Validating` | `StageFailed` | `Failed` |
/// | `Scheduling` | `Execute` | `Running(0)` |
/// | `Scheduling` | `StageFailed` | `Failed` |
/// | `Running(n)` | `StageComplete(n)` | `Running(n+1)` or `Completed` |
/// | `Running(n)` | `StageFailed` | `Failed` |
/// | `Running(n)` | `Pause` | `Paused(n)` |
/// | `Paused(n)` | `Resume` | `Running(n)` |
/// | `Paused(n)` | `Cancel` | `Failed` |
/// | `Failed` | `Validate` | `Idle` |
/// | `Completed` | `Validate` | `Idle` |
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateMachine {
    /// Current pipeline state.
    current: PipelineState,
    /// Append-only log of `(from_state, event, timestamp)` triples.
    history: Vec<(PipelineState, PipelineEvent, DateTime<Utc>)>,
    /// The pipeline DAG this machine manages.
    dag: PipelineDag,
    /// Optional external run identifier for traceability.
    run_id: Option<String>,
}

impl StateMachine {
    /// Create a new state machine in the [`PipelineState::Idle`] state
    /// for the given [`PipelineDag`].
    pub fn new(dag: PipelineDag) -> Self {
        Self {
            current: PipelineState::Idle,
            history: Vec::new(),
            dag,
            run_id: None,
        }
    }

    /// Attempt a state transition driven by `event`.
    ///
    /// On success, the new state is recorded in the transition history
    /// and returned.  On failure (invalid transition for the current
    /// state), an error string is returned describing the violation.
    pub fn transition(&mut self, event: PipelineEvent) -> Result<PipelineState, String> {
        if !self.can_transition(&event) {
            return Err(format!(
                "invalid transition: {:?} → {:?}",
                self.current, event
            ));
        }

        let next_state = match (&self.current, &event) {
            // ── Idle ─────────────────────────────────────────────────
            (PipelineState::Idle, PipelineEvent::Validate) => PipelineState::Validating,

            // ── Validating ───────────────────────────────────────────
            (PipelineState::Validating, PipelineEvent::Schedule) => PipelineState::Scheduling,
            (PipelineState::Validating, PipelineEvent::StageFailed(err)) => {
                PipelineState::Failed(err.clone())
            }

            // ── Scheduling ───────────────────────────────────────────
            (PipelineState::Scheduling, PipelineEvent::Execute) => PipelineState::Running(0),
            (PipelineState::Scheduling, PipelineEvent::StageFailed(err)) => {
                PipelineState::Failed(err.clone())
            }

            // ── Running ──────────────────────────────────────────────
            (PipelineState::Running(n), PipelineEvent::StageComplete(m)) if n == m => {
                let next = n + 1;
                if next >= self.dag.stages.len() as u32 {
                    PipelineState::Completed
                } else {
                    PipelineState::Running(next)
                }
            }
            (PipelineState::Running(_), PipelineEvent::StageFailed(err)) => {
                PipelineState::Failed(err.clone())
            }
            (PipelineState::Running(n), PipelineEvent::Pause) => PipelineState::Paused(*n),

            // ── Paused ───────────────────────────────────────────────
            (PipelineState::Paused(n), PipelineEvent::Resume) => PipelineState::Running(*n),
            (PipelineState::Paused(_), PipelineEvent::Cancel) => {
                PipelineState::Failed(PipelineError::InputValidation("cancelled".into()))
            }

            // ── Failed / Completed — restart ─────────────────────────
            (PipelineState::Failed(_), PipelineEvent::Validate) => PipelineState::Idle,
            (PipelineState::Completed, PipelineEvent::Validate) => PipelineState::Idle,

            // All other combinations are rejected by `can_transition` above.
            _ => unreachable!(),
        };

        let timestamp = Utc::now();
        self.history
            .push((self.current.clone(), event, timestamp));
        self.current = next_state.clone();
        Ok(next_state)
    }

    /// Check whether `event` is a valid transition from the current state.
    pub fn can_transition(&self, event: &PipelineEvent) -> bool {
        match (&self.current, event) {
            (PipelineState::Idle, PipelineEvent::Validate) => true,
            (PipelineState::Validating, PipelineEvent::Schedule) => true,
            (PipelineState::Validating, PipelineEvent::StageFailed(_)) => true,
            (PipelineState::Scheduling, PipelineEvent::Execute) => true,
            (PipelineState::Scheduling, PipelineEvent::StageFailed(_)) => true,
            (PipelineState::Running(n), PipelineEvent::StageComplete(m)) => n == m,
            (PipelineState::Running(_), PipelineEvent::StageFailed(_)) => true,
            (PipelineState::Running(_), PipelineEvent::Pause) => true,
            (PipelineState::Paused(_), PipelineEvent::Resume) => true,
            (PipelineState::Paused(_), PipelineEvent::Cancel) => true,
            (PipelineState::Failed(_), PipelineEvent::Validate) => true,
            (PipelineState::Completed, PipelineEvent::Validate) => true,
            _ => false,
        }
    }

    /// Borrow the current pipeline state.
    pub fn state(&self) -> &PipelineState {
        &self.current
    }

    /// Borrow the full transition history slice.
    pub fn history(&self) -> &[(PipelineState, PipelineEvent, DateTime<Utc>)] {
        &self.history
    }

    /// Return a lightweight, serializable snapshot of the machine's state.
    pub fn snapshot(&self) -> StateSnapshot {
        StateSnapshot {
            current_state: self.current.clone(),
            run_id: self.run_id.clone(),
            history_len: self.history.len(),
            timestamp: Utc::now(),
        }
    }
}

/// A lightweight, serializable snapshot of the pipeline state machine.
///
/// Provides a point-in-time summary without exposing the full history
/// or the DAG — suitable for API responses and telemetry events.
#[derive(Debug, Clone, Serialize)]
pub struct StateSnapshot {
    pub current_state: PipelineState,
    pub run_id: Option<String>,
    pub history_len: usize,
    pub timestamp: DateTime<Utc>,
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_types::{
        CapsuleProfile, PipelineDag, ResourceRequirements, StageSpec,
    };

    /// Build a sequential DAG with `count` stages (no ports — just identity).
    fn make_dag(count: u32) -> PipelineDag {
        let stages: Vec<StageSpec> = (0..count)
            .map(|i| StageSpec {
                name: format!("stage_{i}"),
                profile: CapsuleProfile {
                    name: format!("stage_{i}"),
                    ports: vec![],
                    resources: ResourceRequirements {
                        min_ram_gb: 0.0,
                        min_vram_gb: 0.0,
                        requires_gpu: false,
                        cpu_cores: None,
                        scratch_disk_gb: None,
                    },
                    image: None,
                    timeout_seconds: None,
                },
                input_ports: vec![],
                output_ports: vec![],
                error_routes: vec![],
                env: None,
                checkpoint_interval_seconds: None,
                secret_refs: None,
                flow_control: None,
            })
            .collect();
        PipelineDag::from_sequential(stages)
    }

    /// Drive the machine through Validate → Schedule → Execute so it
    /// lands on `Running(0)`.
    fn to_running0(machine: &mut StateMachine) {
        machine.transition(PipelineEvent::Validate).unwrap();
        machine.transition(PipelineEvent::Schedule).unwrap();
        machine.transition(PipelineEvent::Execute).unwrap();
    }

    // ── Happy path ──────────────────────────────────────────────────────

    #[test]
    fn happy_path_idle_to_completed() {
        let dag = make_dag(3);
        let mut sm = StateMachine::new(dag);

        assert_eq!(*sm.state(), PipelineState::Idle);

        // Idle → Validating
        let state = sm.transition(PipelineEvent::Validate).unwrap();
        assert_eq!(state, PipelineState::Validating);
        assert_eq!(*sm.state(), PipelineState::Validating);

        // Validating → Scheduling
        let state = sm.transition(PipelineEvent::Schedule).unwrap();
        assert_eq!(state, PipelineState::Scheduling);
        assert_eq!(*sm.state(), PipelineState::Scheduling);

        // Scheduling → Running(0)
        let state = sm.transition(PipelineEvent::Execute).unwrap();
        assert_eq!(state, PipelineState::Running(0));
        assert_eq!(*sm.state(), PipelineState::Running(0));

        // Running(0) → Running(1)
        let state = sm.transition(PipelineEvent::StageComplete(0)).unwrap();
        assert_eq!(state, PipelineState::Running(1));

        // Running(1) → Running(2)
        let state = sm.transition(PipelineEvent::StageComplete(1)).unwrap();
        assert_eq!(state, PipelineState::Running(2));

        // Running(2) → Completed (last of 3 stages)
        let state = sm.transition(PipelineEvent::StageComplete(2)).unwrap();
        assert_eq!(state, PipelineState::Completed);

        // History has 6 entries, one per transition() call: Validate,
        // Schedule, Execute, StageComplete(0), StageComplete(1),
        // StageComplete(2) — confirmed one-entry-per-call by the sibling
        // history_tracks_all_transitions test (#822 — this used to assert
        // 7, off by one from its own 6-item breakdown above).
        assert_eq!(sm.history().len(), 6);
    }

    // ── Pause / Resume cycle ────────────────────────────────────────────

    #[test]
    fn pause_resume_cycle() {
        let dag = make_dag(5);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);

        // Running(0) → Paused(0)
        let state = sm.transition(PipelineEvent::Pause).unwrap();
        assert_eq!(state, PipelineState::Paused(0));
        assert_eq!(*sm.state(), PipelineState::Paused(0));

        // Paused(0) → Running(0)
        let state = sm.transition(PipelineEvent::Resume).unwrap();
        assert_eq!(state, PipelineState::Running(0));
        assert_eq!(*sm.state(), PipelineState::Running(0));

        // Two more transitions recorded
        assert_eq!(sm.history().len(), 5);
    }

    // ── Cancel from Paused → Failed ────────────────────────────────────

    #[test]
    fn cancel_from_paused_transitions_to_failed() {
        let dag = make_dag(3);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);

        sm.transition(PipelineEvent::Pause).unwrap();
        let state = sm.transition(PipelineEvent::Cancel).unwrap();

        assert_eq!(
            state,
            PipelineState::Failed(PipelineError::InputValidation("cancelled".into()))
        );
        assert_eq!(
            *sm.state(),
            PipelineState::Failed(PipelineError::InputValidation("cancelled".into()))
        );
    }

    // ── Stage failure transitions to Failed ─────────────────────────────

    #[test]
    fn stage_failure_transitions_to_failed() {
        let dag = make_dag(3);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);

        let err = PipelineError::StageCrashed("out of memory".into());
        let state = sm.transition(PipelineEvent::StageFailed(err.clone())).unwrap();
        assert_eq!(state, PipelineState::Failed(err));
    }

    // ── Invalid transition returns error ────────────────────────────────

    #[test]
    fn invalid_transition_returns_error() {
        let dag = make_dag(1);
        let mut sm = StateMachine::new(dag);

        // Idle → StageComplete — wrong event for current state
        let result = sm.transition(PipelineEvent::StageComplete(0));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid transition"));

        // Running(0) → Schedule — wrong event for current state
        sm.transition(PipelineEvent::Validate).unwrap();
        sm.transition(PipelineEvent::Schedule).unwrap();
        sm.transition(PipelineEvent::Execute).unwrap();
        let result = sm.transition(PipelineEvent::Schedule);
        assert!(result.is_err());

        // Running(0) with StageComplete(1) — mismatched stage index
        let result = sm.transition(PipelineEvent::StageComplete(1));
        assert!(result.is_err());
    }

    // ── Serialization round-trip ────────────────────────────────────────

    #[test]
    fn serialize_deserialize_round_trip() {
        let dag = make_dag(2);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);
        sm.transition(PipelineEvent::StageComplete(0)).unwrap();
        sm.transition(PipelineEvent::StageComplete(1)).unwrap();

        let json = serde_json::to_string(&sm).unwrap();
        let deserialized: StateMachine = serde_json::from_str(&json).unwrap();

        assert_eq!(*deserialized.state(), PipelineState::Completed);
        assert_eq!(deserialized.history().len(), 5);
    }

    // ── History tracks all transitions ──────────────────────────────────

    #[test]
    fn history_tracks_all_transitions() {
        let dag = make_dag(3);
        let mut sm = StateMachine::new(dag);

        assert_eq!(sm.history().len(), 0);

        sm.transition(PipelineEvent::Validate).unwrap();
        assert_eq!(sm.history().len(), 1);

        sm.transition(PipelineEvent::Schedule).unwrap();
        assert_eq!(sm.history().len(), 2);

        sm.transition(PipelineEvent::Execute).unwrap();
        assert_eq!(sm.history().len(), 3);

        // Each history entry records (from_state, event, timestamp)
        let (from, ev, _ts) = &sm.history()[0];
        assert_eq!(*from, PipelineState::Idle);
        assert_eq!(*ev, PipelineEvent::Validate);
    }

    // ── Snapshot ────────────────────────────────────────────────────────

    #[test]
    fn snapshot_captures_current_state() {
        let dag = make_dag(2);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);

        let snap = sm.snapshot();
        assert_eq!(snap.current_state, PipelineState::Running(0));
        assert!(snap.run_id.is_none());
        assert_eq!(snap.history_len, 3);
    }

    // ── Restart from Failed ─────────────────────────────────────────────

    #[test]
    fn restart_from_failed() {
        let dag = make_dag(3);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);

        // Trigger failure
        let err = PipelineError::StageCrashed("unexpected crash".into());
        sm.transition(PipelineEvent::StageFailed(err)).unwrap();

        // Failed → Idle via Validate
        let state = sm.transition(PipelineEvent::Validate).unwrap();
        assert_eq!(state, PipelineState::Idle);

        // Re-run the pipeline
        sm.transition(PipelineEvent::Validate).unwrap();
        assert_eq!(*sm.state(), PipelineState::Validating);
    }

    // ── Restart from Completed ──────────────────────────────────────────

    #[test]
    fn restart_from_completed() {
        let dag = make_dag(1);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);
        sm.transition(PipelineEvent::StageComplete(0)).unwrap();
        assert_eq!(*sm.state(), PipelineState::Completed);

        // Completed → Idle → re-run
        let state = sm.transition(PipelineEvent::Validate).unwrap();
        assert_eq!(state, PipelineState::Idle);

        sm.transition(PipelineEvent::Validate).unwrap();
        assert_eq!(*sm.state(), PipelineState::Validating);
    }

    // ── Validate from Failed goes to Idle first ─────────────────────────

    #[test]
    fn validate_from_failed_goes_to_idle() {
        let dag = make_dag(2);
        let mut sm = StateMachine::new(dag);
        to_running0(&mut sm);

        sm.transition(PipelineEvent::StageFailed(PipelineError::InputValidation("err".into())))
            .unwrap();
        assert!(matches!(*sm.state(), PipelineState::Failed(_)));

        // Failed → Idle
        sm.transition(PipelineEvent::Validate).unwrap();
        assert_eq!(*sm.state(), PipelineState::Idle);
    }
}
