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

/// n8n webhook payload for a sudo-grant escalation (PRD-SUDO-OPERATOR-GOVERNANCE).
/// Fired when the adversarial review model can't confidently Grant or Deny.
/// n8n fans this out to whatever downstream channels (Slack/email/SMS) are
/// configured in the workflow — b00t itself only needs to know about n8n.
#[derive(Debug, Serialize)]
pub struct SudoEscalationAlert {
    pub event: String,
    pub command: String,
    pub justification: String,
    pub cited_commits: Vec<String>,
    pub reason: String,
    pub timestamp: String,
}

impl SudoEscalationAlert {
    /// Build the webhook payload — separate from the HTTP fire so payload
    /// construction is unit-testable without a network.
    pub fn new(
        command: &str,
        justification: &str,
        cited_commits: &[String],
        reason: &str,
    ) -> Self {
        Self {
            event: "sudo_grant_escalation".to_string(),
            command: command.to_string(),
            justification: justification.to_string(),
            cited_commits: cited_commits.to_vec(),
            reason: reason.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Fire a sudo-grant escalation to the configured n8n webhook.
///
/// Call site (`b00t exec`'s `handle_exec`) is a synchronous function that
/// runs on a thread owned by b00t-cli's `#[tokio::main]` runtime, so a
/// plain `reqwest::blocking::Client` panics ("Cannot drop a runtime in a
/// context where blocking is not allowed"). Uses `block_in_place` +
/// `block_on` to do the async HTTP call from nested sync code instead —
/// same HTTP-POST-JSON shape as `BudgetController::send_alert`, just not
/// tied to the async k8s reconciliation path that struct lives on.
/// Best-effort: a webhook failure is logged but never blocks the
/// (already-denied) command path.
///
/// Webhook URL comes from `B00T_N8N_WEBHOOK` (legacy alias
/// `B00T_N8N_WEBHOOK_URL` still honored); if neither is set, skip with a
/// single stderr note — same "don't gate on the webhook" policy as
/// `BudgetController::send_alert` when its `webhook_url` is `None`.
pub fn fire_sudo_escalation(command: &str, justification: &str, cited_commits: &[String], reason: &str) {
    let Ok(webhook_url) =
        std::env::var("B00T_N8N_WEBHOOK").or_else(|_| std::env::var("B00T_N8N_WEBHOOK_URL"))
    else {
        eprintln!("📡 B00T_N8N_WEBHOOK not set — skipping n8n escalation webhook");
        return;
    };

    let alert = SudoEscalationAlert::new(command, justification, cited_commits, reason);

    let result: std::result::Result<(), String> = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let client = reqwest::Client::new();
            let resp = client
                .post(&webhook_url)
                .json(&alert)
                .send()
                .await
                .map_err(|e| format!("sudo-escalation webhook failed: {e}"))?;
            if resp.status().is_success() {
                Ok(())
            } else {
                Err(format!("sudo-escalation webhook returned {}", resp.status()))
            }
        })
    });

    if let Err(e) = result {
        eprintln!("⚠️  {e}");
    }
}

/// GPU-aware ch0nky model router.
///
/// Checks local GPU free memory and selects the appropriate ch0nky model endpoint:
/// - GPU free ≥ 4000 MB → local Qwen3-Coder-30B on vLLM (cheap, fast)
/// - GPU free < 4000 MB → Fable 5 via Anthropic API (cloud burst, tool-use optimized)
///
/// Callers set the returned env vars before spawning sub-agents:
/// ```text
/// B00T_AI_CH0NKY_MODEL=<model>  B00T_AI_CH0NKY_BASE=<base_url>
/// ```
#[derive(Debug, Clone)]
pub struct ChonkyModelGate {
    /// Threshold below which we fall back to Fable 5 (default: LOCAL_GPU_FREE_MB_GATE)
    pub gpu_threshold_mb: u32,
    /// Local vLLM model name (Qwen3-Coder-30B)
    pub local_model: String,
    /// Local vLLM base URL
    pub local_base: String,
    /// Fable 5 model id for Anthropic fallback
    pub fable_model: String,
    /// Anthropic API base URL
    pub fable_base: String,
}

#[derive(Debug, Clone)]
pub struct ChonkyModelSelection {
    pub model: String,
    pub base_url: String,
    pub tier_source: ChonkyTierSource,
    pub gpu_free_mb: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChonkyTierSource {
    LocalVllm,     // GPU free — use local Qwen3-Coder
    FableFallback, // GPU claimed — burst to Fable 5
    EnvOverride,   // B00T_AI_CH0NKY_MODEL set explicitly in environment
}

impl Default for ChonkyModelGate {
    fn default() -> Self {
        Self {
            gpu_threshold_mb: 4000,
            local_model: "Qwen/Qwen3-Coder-30B-Instruct".into(),
            local_base: "http://localhost:8000/v1".into(),
            fable_model: "claude-fable-5".into(),
            fable_base: "https://api.anthropic.com/v1".into(),
        }
    }
}

impl ChonkyModelGate {
    /// Select ch0nky model based on GPU availability.
    ///
    /// Checks `B00T_AI_CH0NKY_MODEL` env var first (explicit override wins).
    /// Falls back to GPU probe → local or Fable 5.
    pub fn select(&self) -> ChonkyModelSelection {
        // Env override takes precedence
        if let Ok(m) = std::env::var("B00T_AI_CH0NKY_MODEL") {
            let base = std::env::var("B00T_AI_CH0NKY_BASE").unwrap_or_else(|_| self.local_base.clone());
            return ChonkyModelSelection {
                model: m,
                base_url: base,
                tier_source: ChonkyTierSource::EnvOverride,
                gpu_free_mb: None,
            };
        }

        // Probe GPU
        let gpu_free_mb = crate::hive::SystemSnapshot::capture()
            .ok()
            .and_then(|s| s.gpu_free_mb);

        let is_local_available = gpu_free_mb.map(|mb| mb >= self.gpu_threshold_mb).unwrap_or(false);

        if is_local_available {
            ChonkyModelSelection {
                model: self.local_model.clone(),
                base_url: self.local_base.clone(),
                tier_source: ChonkyTierSource::LocalVllm,
                gpu_free_mb,
            }
        } else {
            ChonkyModelSelection {
                model: self.fable_model.clone(),
                base_url: self.fable_base.clone(),
                tier_source: ChonkyTierSource::FableFallback,
                gpu_free_mb,
            }
        }
    }

    /// Emit shell export lines for the selected model (for eval in shell scripts).
    pub fn export_env(&self, sel: &ChonkyModelSelection) -> String {
        format!(
            "export B00T_AI_CH0NKY_MODEL={} B00T_AI_CH0NKY_BASE={}",
            shlex_quote(&sel.model),
            shlex_quote(&sel.base_url),
        )
    }
}

fn shlex_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_alphanumeric() || "-_:/=.".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
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

    // Serialize env-var-mutating tests to prevent races between parallel test threads.
    static CH0NKY_ENV_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

    #[test]
    fn test_ch0nky_gate_env_override() {
        let _guard = CH0NKY_ENV_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
        unsafe { std::env::set_var("B00T_AI_CH0NKY_MODEL", "test-model") };
        unsafe { std::env::set_var("B00T_AI_CH0NKY_BASE", "http://test:8000/v1") };
        let gate = ChonkyModelGate::default();
        let sel = gate.select();
        assert_eq!(sel.tier_source, ChonkyTierSource::EnvOverride);
        assert_eq!(sel.model, "test-model");
        assert_eq!(sel.base_url, "http://test:8000/v1");
        unsafe { std::env::remove_var("B00T_AI_CH0NKY_MODEL") };
        unsafe { std::env::remove_var("B00T_AI_CH0NKY_BASE") };
    }

    #[test]
    fn test_ch0nky_gate_fable_fallback_when_gpu_unavailable() {
        let _guard = CH0NKY_ENV_LOCK.get_or_init(|| std::sync::Mutex::new(())).lock().unwrap();
        unsafe { std::env::remove_var("B00T_AI_CH0NKY_MODEL") };
        let gate = ChonkyModelGate {
            gpu_threshold_mb: u32::MAX, // force fallback by setting impossible threshold
            ..ChonkyModelGate::default()
        };
        let sel = gate.select();
        assert_eq!(sel.tier_source, ChonkyTierSource::FableFallback);
        assert_eq!(sel.model, "claude-fable-5");
    }

    #[test]
    fn test_ch0nky_gate_export_env() {
        let gate = ChonkyModelGate::default();
        let sel = ChonkyModelSelection {
            model: "claude-fable-5".into(),
            base_url: "https://api.anthropic.com/v1".into(),
            tier_source: ChonkyTierSource::FableFallback,
            gpu_free_mb: Some(1000),
        };
        let env_str = gate.export_env(&sel);
        assert!(env_str.contains("B00T_AI_CH0NKY_MODEL=claude-fable-5"));
        assert!(env_str.contains("B00T_AI_CH0NKY_BASE=https://api.anthropic.com/v1"));
    }

    #[test]
    fn test_sudo_escalation_alert_payload() {
        let alert = SudoEscalationAlert::new(
            "sudo systemctl restart k0scontroller",
            "kubelet device-plugin registration wedged",
            &["deadbeef".to_string()],
            "blast radius ambiguous",
        );
        let json = serde_json::to_value(&alert).unwrap();
        assert_eq!(json["event"], "sudo_grant_escalation");
        assert_eq!(json["command"], "sudo systemctl restart k0scontroller");
        assert_eq!(json["justification"], "kubelet device-plugin registration wedged");
        assert_eq!(json["cited_commits"][0], "deadbeef");
        assert_eq!(json["reason"], "blast radius ambiguous");
        // ts present and RFC 3339-parseable
        let ts = json["timestamp"].as_str().unwrap();
        assert!(chrono::DateTime::parse_from_rfc3339(ts).is_ok());
    }

    #[test]
    fn test_sudo_escalation_alert_empty_cites() {
        let alert = SudoEscalationAlert::new("cmd", "why", &[], "unsure");
        let json = serde_json::to_value(&alert).unwrap();
        assert!(json["cited_commits"].as_array().unwrap().is_empty());
    }
}
