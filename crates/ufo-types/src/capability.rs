//! Agent capability types — UFO-grounded generic capabilities extracted from
//! AgentSea skillpacks (vendor/agentsea-skillpacks/skillpacks/).
//!
//! Every agent capability maps to one of five fundamental domains:
//!
//! ```text
//! CapabilityDomain ──── OODA phase ──── AgentSea skill
//! ─────────────────────────────────────────────────────
//! Perceive             Observe          explore, img, select
//! Reason               Orient+Decide    select, learn
//! Act                  Act              task, chat
//! Verify               Verify           review, rating
//! Remember             (cross-cutting)  history, learn
//! ```
//!
//! # UFO grounding
//!
//! | Type              | Stereotype | Category  | AgentSea source        |
//! |-------------------|------------|-----------|------------------------|
//! | Task              | Kind       | Endurant  | task_old.py::Task      |
//! | Attempt           | Perdurant  | Event     | task_old.py::Attempt   |
//! | ActionRecord      | Perdurant  | Event     | base.py::ActionEvent   |
//! | Episode           | Kind       | Endurant  | base.py::Episode       |
//! | Review            | Relator    | Moment    | review.py::Review      |
//! | StateObservation  | Mode       | Moment    | base.py::EnvState      |
//! | Solution          | Kind       | Endurant  | task_old.py::Solution  |
//! | TrainingCorpus    | Kind       | Endurant  | task_old.py::TrainingSet|
//! | History           | Relator    | Moment    | history/base.py        |
//! | CapabilityDomain  | Kind       | Endurant  | (generic)              |

use serde::{Deserialize, Serialize};

use crate::stereotype::{Stereotyped, UfoStereotype};

// ══════════════════════════════════════════════════════════════════════════════
// CapabilityDomain — the five fundamental agent capability categories
// ══════════════════════════════════════════════════════════════════════════════

/// The five fundamental domains of agent capability. Every skill
/// (explore, review, learn, etc.) belongs to exactly one domain.
///
/// These map directly to OODA phases:
/// Perceive=Observe, Reason=Orient+Decide, Act=Act, Verify=Verify.
/// Remember is cross-cutting — it supports all phases.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CapabilityDomain {
    /// Sense the environment — gather observations.
    /// AgentSea skills: explore, img, select (visual)
    Perceive,

    /// Process information and make decisions.
    /// AgentSea skills: learn, select (decision)
    Reason,

    /// Execute actions in the environment.
    /// AgentSea skills: task, chat, action_opts
    Act,

    /// Check correctness and quality.
    /// AgentSea skills: review, rating
    Verify,

    /// Store, index, and recall information across episodes.
    /// AgentSea skills: history, learn (retention)
    Remember,
}

impl CapabilityDomain {
    /// The OODA phase this domain corresponds to.
    pub fn ooda_phase(&self) -> &str {
        match self {
            CapabilityDomain::Perceive => "Observe",
            CapabilityDomain::Reason => "Orient/Decide",
            CapabilityDomain::Act => "Act",
            CapabilityDomain::Verify => "Verify",
            CapabilityDomain::Remember => "Remember",
        }
    }

    /// AgentSea skills that belong to this domain.
    pub fn skills(&self) -> &[&str] {
        match self {
            CapabilityDomain::Perceive => &["explore", "img", "select/visual"],
            CapabilityDomain::Reason => &["select/decision", "learn/reason"],
            CapabilityDomain::Act => &["task", "chat", "action_opts"],
            CapabilityDomain::Verify => &["review", "rating"],
            CapabilityDomain::Remember => &["history", "learn/retain"],
        }
    }
}

impl std::fmt::Display for CapabilityDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityDomain::Perceive => write!(f, "Perceive"),
            CapabilityDomain::Reason => write!(f, "Reason"),
            CapabilityDomain::Act => write!(f, "Act"),
            CapabilityDomain::Verify => write!(f, "Verify"),
            CapabilityDomain::Remember => write!(f, "Remember"),
        }
    }
}

impl Stereotyped for CapabilityDomain {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("CapabilityDomain".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Task — the fundamental unit of agent work
// ══════════════════════════════════════════════════════════════════════════════

/// A task to be accomplished. Tasks are Endurant entities — they exist as
/// persistent units of work that can be attempted, completed, or abandoned.
///
/// Maps to AgentSea `task_old.py::Task` and b00t's task queue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task<G = String> {
    /// Unique task identifier.
    pub id: String,
    /// Human-readable description of the goal.
    pub description: String,
    /// The goal/constraint this task must satisfy.
    pub goal: G,
    /// The capability domain required to execute this task.
    pub domain: CapabilityDomain,
    /// Current task status.
    pub status: TaskStatus,
}

/// Task lifecycle states.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum TaskStatus {
    /// Task defined but not yet attempted.
    Defined,
    /// An attempt is in progress.
    InProgress,
    /// Task completed successfully.
    Completed,
    /// Task abandoned or failed.
    Abandoned,
}

impl Stereotyped for Task {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Task".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Attempt — a single try at a task
// ══════════════════════════════════════════════════════════════════════════════

/// An attempt to complete a task. Attempts are Perdurant — they unfold
/// over time as a sequence of actions.
///
/// Maps to AgentSea `task_old.py::Attempt`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Attempt {
    /// Unique attempt identifier.
    pub id: String,
    /// The task being attempted.
    pub task_id: String,
    /// Sequence of actions taken.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionRecord>,
    /// Current attempt status.
    pub status: AttemptStatus,
}

/// Attempt lifecycle states.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AttemptStatus {
    Created,
    InProgress,
    Finished,
    Errored,
}

impl Stereotyped for Attempt {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Attempt".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// ActionRecord — an atomic agent action
// ══════════════════════════════════════════════════════════════════════════════

/// A single action taken by an agent. Actions are Perdurant — atomic
/// events that change state.
///
/// Maps to AgentSea `base.py::ActionEvent` and `task_old.py::ActionEvent`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActionRecord {
    /// Unique action identifier.
    pub id: String,
    /// Name of the action (tool or method invoked).
    pub name: String,
    /// The capability domain this action belongs to.
    pub domain: CapabilityDomain,
    /// Input state before the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_state: Option<String>,
    /// Output/result after the action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Whether this action was reviewed and approved.
    #[serde(default)]
    pub reviewed: bool,
    /// Whether the review approved this action.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved: Option<bool>,
}

impl Stereotyped for ActionRecord {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("ActionRecord".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Episode — a sequence of actions toward a goal
// ══════════════════════════════════════════════════════════════════════════════

/// A composed sequence of actions forming a coherent unit of work.
/// Episodes are Endurant — they are persistent groupings.
///
/// Maps to AgentSea `base.py::Episode`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Episode {
    /// Unique episode identifier.
    pub id: String,
    /// The task this episode is attempting.
    pub task_id: String,
    /// The capability domain.
    pub domain: CapabilityDomain,
    /// Actions taken in this episode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionRecord>,
    /// Reviews of this episode.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviews: Vec<ReviewVerdict>,
}

impl Stereotyped for Episode {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Episode".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// ReviewVerdict — a review of an action or episode
// ══════════════════════════════════════════════════════════════════════════════

/// A review verdict. Reviews are Relators — they mediate between
/// a reviewer and the reviewed action/episode.
///
/// Maps to AgentSea `review.py::Review`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewVerdict {
    /// Unique review identifier.
    pub id: String,
    /// Who performed the review.
    pub reviewer: String,
    /// Whether the action was approved.
    pub approved: bool,
    /// Type of reviewer (human, agent, gate).
    pub reviewer_type: ReviewerType,
    /// Reason for approval or rejection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Suggested correction if rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correction: Option<String>,
}

/// Who or what performed the review.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReviewerType {
    Human,
    Agent,
    Gate,
}

impl Stereotyped for ReviewVerdict {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Relator("ReviewVerdict".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Solution — an accepted answer to a task
// ══════════════════════════════════════════════════════════════════════════════

/// An accepted solution to a task. Solutions are Endurant — they persist
/// as reusable artifacts (recipes, datums, fine-tune examples).
///
/// Maps to AgentSea `task_old.py::AcceptedSolution`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Solution {
    /// Unique solution identifier.
    pub id: String,
    /// The task this solves.
    pub task_id: String,
    /// The capability domain.
    pub domain: CapabilityDomain,
    /// The accepted action sequence.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<ActionRecord>,
    /// The compressed form (recipe name, datum key, etc.).
    pub compressed_form: String,
    /// Carmack memoization: how many times this solution was recalled
    /// without re-solving.
    #[serde(default)]
    pub recall_count: u64,
}

impl Stereotyped for Solution {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("Solution".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// TrainingCorpus — a collection of solutions for fine-tuning
// ══════════════════════════════════════════════════════════════════════════════

/// A training corpus composed of accepted solutions. The corpus is
/// the ultimate Carmack cache — fine-tuned model weights.
///
/// Maps to AgentSea `task_old.py::TrainingSet`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrainingCorpus {
    /// Unique corpus identifier.
    pub id: String,
    /// The capability domain this corpus trains.
    pub domain: CapabilityDomain,
    /// Solutions that form the training examples.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub solutions: Vec<Solution>,
    /// Number of episodes that contributed to this corpus.
    #[serde(default)]
    pub episode_count: u64,
}

impl Stereotyped for TrainingCorpus {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Kind("TrainingCorpus".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// History — chronological record of actions and reviews
// ══════════════════════════════════════════════════════════════════════════════

/// A chronological record of events. History is a Relator — it connects
/// past states to present understanding.
///
/// Maps to AgentSea `history/base.py::History`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct History {
    /// Chronological events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<ActionRecord>,
    /// Reviews applied to events.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reviews: Vec<ReviewVerdict>,
}

impl Stereotyped for History {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Relator("History".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// AgentCapability trait — what every skill implements
// ══════════════════════════════════════════════════════════════════════════════

/// The generic capability contract. Every AgentSea skill (explore, review,
/// learn, etc.) implements this trait at the Rust type level.
pub trait AgentCapability {
    /// The domain this capability belongs to.
    fn domain(&self) -> CapabilityDomain;

    /// Execute this capability, producing an action record.
    fn execute(&self, input: &str) -> ActionRecord;

    /// The UFO stereotype for this capability instance.
    fn stereotype(&self) -> UfoStereotype;
}

// ══════════════════════════════════════════════════════════════════════════════
// StateObservation — environment state snapshot
// ══════════════════════════════════════════════════════════════════════════════

/// A snapshot of the environment state. StateObservations are Modes —
/// intrinsic qualities of the environment at a point in time.
///
/// Maps to AgentSea `state.py::EnvState`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StateObservation {
    /// The capability domain this observation belongs to.
    pub domain: CapabilityDomain,
    /// Serialized state data.
    pub data: String,
    /// Timestamp of observation (Unix epoch seconds).
    #[serde(default)]
    pub timestamp: f64,
}

impl Stereotyped for StateObservation {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Mode("StateObservation".into())
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Carmack entanglement — energy-budgeted capability execution
// ══════════════════════════════════════════════════════════════════════════════

/// Carmack's energy budget for a capability invocation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct EnergyBudget {
    /// GPU watt-seconds consumed by LLM inference.
    pub gpu_watt_seconds: f64,
    /// CPU-only operations (recipes, tests, grep).
    pub cpu_operations: u64,
    /// Number of times a memoized solution was recalled instead of re-solved.
    pub memoization_hits: u64,
    /// Number of times the LLM was invoked (non-deterministic).
    pub llm_invocations: u64,
}

impl Default for EnergyBudget {
    fn default() -> Self {
        Self {
            gpu_watt_seconds: 0.0,
            cpu_operations: 0,
            memoization_hits: 0,
            llm_invocations: 0,
        }
    }
}

impl EnergyBudget {
    /// The Carmack efficiency ratio: memoized / total solves.
    /// 1.0 = all solves were cache hits (zero LLM calls since first solve).
    /// 0.0 = every solve required a fresh LLM call.
    pub fn efficiency_ratio(&self) -> f64 {
        let total_solves = self.memoization_hits + self.llm_invocations;
        if total_solves == 0 {
            return 1.0;
        }
        (self.memoization_hits as f64 / total_solves as f64).min(1.0)
    }
}

/// A Carmack-tracked solution — wraps a Solution with energy accounting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CarmackSolution {
    /// The accepted solution.
    pub solution: Solution,
    /// Energy budget for the ORIGINAL solve (first time).
    pub first_solve_budget: EnergyBudget,
    /// How many times this was recalled without re-solving.
    pub recall_count: u64,
    /// Estimated total energy saved by memoization.
    pub energy_saved: f64,
}

impl CarmackSolution {
    /// Create a new Carmack solution from a first-time solve.
    pub fn from_first_solve(solution: Solution, budget: EnergyBudget) -> Self {
        Self {
            solution,
            first_solve_budget: budget,
            recall_count: 0,
            energy_saved: 0.0,
        }
    }

    /// Record a recall — the solution was used without LLM invocation.
    pub fn recall(&mut self) {
        self.recall_count += 1;
        self.energy_saved += self.first_solve_budget.gpu_watt_seconds;
    }

    /// Total effective cost including first solve minus energy saved.
    pub fn effective_cost(&self) -> f64 {
        (self.first_solve_budget.gpu_watt_seconds - self.energy_saved).max(0.0)
    }
}

impl Stereotyped for CarmackSolution {
    fn ufo_stereotype(&self) -> UfoStereotype {
        UfoStereotype::Relator("CarmackSolution".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_domain_ufo_grounding() {
        assert_eq!(
            CapabilityDomain::Perceive.ufo_stereotype().to_string(),
            "Kind:CapabilityDomain"
        );
    }

    #[test]
    fn five_domains_cover_all_ooda() {
        let domains = [
            CapabilityDomain::Perceive,
            CapabilityDomain::Reason,
            CapabilityDomain::Act,
            CapabilityDomain::Verify,
            CapabilityDomain::Remember,
        ];
        assert_eq!(domains.len(), 5);
        // Ensure no duplicates
        let mut set = std::collections::HashSet::new();
        for d in &domains {
            assert!(set.insert(*d), "duplicate domain: {d}");
        }
    }

    #[test]
    fn domain_ooda_mapping() {
        assert_eq!(CapabilityDomain::Perceive.ooda_phase(), "Observe");
        assert_eq!(CapabilityDomain::Act.ooda_phase(), "Act");
        assert_eq!(CapabilityDomain::Verify.ooda_phase(), "Verify");
    }

    #[test]
    fn domain_skills_non_empty() {
        for domain in &[
            CapabilityDomain::Perceive,
            CapabilityDomain::Reason,
            CapabilityDomain::Act,
            CapabilityDomain::Verify,
            CapabilityDomain::Remember,
        ] {
            assert!(!domain.skills().is_empty(), "{domain} has no skills");
        }
    }

    #[test]
    fn task_lifecycle() {
        let t = Task {
            id: "task-1".into(),
            description: "test".into(),
            goal: "test goal".into(),
            domain: CapabilityDomain::Act,
            status: TaskStatus::Defined,
        };
        assert_eq!(t.ufo_stereotype().to_string(), "Kind:Task");
    }

    #[test]
    fn review_is_relator() {
        let r = ReviewVerdict {
            id: "r1".into(),
            reviewer: "carmack".into(),
            approved: true,
            reviewer_type: ReviewerType::Agent,
            reason: None,
            correction: None,
        };
        assert_eq!(r.ufo_stereotype().to_string(), "Relator:ReviewVerdict");
    }

    #[test]
    fn solution_has_compressed_form() {
        let s = Solution {
            id: "s1".into(),
            task_id: "t1".into(),
            domain: CapabilityDomain::Act,
            actions: vec![],
            compressed_form: "just submodule-status".into(),
            recall_count: 42,
        };
        assert!(s.compressed_form.contains("just"));
        assert_eq!(s.recall_count, 42);
    }

    #[test]
    fn training_corpus_aggregates_solutions() {
        let solutions = vec![Solution {
            id: "s1".into(),
            task_id: "t1".into(),
            domain: CapabilityDomain::Act,
            actions: vec![],
            compressed_form: "just tidy".into(),
            recall_count: 0,
        }];
        let corpus = TrainingCorpus {
            id: "corpus-1".into(),
            domain: CapabilityDomain::Act,
            solutions,
            episode_count: 5,
        };
        assert_eq!(corpus.ufo_stereotype().to_string(), "Kind:TrainingCorpus");
        assert_eq!(corpus.episode_count, 5);
    }

    #[test]
    fn history_is_relator() {
        let h = History {
            events: vec![],
            reviews: vec![],
        };
        assert_eq!(h.ufo_stereotype().to_string(), "Relator:History");
    }

    #[test]
    fn state_observation_is_mode() {
        let s = StateObservation {
            domain: CapabilityDomain::Perceive,
            data: "{}".into(),
            timestamp: 1.0,
        };
        assert_eq!(s.ufo_stereotype().to_string(), "Mode:StateObservation");
    }

    // ── Carmack entanglement tests ──

    #[test]
    fn energy_budget_efficiency_perfect() {
        let b = EnergyBudget {
            memoization_hits: 10,
            llm_invocations: 0,
            ..Default::default()
        };
        assert_eq!(b.efficiency_ratio(), 1.0);
    }

    #[test]
    fn energy_budget_efficiency_mixed() {
        let b = EnergyBudget {
            memoization_hits: 7,
            llm_invocations: 3,
            ..Default::default()
        };
        let r = b.efficiency_ratio();
        assert!((r - 0.7).abs() < 0.01, "expected ~0.7, got {r}");
    }

    #[test]
    fn carmack_solution_recall_saves_energy() {
        let solution = Solution {
            id: "s1".into(),
            task_id: "t1".into(),
            domain: CapabilityDomain::Act,
            actions: vec![],
            compressed_form: "just submodule-status".into(),
            recall_count: 0,
        };
        let budget = EnergyBudget {
            gpu_watt_seconds: 0.3,
            cpu_operations: 1,
            llm_invocations: 1,
            ..Default::default()
        };
        let mut cs = CarmackSolution::from_first_solve(solution, budget);
        assert_eq!(cs.recall_count, 0);
        assert_eq!(cs.energy_saved, 0.0);

        cs.recall();
        assert_eq!(cs.recall_count, 1);
        assert_eq!(cs.energy_saved, 0.3);

        cs.recall();
        assert_eq!(cs.recall_count, 2);
        assert_eq!(cs.energy_saved, 0.6);
        assert_eq!(cs.effective_cost(), 0.0); // saved more than cost
    }

    #[test]
    fn carmack_solution_is_relator() {
        let cs = CarmackSolution::from_first_solve(
            Solution {
                id: "s1".into(),
                task_id: "t1".into(),
                domain: CapabilityDomain::Act,
                actions: vec![],
                compressed_form: "just tidy".into(),
                recall_count: 0,
            },
            EnergyBudget::default(),
        );
        assert_eq!(
            cs.ufo_stereotype().to_string(),
            "Relator:CarmackSolution"
        );
    }
}
