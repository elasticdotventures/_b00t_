//! systemone — TypeSafe AI Jev (System One) client + s0ul-scope classifier.
//!
//! Task #61 / datum: `jev-systemone.ai.toml` (TypeSafe's Jev — typed
//! probabilistic decisions: n0ul | ch0ice | sc0re; NOT a text-generation LLM).
//! This module wires the `ch0ice` primitive into the lesson-recording path:
//! classify whether a lesson belongs to repo scope or global (hive) scope
//! from its attributes, instead of silently defaulting.
//!
//! # Design invariants
//! - **Env-gated**: `SystemOneClient::from_env()` returns `None` unless BOTH
//!   `TYPESAFE_API_BASE` and `TYPESAFE_API_KEY` are set — no creds, no calls,
//!   behavior identical to pre-#61 (repo default).
//! - **Explicit wins**: an operator-supplied `--global` is NEVER overridden by
//!   the classifier; classification only fills the unspecified case.
//! - **Asymmetric caution**: global writes are cross-hive memory; promotion to
//!   global requires `confidence >= GLOBAL_CONFIDENCE_FLOOR`. Below the floor
//!   (or on any error/timeout) the decision degrades to Repo — and the
//!   existing disclosure gate (#1101) still guards every global write.
//!
//! ⚠️ WIRE FORMAT PROVISIONAL: the jev-systemone.ai datum records that
//! TypeSafe's actual API base/auth scheme were absent from the source
//! reference. `choice()` implements a documented-guess JSON contract
//! (POST {base}/v1/choice) to be reconciled against docs.typesafe.ai the
//! first time real credentials are available — the transport is isolated to
//! this one function; everything else (env gating, decision logic) is final.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Minimum confidence for promoting a lesson to global (hive) scope.
/// 🤓 Asymmetric caution: false-global leaks repo knowledge hive-wide and
///    pollutes other repos; false-repo merely under-shares. Floor is high.
pub const GLOBAL_CONFIDENCE_FLOOR: f64 = 0.8;

/// Request timeout for classifier calls — classification must never hang
/// the recording path; a timeout degrades to the repo default.
pub const CHOICE_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChoiceOption {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Typed outcome of a jev `ch0ice` decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChoiceOutcome {
    /// Winning option label
    pub choice: String,
    /// Probability of the winning option (0-1)
    #[serde(default)]
    pub probability: f64,
    /// Model confidence in the decision (0-1)
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoulScope {
    Repo,
    Global,
}

/// Final scope decision with provenance — surfaced to the operator.
#[derive(Debug, Clone, PartialEq)]
pub struct ScopeDecision {
    pub scope: SoulScope,
    /// Which path produced the decision
    pub source: DecisionSource,
    /// Classifier confidence when source == SystemOne
    pub confidence: Option<f64>,
    /// Set when the classifier wanted global but fell below the floor
    pub degraded_from_global: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionSource {
    /// Explicit operator flag (--global) — never classifier-overridden
    Explicit,
    /// Jev ch0ice decision
    SystemOne,
    /// No creds / call failed / below floor → pre-#61 behavior
    Default,
}

/// Env-gated Jev client. Construct ONLY via `from_env()`.
#[derive(Debug, Clone)]
pub struct SystemOneClient {
    pub base_url: String,
    api_key: String,
    http: reqwest::Client,
}

impl SystemOneClient {
    /// None unless BOTH TYPESAFE_API_BASE and TYPESAFE_API_KEY are set
    /// (non-empty). Mirrors the [env] block of jev-systemone.ai.toml.
    pub fn from_env() -> Option<Self> {
        let base = std::env::var("TYPESAFE_API_BASE").ok()?;
        let key = std::env::var("TYPESAFE_API_KEY").ok()?;
        if base.trim().is_empty() || key.trim().is_empty() {
            return None;
        }
        // 🤓 base may or may not carry a trailing slash; normalize once here.
        let base_url = base.trim_end_matches('/').to_string();
        Some(Self {
            base_url,
            api_key: key,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(CHOICE_TIMEOUT_SECS))
                .build()
                .ok()?,
        })
    }

    /// ⚠️ PROVISIONAL wire format — see module header. One function to
    ///    reconcile against docs.typesafe.ai when credentials land.
    pub async fn choice(&self, prompt: &str, options: &[ChoiceOption]) -> Result<ChoiceOutcome> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            prompt: &'a str,
            options: &'a [ChoiceOption],
        }
        let url = format!("{}/v1/choice", self.base_url);
        let resp = self
            .http
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&Req {
                model: "jev",
                prompt,
                options,
            })
            .send()
            .await
            .with_context(|| format!("jev choice POST {url}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("jev choice {status}: {}", truncate(&body, 200));
        }
        let outcome: ChoiceOutcome =
            serde_json::from_str(&body).context("parse jev choice outcome")?;
        Ok(outcome)
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

/// Canonical ch0ice options for the s0ul-scope question.
/// 🤓 Includes the 'other/ask' escape per jev guidance — the taxonomy of
///    future lesson kinds is not exhaustive.
pub fn scope_choice_options() -> Vec<ChoiceOption> {
    vec![
        ChoiceOption {
            label: "repo".into(),
            description: Some(
                "Project-specific knowledge: this repo's tools, paths, sharp edges, workflows"
                    .into(),
            ),
        },
        ChoiceOption {
            label: "global".into(),
            description: Some(
                "Hive-wide knowledge: cross-repo tooling, tribal remedies, protocol truths"
                    .into(),
            ),
        },
        ChoiceOption {
            label: "ask".into(),
            description: Some("Ambiguous — leave the decision to the operator".into()),
        },
    ]
}

/// Build the classification prompt from lesson attributes (topic + body).
pub fn scope_prompt(lesson_topic: &str, lesson_body: &str) -> String {
    format!(
        "Classify the memory scope of this b00t lesson.\nTopic: {lesson_topic}\nLesson: {body}\n\nrepo = only useful inside the recording project; global = useful to every hive project; ask = genuinely ambiguous.",
        body = truncate(lesson_body, 1200)
    )
}

/// PURE decision logic — no I/O. Maps a transport outcome (or its absence)
/// to a ScopeDecision under the asymmetric-caution rule.
pub fn decide_scope(outcome: Option<ChoiceOutcome>) -> ScopeDecision {
    let Some(outcome) = outcome else {
        return ScopeDecision {
            scope: SoulScope::Repo,
            source: DecisionSource::Default,
            confidence: None,
            degraded_from_global: false,
        };
    };
    match outcome.choice.as_str() {
        "global" if outcome.confidence >= GLOBAL_CONFIDENCE_FLOOR => ScopeDecision {
            scope: SoulScope::Global,
            source: DecisionSource::SystemOne,
            confidence: Some(outcome.confidence),
            degraded_from_global: false,
        },
        "global" => ScopeDecision {
            // Wanted global, below floor → stay repo, surface the near-miss
            scope: SoulScope::Repo,
            source: DecisionSource::Default,
            confidence: Some(outcome.confidence),
            degraded_from_global: true,
        },
        "repo" => ScopeDecision {
            scope: SoulScope::Repo,
            source: DecisionSource::SystemOne,
            confidence: Some(outcome.confidence),
            degraded_from_global: false,
        },
        // "ask" or any unknown label → operator default
        _ => ScopeDecision {
            scope: SoulScope::Repo,
            source: DecisionSource::Default,
            confidence: Some(outcome.confidence),
            degraded_from_global: false,
        },
    }
}

/// Full classification path: env-gated client → choice call → decision.
/// Never fails the caller: any error degrades to the repo default.
pub async fn classify_soul_scope(lesson_topic: &str, lesson_body: &str) -> ScopeDecision {
    let Some(client) = SystemOneClient::from_env() else {
        return decide_scope(None);
    };
    let outcome = client
        .choice(&scope_prompt(lesson_topic, lesson_body), &scope_choice_options())
        .await;
    match outcome {
        Ok(o) => decide_scope(Some(o)),
        Err(e) => {
            eprintln!("[scope:systemone] classifier unavailable ({e}) — defaulting to repo scope");
            decide_scope(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(choice: &str, confidence: f64) -> ChoiceOutcome {
        ChoiceOutcome {
            choice: choice.into(),
            probability: confidence,
            confidence,
        }
    }

    #[test]
    fn no_outcome_defaults_to_repo() {
        let d = decide_scope(None);
        assert_eq!(d.scope, SoulScope::Repo);
        assert_eq!(d.source, DecisionSource::Default);
        assert!(!d.degraded_from_global);
    }

    #[test]
    fn high_confidence_global_promotes() {
        let d = decide_scope(Some(outcome("global", 0.93)));
        assert_eq!(d.scope, SoulScope::Global);
        assert_eq!(d.source, DecisionSource::SystemOne);
        assert_eq!(d.confidence, Some(0.93));
    }

    #[test]
    fn exactly_at_floor_promotes() {
        let d = decide_scope(Some(outcome("global", GLOBAL_CONFIDENCE_FLOOR)));
        assert_eq!(d.scope, SoulScope::Global, "floor is inclusive");
    }

    #[test]
    fn low_confidence_global_degrades_to_repo_with_flag() {
        // Asymmetric caution: false-global is the costlier error.
        let d = decide_scope(Some(outcome("global", 0.55)));
        assert_eq!(d.scope, SoulScope::Repo);
        assert!(d.degraded_from_global, "near-miss must be surfaced");
        assert_eq!(d.confidence, Some(0.55));
    }

    #[test]
    fn repo_choice_stays_repo_with_provenance() {
        let d = decide_scope(Some(outcome("repo", 0.99)));
        assert_eq!(d.scope, SoulScope::Repo);
        assert_eq!(d.source, DecisionSource::SystemOne);
    }

    #[test]
    fn ask_and_unknown_labels_default() {
        for label in ["ask", "both", "", "GLOBAL"] {
            let d = decide_scope(Some(outcome(label, 0.99)));
            assert_eq!(d.scope, SoulScope::Repo, "label {label:?} must not promote");
            assert_eq!(d.source, DecisionSource::Default);
        }
    }

    #[test]
    fn choice_options_include_ask_escape() {
        // jev guidance: include an 'other' option when the taxonomy may not
        // be exhaustive — scope decisions qualify.
        let opts = scope_choice_options();
        assert!(opts.iter().any(|o| o.label == "ask"));
        assert_eq!(opts.len(), 3);
    }

    #[test]
    fn outcome_json_parses_with_defaults() {
        // Forward-compat: partial payloads still parse (serde defaults)
        let o: ChoiceOutcome =
            serde_json::from_str(r#"{"choice":"repo"}"#).expect("minimal payload must parse");
        assert_eq!(o.choice, "repo");
        assert_eq!(o.confidence, 0.0);
    }

    #[test]
    fn prompt_truncates_giant_bodies() {
        let body = "x".repeat(5000);
        let p = scope_prompt("t", &body);
        assert!(p.len() < 2500, "prompt must bound lesson body: {}", p.len());
        assert!(p.contains("Topic: t"));
    }
}
