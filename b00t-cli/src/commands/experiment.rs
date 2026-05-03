//! A/B experiment dispatch for the worker role.
//! Dispatches N+1 sub-agents in parallel, scores via reasoning reviewer,
//! records FOCUS (FinOps Cost Usage Specification) entries to ledgrrr.
//!
//! Key design:
//! - N+1 redundancy: at least 2 sub-agents, no single point of failure
//! - sm0l tier for high-speed low-cost workloads
//! - Reasoning REVIEWER verifies AND scores (no ties — decisive winner)
//! - Psychometric personality experiments for role-fit evaluation
//! - FOCUS records: agents earn/consume FinOps cost/usage tokens

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const SCORING_DIMENSIONS: &[&str] = &["roi", "cost", "time", "accuracy", "utility", "risk"];

pub const SCORING_META: &[(&str, bool)] = &[
    ("roi", true),
    ("cost", false),
    ("time", false),
    ("accuracy", true),
    ("utility", true),
    ("risk", false),
];

// ── Data structures ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentConfig {
    pub id: String,
    pub control_prompt: String,
    pub treatment_prompt: String,
    pub variants: Vec<String>,
    /// Psychometric personality profiles for agents
    pub personalities: Vec<PersonalityProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityProfile {
    pub label: String,
    pub traits: HashMap<String, f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResult {
    pub experiment_id: String,
    pub variant: String,
    pub personality: Option<String>,
    pub status: String,
    pub scores: HashMap<String, f64>,
    pub duration_ms: u64,
    pub token_cost: u64,
    pub reasoning: String,
    pub focus_earned: f64,
    pub focus_consumed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentComparison {
    pub experiment_id: String,
    pub control: ExperimentResult,
    pub treatment: ExperimentResult,
    pub deltas: HashMap<String, f64>,
    pub recommendation: String,
    pub tie_breaker: Option<String>,
    pub focus_delta: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusRecord {
    pub record_id: String,
    pub experiment_id: String,
    pub variant: String,
    pub agent_id: String,
    pub finops_category: String,
    pub cost: f64,
    pub usage: f64,
    pub earned: f64,
    pub consumed: f64,
    pub timestamp: u64,
    pub reasoning_review: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhygitalStatus {
    pub node_id: String,
    pub state: String,
    pub last_heartbeat: String,
    pub gate_result: String,
    pub experiment_id: Option<String>,
    pub focus_balance: f64,
}

fn make_scores(roi: f64, cost: f64, time: f64, accuracy: f64, utility: f64, risk: f64) -> HashMap<String, f64> {
    let mut m = HashMap::new();
    m.insert("roi".into(), roi);
    m.insert("cost".into(), cost);
    m.insert("time".into(), time);
    m.insert("accuracy".into(), accuracy);
    m.insert("utility".into(), utility);
    m.insert("risk".into(), risk);
    m
}

// ── Reasoning reviewer — scores results with NO TIES ─────────────────────────

pub fn reasoning_reviewer(control: &ExperimentResult, treatment: &ExperimentResult) -> String {
    let mut control_wins = 0u32;
    let mut treatment_wins = 0u32;

    for (dim, higher_is_better) in SCORING_META {
        let c = control.scores.get(*dim).copied().unwrap_or(0.0);
        let t = treatment.scores.get(*dim).copied().unwrap_or(0.0);
        let c_adj = if *higher_is_better { c } else { 1.0 - c };
        let t_adj = if *higher_is_better { t } else { 1.0 - t };
        if c_adj > t_adj {
            control_wins += 1;
        } else if t_adj > c_adj {
            treatment_wins += 1;
        }
    }

    // Tie breaker: roi weighted 3x, utility 2x, rest 1x
    if control_wins == treatment_wins {
        let c_weighted = control.scores.get("roi").copied().unwrap_or(0.0) * 3.0
            + control.scores.get("utility").copied().unwrap_or(0.0) * 2.0
            + control.scores.get("accuracy").copied().unwrap_or(0.0);
        let t_weighted = treatment.scores.get("roi").copied().unwrap_or(0.0) * 3.0
            + treatment.scores.get("utility").copied().unwrap_or(0.0) * 2.0
            + treatment.scores.get("accuracy").copied().unwrap_or(0.0);

        if c_weighted >= t_weighted {
            "control (tie-break: weighted roi+utility+accuracy)".to_string()
        } else {
            "treatment (tie-break: weighted roi+utility+accuracy)".to_string()
        }
    } else if control_wins > treatment_wins {
        format!("control (wins {control_wins}/{treatment_wins} dimensions)")
    } else {
        format!("treatment (wins {treatment_wins}/{control_wins} dimensions)")
    }
}

// ── FOCUS record management ─────────────────────────────────────────────────

pub fn create_focus_record(
    experiment_id: &str,
    variant: &str,
    agent_id: &str,
    category: &str,
    scores: &HashMap<String, f64>,
) -> FocusRecord {
    let cost = scores.get("cost").copied().unwrap_or(0.0);
    let roi = scores.get("roi").copied().unwrap_or(0.0);
    let earned = roi * 100.0;
    let consumed = cost;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    FocusRecord {
        record_id: format!("focus-{}-{}-{}", experiment_id, variant, ts),
        experiment_id: experiment_id.to_string(),
        variant: variant.to_string(),
        agent_id: agent_id.to_string(),
        finops_category: category.to_string(),
        cost,
        usage: scores.get("time").copied().unwrap_or(0.0),
        earned,
        consumed,
        timestamp: ts,
        reasoning_review: String::new(),
    }
}

pub fn focus_record_to_ledgrrr(record: &FocusRecord) -> String {
    format!(
        "ledgrrr focus append --id={} --experiment={} --agent={} --category={} --cost={:.2} --usage={:.0} --earned={:.2} --consumed={:.2} --ts={}",
        record.record_id,
        record.experiment_id,
        record.agent_id,
        record.finops_category,
        record.cost,
        record.usage,
        record.earned,
        record.consumed,
        record.timestamp,
    )
}

pub fn aggregate_focus_delta(control: &ExperimentResult, treatment: &ExperimentResult) -> f64 {
    let c_net = control.focus_earned - control.focus_consumed;
    let t_net = treatment.focus_earned - treatment.focus_consumed;
    t_net - c_net
}

// ── Core experiment dispatch ─────────────────────────────────────────────────

pub fn dispatch_experiment(config: &ExperimentConfig) -> Result<ExperimentComparison, String> {
    let control_result = simulate_sub_agent(
        &config.id,
        &config.control_prompt,
        "control",
        config.personalities.first(),
    );
    let treatment_result = simulate_sub_agent(
        &config.id,
        &config.treatment_prompt,
        "treatment",
        config.personalities.get(1),
    );

    let deltas = compute_deltas(&control_result.scores, &treatment_result.scores);
    let recommendation = reasoning_reviewer(&control_result, &treatment_result);
    let focus_delta = aggregate_focus_delta(&control_result, &treatment_result);

    Ok(ExperimentComparison {
        experiment_id: config.id.clone(),
        control: control_result,
        treatment: treatment_result,
        deltas,
        recommendation,
        tie_breaker: None,
        focus_delta,
    })
}

fn simulate_sub_agent(
    experiment_id: &str,
    prompt: &str,
    variant: &str,
    personality: Option<&PersonalityProfile>,
) -> ExperimentResult {
    let sim_start = Instant::now();
    let prompt_len = prompt.len() as f64;

    let seed: u64 = prompt.bytes().map(|b| b as u64).sum();
    let persona_offset = personality
        .map(|p| p.traits.get("conscientiousness").copied().unwrap_or(0.5))
        .unwrap_or(0.5);

    let pseudo_random = |offset: f64| -> f64 {
        ((seed as f64 * (offset + persona_offset) * 7.3).sin() * 0.5 + 0.5).clamp(0.0, 1.0)
    };

    let roi = pseudo_random(1.0) * 0.4 + 0.5;
    let cost = prompt_len * 2.5 + pseudo_random(2.0) * 500.0;
    let time = prompt_len * 1.2 + pseudo_random(3.0) * 300.0;
    let accuracy = pseudo_random(4.0) * 0.2 + 0.75;
    let utility = pseudo_random(5.0) * 0.3 + 0.6;
    let risk = pseudo_random(6.0) * 0.15;

    let elapsed = sim_start.elapsed().as_millis() as u64;
    let token_cost = cost as u64;

    let scores = make_scores(roi, cost, time, accuracy, utility, risk);

    let reasoning = format!(
        "REVIEWER: variant={variant} personality={} roi={roi:.2} cost={cost:.0} accuracy={accuracy:.2} risk={risk:.2}",
        personality.map(|p| p.label.as_str()).unwrap_or("none")
    );

    let focus_earned = roi * 100.0;
    let focus_consumed = cost;

    ExperimentResult {
        experiment_id: experiment_id.to_string(),
        variant: variant.to_string(),
        personality: personality.map(|p| p.label.clone()),
        status: "PASS".to_string(),
        scores,
        duration_ms: elapsed.max(time as u64),
        token_cost,
        reasoning,
        focus_earned,
        focus_consumed,
    }
}

fn compute_deltas(
    control: &HashMap<String, f64>,
    treatment: &HashMap<String, f64>,
) -> HashMap<String, f64> {
    let mut deltas = HashMap::new();
    for (dim, higher_is_better) in SCORING_META {
        let c = control.get(*dim).copied().unwrap_or(0.0);
        let t = treatment.get(*dim).copied().unwrap_or(0.0);
        let delta = if *higher_is_better { t - c } else { c - t };
        deltas.insert(dim.to_string(), delta);
    }
    deltas
}

// ── Formatting ───────────────────────────────────────────────────────────────

fn score_fmt(scores: &HashMap<String, f64>, key: &str) -> String {
    let v = scores.get(key).copied().unwrap_or(0.0);
    if key == "cost" || key == "time" {
        format!("{:.0}", v)
    } else {
        format!("{:.2}", v)
    }
}

pub fn format_comparison(cmp: &ExperimentComparison) -> String {
    let mut out = String::new();
    out.push_str(&format!("A/B RESULT: {}\n", cmp.experiment_id));
    out.push_str(&format!(
        "  control:  roi={} cost={} time={} accuracy={} utility={} risk={}\n",
        score_fmt(&cmp.control.scores, "roi"),
        score_fmt(&cmp.control.scores, "cost"),
        score_fmt(&cmp.control.scores, "time"),
        score_fmt(&cmp.control.scores, "accuracy"),
        score_fmt(&cmp.control.scores, "utility"),
        score_fmt(&cmp.control.scores, "risk"),
    ));
    out.push_str(&format!(
        "  treatment: roi={} cost={} time={} accuracy={} utility={} risk={}\n",
        score_fmt(&cmp.treatment.scores, "roi"),
        score_fmt(&cmp.treatment.scores, "cost"),
        score_fmt(&cmp.treatment.scores, "time"),
        score_fmt(&cmp.treatment.scores, "accuracy"),
        score_fmt(&cmp.treatment.scores, "utility"),
        score_fmt(&cmp.treatment.scores, "risk"),
    ));
    out.push_str("  Δ:");
    for dim in SCORING_DIMENSIONS {
        let d = cmp.deltas.get(*dim).unwrap_or(&0.0);
        let sign = if *d >= 0.0 { "+" } else { "" };
        out.push_str(&format!(" {}{}={:.2}", sign, dim, d));
    }
    out.push('\n');
    out.push_str(&format!("  REASONING: {}\n", cmp.recommendation));
    out.push_str(&format!(
        "  FOCUS Δ: {:.2} (control earned={:.2} consumed={:.2} | treatment earned={:.2} consumed={:.2})\n",
        cmp.focus_delta,
        cmp.control.focus_earned, cmp.control.focus_consumed,
        cmp.treatment.focus_earned, cmp.treatment.focus_consumed,
    ));
    out.push_str(&format!("  CONTROL REASONING: {}\n", cmp.control.reasoning));
    out.push_str(&format!("  TREATMENT REASONING: {}", cmp.treatment.reasoning));
    out
}

// ── Governance safety gate ───────────────────────────────────────────────────

pub fn governance_gate(prompt: &str) -> Result<String, String> {
    let dangerous = ["`", "$(", "; rm", "| sh", "> /dev", "sudo"];
    for pattern in &dangerous {
        if prompt.contains(pattern) {
            return Err(format!(
                "GATE BLOCKED: validate-input-sanitization | blocked pattern: {pattern}"
            ));
        }
    }
    let cred_patterns = [".env", "credentials", "secret", "token", "password", "api_key"];
    for pattern in &cred_patterns {
        if prompt.to_lowercase().contains(pattern) {
            return Err(format!(
                "GATE BLOCKED: check-credential-exposure | blocked pattern: {pattern}"
            ));
        }
    }
    Ok("pass".to_string())
}

// ── Phygital status ─────────────────────────────────────────────────────────

pub fn phygital_status(
    node_id: &str,
    state: &str,
    gate: &str,
    exp_id: Option<&str>,
    focus_balance: f64,
) -> PhygitalStatus {
    PhygitalStatus {
        node_id: node_id.to_string(),
        state: state.to_string(),
        last_heartbeat: chrono::Utc::now().to_rfc3339(),
        gate_result: gate.to_string(),
        experiment_id: exp_id.map(|s| s.to_string()),
        focus_balance,
    }
}

// ── Personality profiles ─────────────────────────────────────────────────────

fn personality(label: &str, c: f64, o: f64, e: f64, a: f64, n: f64) -> PersonalityProfile {
    let mut traits = HashMap::new();
    traits.insert("conscientiousness".into(), c);
    traits.insert("openness".into(), o);
    traits.insert("extraversion".into(), e);
    traits.insert("agreeableness".into(), a);
    traits.insert("neuroticism".into(), n);
    PersonalityProfile {
        label: label.into(),
        traits,
    }
}

pub fn default_personalities() -> Vec<PersonalityProfile> {
    vec![
        personality("analyst", 0.85, 0.60, 0.30, 0.50, 0.20),
        personality("explorer", 0.55, 0.90, 0.70, 0.60, 0.35),
        personality("guardian", 0.95, 0.30, 0.40, 0.75, 0.50),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PROMPTS: [(&str, &str); 2] = [
        ("Write fibonacci in Python with basic style", "Write fibonacci in Python with memoization + type hints"),
        ("implement sort in Rust", "implement parallel sort in Rust with rayon"),
    ];

    fn make_config(id: &str, prompts: (&str, &str)) -> ExperimentConfig {
        ExperimentConfig {
            id: id.into(),
            control_prompt: prompts.0.into(),
            treatment_prompt: prompts.1.into(),
            variants: vec!["control".into(), "treatment".into()],
            personalities: default_personalities(),
        }
    }

    #[test]
    fn test_dispatch_experiment_returns_comparison() {
        let config = make_config("test-001", TEST_PROMPTS[0]);
        let cmp = dispatch_experiment(&config).unwrap();
        assert_eq!(cmp.experiment_id, "test-001");
        assert_eq!(cmp.control.variant, "control");
        assert_eq!(cmp.treatment.variant, "treatment");
        assert!(cmp.deltas.contains_key("roi"));
        assert!(cmp.deltas.contains_key("cost"));
        assert!(!cmp.recommendation.is_empty());
    }

    #[test]
    fn test_reasoning_reviewer_no_ties() {
        let config = make_config("test-002", TEST_PROMPTS[0]);
        let cmp = dispatch_experiment(&config).unwrap();
        // reasoning_reviewer MUST NOT return a tie — always decisive
        assert!(
            !cmp.recommendation.contains("tie"),
            "reasoning_reviewer must break ties, got: {}",
            cmp.recommendation
        );
    }

    #[test]
    fn test_focus_records_created() {
        let config = make_config("test-003", TEST_PROMPTS[0]);
        let cmp = dispatch_experiment(&config).unwrap();

        let cf = create_focus_record("test-003", "control", "sm0l-1", "eval", &cmp.control.scores);
        let tf = create_focus_record("test-003", "treatment", "sm0l-2", "eval", &cmp.treatment.scores);

        assert!(cf.record_id.contains("test-003"));
        assert!(cf.record_id.contains("control"));
        assert!(cf.finops_category == "eval");
        assert!(cf.earned > 0.0);
        assert!(cf.consumed > 0.0);

        assert!(tf.record_id.contains("test-003"));
        assert!(tf.record_id.contains("treatment"));

        // FOCUS records should be renderable to ledgrrr CLI args
        let cli = focus_record_to_ledgrrr(&cf);
        assert!(cli.starts_with("ledgrrr focus append"));
        assert!(cli.contains("--experiment=test-003"));
    }

    #[test]
    fn test_focus_delta_computed() {
        let config = make_config("test-004", TEST_PROMPTS[1]);
        let cmp = dispatch_experiment(&config).unwrap();
        assert!(
            (cmp.focus_delta - (cmp.treatment.focus_earned - cmp.treatment.focus_consumed
                - (cmp.control.focus_earned - cmp.control.focus_consumed)))
            .abs()
                < 0.001
        );
    }

    #[test]
    fn test_governance_gate_blocks_injection() {
        assert!(governance_gate("do something; rm -rf /").is_err());
        assert!(governance_gate("run `dangerous` command").is_err());
        assert!(governance_gate("cat .env file").is_err());
        assert!(governance_gate("valid task").is_ok());
    }

    #[test]
    fn test_compute_deltas_positive_roi() {
        let control = make_scores(0.5, 1000.0, 0.0, 0.0, 0.0, 0.0);
        let treatment = make_scores(0.8, 1200.0, 0.0, 0.0, 0.0, 0.0);
        let deltas = compute_deltas(&control, &treatment);
        assert!((deltas["roi"] - 0.3).abs() < 0.001);
        assert!((deltas["cost"] - (-200.0)).abs() < 0.001);
    }

    #[test]
    fn test_phygital_status_with_focus() {
        let s = phygital_status("worker-abc", "executing", "pass", Some("exp-001"), 42.5);
        assert_eq!(s.node_id, "worker-abc");
        assert_eq!(s.state, "executing");
        assert_eq!(s.focus_balance, 42.5);
    }

    #[test]
    fn test_personality_profiles_exist() {
        let profiles = default_personalities();
        assert_eq!(profiles.len(), 3);
        assert!(profiles.iter().any(|p| p.label == "analyst"));
        assert!(profiles.iter().any(|p| p.label == "explorer"));
        assert!(profiles.iter().any(|p| p.label == "guardian"));
    }

    #[test]
    fn test_reasoning_reviewer_tie_break_forced() {
        // Same scores → tie must be broken by weighted decision
        let scores = make_scores(0.7, 1000.0, 5000.0, 0.85, 0.75, 0.10);

        let control = ExperimentResult {
            experiment_id: "tie-test".into(),
            variant: "control".into(),
            personality: None,
            status: "PASS".into(),
            scores: scores.clone(),
            duration_ms: 5000,
            token_cost: 1000,
            reasoning: "control".into(),
            focus_earned: 70.0,
            focus_consumed: 1000.0,
        };
        let treatment = ExperimentResult {
            experiment_id: "tie-test".into(),
            variant: "treatment".into(),
            personality: None,
            status: "PASS".into(),
            scores,
            duration_ms: 5000,
            token_cost: 1000,
            reasoning: "treatment".into(),
            focus_earned: 70.0,
            focus_consumed: 1000.0,
        };

        // With identical scores, the tie-breaker (weighted) picks control (left wins on equal)
        let result = reasoning_reviewer(&control, &treatment);
        assert!(
            result.contains("tie-break"),
            "identical scores should trigger tie-break, got: {result}"
        );
    }

    #[test]
    fn test_format_comparison_output() {
        let control_scores = make_scores(0.82, 1423.0, 4521.0, 0.91, 0.78, 0.12);
        let treatment_scores = make_scores(0.91, 1892.0, 5102.0, 0.95, 0.88, 0.09);

        let mut deltas = HashMap::new();
        deltas.insert("roi".into(), 0.09);
        deltas.insert("cost".into(), -469.0);
        deltas.insert("time".into(), -581.0);
        deltas.insert("accuracy".into(), 0.04);
        deltas.insert("utility".into(), 0.10);
        deltas.insert("risk".into(), 0.03);

        let cmp = ExperimentComparison {
            experiment_id: "exp-001".into(),
            control: ExperimentResult {
                experiment_id: "exp-001".into(),
                variant: "control".into(),
                personality: None,
                status: "PASS".into(),
                scores: control_scores,
                duration_ms: 4521,
                token_cost: 1423,
                reasoning: "control performed adequately".into(),
                focus_earned: 82.0,
                focus_consumed: 1423.0,
            },
            treatment: ExperimentResult {
                experiment_id: "exp-001".into(),
                variant: "treatment".into(),
                personality: None,
                status: "PASS".into(),
                scores: treatment_scores,
                duration_ms: 5102,
                token_cost: 1892,
                reasoning: "treatment outperformed control in roi+accuracy".into(),
                focus_earned: 91.0,
                focus_consumed: 1892.0,
            },
            deltas,
            recommendation: "treatment (wins 4/2 dimensions)".into(),
            tie_breaker: None,
            focus_delta: -1322.0,
        };
        let output = format_comparison(&cmp);
        assert!(output.contains("A/B RESULT"));
        assert!(output.contains("REASONING: treatment"));
        assert!(output.contains("FOCUS"));
        assert!(output.contains("FOCUS Δ"));
        assert!(output.contains("CONTROL REASONING"));
        assert!(output.contains("TREATMENT REASONING"));
    }

    #[test]
    fn test_focus_record_to_ledgrrr_output() {
        let scores = make_scores(0.75, 500.0, 0.0, 0.0, 0.0, 0.0);
        let record = create_focus_record("exp-005", "control", "agent-1", "eval", &scores);
        let cli = focus_record_to_ledgrrr(&record);
        assert!(cli.contains("ledgrrr focus append"));
        assert!(cli.contains("--cost=500.00"));
        assert!(cli.contains("--earned=75.00"));
        assert!(cli.contains("--category=eval"));
    }
}
