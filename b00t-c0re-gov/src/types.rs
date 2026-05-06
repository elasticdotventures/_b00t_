use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Result of a governance gate check.
/// Agent NEVER blocks — if Hook is returned, agent snapshots and goes productive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GateResult {
    Allow,
    Deny {
        reason: String,
        escalation_path: Option<EscalationPath>,
    },
    Hook(HookToken),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EscalationPath {
    Human { channel: String, message: String },
    Supervisor { agent_id: String },
    WaitForHook { token: HookToken },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookToken {
    pub id: Uuid,
    pub hook_type: HookType,
    pub created_at: DateTime<Utc>,
    pub ttl_ms: Option<u64>,  // None = effectively infinite (2 years)
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HookType {
    TimerMs(u64),           // Fire after N milliseconds
    AtTimestamp(i64),       // Fire at Unix timestamp
    Event(String),          // Fire when event_id is emitted
    AnyOf(Vec<HookToken>),  // Fire when ANY child fires
    AllOf(Vec<HookToken>),  // Fire when ALL children fire
    Cron(String),           // Cron expression
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentContext {
    pub agent_id: String,
    pub task: String,
    pub gate: String,
    pub result_so_far: serde_json::Value,
    pub reasoning: String,
    pub created_at: DateTime<Utc>,
    pub hook_token: HookToken,
    pub continuation: String,  // What to do next — agent resumes here
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookNotification {
    pub hook_id: Uuid,
    pub event: HookEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HookEvent {
    Fired,
    Cancelled,
    Expired,
    Error(String),
}

/// Multi-dimensional scoring for mission completion.
/// Each dimension is scored 0.0 - 1.0.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreCard {
    pub roi: f64,          // Return on investment (cake earned / cake spent)
    pub cost: f64,         // Calorie efficiency (lower calories = higher score)
    pub time: f64,         // Time to complete (faster = higher)
    pub accuracy: f64,     // Correctness of solution
    pub utility: f64,      // Reusability / applied value
    pub risk: f64,         // Risk level (lower risk = higher score)
}

impl ScoreCard {
    pub fn new(roi: f64, cost: f64, time: f64, accuracy: f64, utility: f64, risk: f64) -> Self {
        Self { roi, cost, time, accuracy, utility, risk }
    }

    /// Weighted sum across all dimensions.
    /// Default weights: roi=1.0, cost=1.0, time=0.8, accuracy=0.9, utility=0.7, risk=0.6
    pub fn weighted_score(&self) -> f64 {
        let weights = [1.0, 1.0, 0.8, 0.9, 0.7, 0.6];
        self.weighted_score_with(&weights)
    }

    pub fn weighted_score_with(&self, weights: &[f64; 6]) -> f64 {
        let values = [self.roi, self.cost, self.time, self.accuracy, self.utility, self.risk];
        let weighted_sum: f64 = values.iter().zip(weights.iter()).map(|(v, w)| v * w).sum();
        let weight_sum: f64 = weights.iter().sum();
        if weight_sum == 0.0 {
            return 0.0;
        }
        (weighted_sum / weight_sum).clamp(0.0, 1.0)
    }

    /// Cake payout = weighted_score * base_bounty
    pub fn cake_payout(&self, base_bounty: f64) -> f64 {
        self.weighted_score() * base_bounty
    }
}

/// Calorie balance — tracks cognitive energy.
/// Agent with 0 calories is dead (☠️).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalorieBalance {
    pub current: f64,
    pub total_burned: f64,
    pub total_earned: f64,
}

impl CalorieBalance {
    pub fn new(initial: f64) -> Self {
        Self {
            current: initial,
            total_burned: 0.0,
            total_earned: initial,
        }
    }

    /// Burn calories for an action. Returns false if insufficient.
    pub fn burn(&mut self, amount: f64) -> bool {
        if self.current < amount {
            return false;
        }
        self.current -= amount;
        self.total_burned += amount;
        true
    }

    /// Earn calories (from cake conversion or grants).
    pub fn earn(&mut self, amount: f64) {
        self.current += amount;
        self.total_earned += amount;
    }

    pub fn is_dead(&self) -> bool {
        self.current <= 0.0
    }
}

/// Cake balance — the persistent value store.
/// Persists across sessions, unlike calories which reset on restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CakeBalance {
    pub total_earned: f64,
    pub total_spent: f64,
    pub missions_completed: u64,
    pub current_streak: u64,  // consecutive successful missions
}
