//! Budget-aware scheduling controller for k8s Stack CRDs
//!
//! Tracks spending, enforces budget limits, and integrates with n8n webhooks.
//! Implements the budget-aware scheduling mechanism from MBSE orchestration architecture.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Budget controller manages spending limits for Stack CRDs
pub struct BudgetController {
    /// Daily spending by stack name
    spending: HashMap<String, f64>,
    /// n8n webhook URL for budget alerts
    webhook_url: Option<String>,
}

/// Budget state tracked in k8s annotations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetState {
    /// Total spent today
    pub spent_today: f64,
    /// Number of jobs completed today
    pub jobs_completed: u32,
    /// Last reset timestamp (UTC)
    pub last_reset: String,
    /// Budget status: ok, warning, exceeded
    pub status: BudgetStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BudgetStatus {
    Ok,
    Warning,  // At 80%+ of daily limit
    Exceeded, // Over daily limit
}

/// Budget policy action
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetAction {
    Allow,  // Job can proceed
    Defer,  // Defer job until next day
    Alert,  // Send alert but allow
    Cancel, // Cancel job permanently
}

/// n8n webhook payload for budget alerts
#[derive(Debug, Serialize)]
pub struct BudgetAlert {
    pub stack_name: String,
    pub event: String,
    pub spent_today: f64,
    pub daily_limit: f64,
    pub percentage: f64,
    pub jobs_completed: u32,
    pub timestamp: String,
}

impl BudgetController {
    /// Create new budget controller
    pub fn new(webhook_url: Option<String>) -> Self {
        Self {
            spending: HashMap::new(),
            webhook_url,
        }
    }

    /// Check if a job can proceed based on budget constraints
    pub fn check_budget(
        &self,
        _stack_name: &str,
        budget_state: &BudgetState,
        daily_limit: f64,
        cost_per_job: f64,
        on_exceeded: &str,
    ) -> BudgetAction {
        let projected_spend = budget_state.spent_today + cost_per_job;

        if projected_spend > daily_limit {
            // Budget would be exceeded
            match on_exceeded {
                "defer" => BudgetAction::Defer,
                "alert" => BudgetAction::Alert,
                "cancel" => BudgetAction::Cancel,
                _ => BudgetAction::Defer, // Default to defer
            }
        } else if projected_spend > daily_limit * 0.8 {
            // Warning threshold (80%+)
            BudgetAction::Alert
        } else {
            BudgetAction::Allow
        }
    }

    /// Update budget state after job completion
    pub fn record_job_completion(
        &mut self,
        stack_name: &str,
        cost_per_job: f64,
        budget_state: &mut BudgetState,
        daily_limit: f64,
    ) -> Result<()> {
        // Check if we need to reset (new day)
        self.check_daily_reset(budget_state)?;

        // Record spending
        budget_state.spent_today += cost_per_job;
        budget_state.jobs_completed += 1;

        // Update status
        let percentage = (budget_state.spent_today / daily_limit) * 100.0;
        budget_state.status = if budget_state.spent_today > daily_limit {
            BudgetStatus::Exceeded
        } else if percentage >= 80.0 {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Ok
        };

        // Update in-memory tracking
        self.spending
            .insert(stack_name.to_string(), budget_state.spent_today);

        Ok(())
    }

    /// Check if budget should be reset (new day)
    fn check_daily_reset(&self, budget_state: &mut BudgetState) -> Result<()> {
        let now = chrono::Utc::now();
        let last_reset = chrono::DateTime::parse_from_rfc3339(&budget_state.last_reset)
            .context("Failed to parse last_reset timestamp")?;

        // Reset if it's a new day (UTC)
        if now.date_naive() != last_reset.date_naive() {
            budget_state.spent_today = 0.0;
            budget_state.jobs_completed = 0;
            budget_state.last_reset = now.to_rfc3339();
            budget_state.status = BudgetStatus::Ok;
        }

        Ok(())
    }

    /// Send budget alert to n8n webhook
    pub async fn send_alert(
        &self,
        stack_name: &str,
        event: &str,
        budget_state: &BudgetState,
        daily_limit: f64,
    ) -> Result<()> {
        let Some(webhook_url) = &self.webhook_url else {
            // No webhook configured, skip
            return Ok(());
        };

        let percentage = (budget_state.spent_today / daily_limit) * 100.0;

        let alert = BudgetAlert {
            stack_name: stack_name.to_string(),
            event: event.to_string(),
            spent_today: budget_state.spent_today,
            daily_limit,
            percentage,
            jobs_completed: budget_state.jobs_completed,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Send HTTP POST to n8n webhook
        let client = reqwest::Client::new();
        let response = client
            .post(webhook_url)
            .json(&alert)
            .send()
            .await
            .context("Failed to send webhook to n8n")?;

        if !response.status().is_success() {
            anyhow::bail!("n8n webhook returned error: {}", response.status());
        }

        Ok(())
    }

    /// Generate k8s annotation key for budget state
    pub fn budget_annotation_key() -> &'static str {
        "b00t.io/budget-state"
    }

    /// Parse budget state from k8s annotation
    pub fn parse_budget_state(annotation: &str) -> Result<BudgetState> {
        serde_json::from_str(annotation).context("Failed to parse budget state annotation")
    }

    /// Serialize budget state for k8s annotation
    pub fn serialize_budget_state(state: &BudgetState) -> Result<String> {
        serde_json::to_string(state).context("Failed to serialize budget state")
    }

    /// Initialize new budget state
    pub fn init_budget_state() -> BudgetState {
        BudgetState {
            spent_today: 0.0,
            jobs_completed: 0,
            last_reset: chrono::Utc::now().to_rfc3339(),
            status: BudgetStatus::Ok,
        }
    }

    /// Get current spending for a stack
    pub fn get_spending(&self, stack_name: &str) -> f64 {
        self.spending.get(stack_name).copied().unwrap_or(0.0)
    }

    /// Get spending report
    pub fn get_report(&self) -> HashMap<String, f64> {
        self.spending.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_check_allow() {
        let controller = BudgetController::new(None);
        let state = BudgetState {
            spent_today: 50.0,
            jobs_completed: 20,
            last_reset: chrono::Utc::now().to_rfc3339(),
            status: BudgetStatus::Ok,
        };

        let action = controller.check_budget("test-stack", &state, 100.0, 2.5, "defer");
        assert_eq!(action, BudgetAction::Allow);
    }

    #[test]
    fn test_budget_check_warning() {
        let controller = BudgetController::new(None);
        let state = BudgetState {
            spent_today: 75.0,
            jobs_completed: 30,
            last_reset: chrono::Utc::now().to_rfc3339(),
            status: BudgetStatus::Ok,
        };

        let action = controller.check_budget("test-stack", &state, 100.0, 10.0, "defer");
        assert_eq!(action, BudgetAction::Alert); // 75 + 10 = 85 > 80%
    }

    #[test]
    fn test_budget_check_exceeded_defer() {
        let controller = BudgetController::new(None);
        let state = BudgetState {
            spent_today: 95.0,
            jobs_completed: 38,
            last_reset: chrono::Utc::now().to_rfc3339(),
            status: BudgetStatus::Warning,
        };

        let action = controller.check_budget("test-stack", &state, 100.0, 10.0, "defer");
        assert_eq!(action, BudgetAction::Defer); // 95 + 10 = 105 > 100
    }

    #[test]
    fn test_budget_check_exceeded_cancel() {
        let controller = BudgetController::new(None);
        let state = BudgetState {
            spent_today: 95.0,
            jobs_completed: 38,
            last_reset: chrono::Utc::now().to_rfc3339(),
            status: BudgetStatus::Warning,
        };

        let action = controller.check_budget("test-stack", &state, 100.0, 10.0, "cancel");
        assert_eq!(action, BudgetAction::Cancel);
    }

    #[test]
    fn test_record_job_completion() {
        let mut controller = BudgetController::new(None);
        let mut state = BudgetController::init_budget_state();

        controller
            .record_job_completion("test-stack", 2.5, &mut state, 100.0)
            .unwrap();

        assert_eq!(state.spent_today, 2.5);
        assert_eq!(state.jobs_completed, 1);
        assert_eq!(state.status, BudgetStatus::Ok);
    }

    #[test]
    fn test_record_job_completion_status_update() {
        let mut controller = BudgetController::new(None);
        let mut state = BudgetState {
            spent_today: 75.0,
            jobs_completed: 30,
            last_reset: chrono::Utc::now().to_rfc3339(),
            status: BudgetStatus::Ok,
        };

        controller
            .record_job_completion("test-stack", 10.0, &mut state, 100.0)
            .unwrap();

        assert_eq!(state.spent_today, 85.0);
        assert_eq!(state.jobs_completed, 31);
        assert_eq!(state.status, BudgetStatus::Warning); // 85% > 80%
    }

    #[test]
    fn test_serialize_deserialize_budget_state() {
        let state = BudgetState {
            spent_today: 42.5,
            jobs_completed: 17,
            last_reset: chrono::Utc::now().to_rfc3339(),
            status: BudgetStatus::Warning,
        };

        let serialized = BudgetController::serialize_budget_state(&state).unwrap();
        let deserialized = BudgetController::parse_budget_state(&serialized).unwrap();

        assert_eq!(deserialized.spent_today, state.spent_today);
        assert_eq!(deserialized.jobs_completed, state.jobs_completed);
        assert_eq!(deserialized.status, state.status);
    }
}
