// b00t-c0re-lib/src/sudo_operator/verdict.rs
// 🤓 Adversarial review: queries sm0l (fast/cheap — this is a bounded
//    classification decision, not open-ended generation) with the command,
//    justification, and `git show --stat` of any cited commits, and parses
//    a structured Grant/Deny/Escalate verdict.
//
//    Safety property: any failure to get a clean, parseable Grant/Deny
//    verdict from the model defaults to Escalate, NEVER to Grant. An
//    ambiguous or malformed model response must never silently authorize
//    a privileged command.

use super::governance::{SudoDisposition, SudoReviewEvent};
use anyhow::Result;
use duct::cmd;
use serde::Deserialize;
use std::path::Path;

/// Default sm0l OpenAI-compatible endpoint. `b00t exec` runs on the host,
/// not inside the k8s cluster, so this must be the NodePort (30801), not
/// the in-cluster Service port (8000) that hive.toml's B00T_AI_CH0NKY_BASE-
/// style vars assume for pod-to-pod traffic. Overridable via env.
const DEFAULT_SM0L_BASE: &str = "http://127.0.0.1:30801/v1";
const SM0L_API_KEY: &str = "local-b00t"; // default; overridable via B00T_AI_SM0L_KEY

/// Resolve `git show --stat` for each cited commit hash, in `project_root`.
/// Commits that fail to resolve are simply omitted — treated as
/// unverifiable, which the review prompt is told about explicitly so the
/// model can weigh an unverifiable citation as weaker evidence.
fn resolve_cited_commits(project_root: &Path, cited_commits: &[String]) -> Vec<String> {
    cited_commits
        .iter()
        .filter_map(|hash| {
            cmd!("git", "show", "--stat", hash)
                .dir(project_root)
                .read()
                .ok()
        })
        .collect()
}

fn build_review_event(
    project_root: &Path,
    command: &str,
    justification: &str,
    cited_commits: &[String],
) -> SudoReviewEvent {
    let cited_commit_evidence = resolve_cited_commits(project_root, cited_commits);
    SudoReviewEvent {
        command: command.to_string(),
        justification: justification.to_string(),
        cited_commits: cited_commits.to_vec(),
        cited_commit_evidence,
    }
}

fn build_prompt(event: &SudoReviewEvent) -> String {
    let unverifiable = event.cited_commits.len().saturating_sub(event.cited_commit_evidence.len());
    let evidence_block = if event.cited_commit_evidence.is_empty() {
        "(no cited commits resolved in this repo)".to_string()
    } else {
        event.cited_commit_evidence.join("\n---\n")
    };

    format!(
        r#"You are a security gate reviewing a request to run a privileged/disruptive command.
Respond with ONLY a JSON object, no other text: {{"disposition": "grant"|"deny"|"escalate", "reason": "<one sentence>", "ttl_seconds": <int, only if grant>}}

Rules:
- "grant" only if the justification is specific and plausible, AND either no commits were cited or at least one cited commit's diff genuinely supports the justification.
- "deny" if the justification is vague, generic, or contradicted by the cited diff.
- "escalate" if you are not confident either way, or the command's blast radius is ambiguous — this is the SAFE default when uncertain.
- {unverifiable} of {total} cited commit(s) could not be resolved in this repository — treat unresolved citations as weaker evidence, lean toward escalate/deny if the justification otherwise relies on them.

COMMAND: {command}

JUSTIFICATION: {justification}

CITED COMMIT EVIDENCE:
{evidence_block}
"#,
        unverifiable = unverifiable,
        total = event.cited_commits.len(),
        command = event.command,
        justification = event.justification,
        evidence_block = evidence_block,
    )
}

#[derive(Debug, Deserialize)]
struct RawVerdict {
    disposition: String,
    reason: String,
    #[serde(default)]
    ttl_seconds: Option<u64>,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Deserialize)]
struct ChatMessage {
    content: String,
}

/// Extract the first `{...}` JSON object from model output — models often
/// wrap JSON in prose or markdown fences despite instructions not to.
fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end > start {
        Some(&text[start..=end])
    } else {
        None
    }
}

/// The adversarial-review path's own verdict space — deliberately narrower
/// than `SudoDisposition` (no `VettedGrant`: that variant is only ever
/// produced by `check_vetted()` on the deterministic `--vetted` path, never
/// by this LLM-judged path). Keeping this as its own type makes that
/// exclusion a compile-time fact for every match over an
/// `adversarial_review()` result, rather than a comment + `unreachable!()`
/// at each call site.
#[derive(Debug, Clone, PartialEq)]
pub enum AdversarialVerdict {
    /// Command may execute; grant expires after ttl_seconds.
    Grant { ttl_seconds: u64 },
    /// Command must not execute.
    Deny { reason: String },
    /// The model can't confidently decide — escalate to a human, deny for now.
    Escalate { reason: String },
}

impl From<AdversarialVerdict> for SudoDisposition {
    fn from(verdict: AdversarialVerdict) -> Self {
        match verdict {
            AdversarialVerdict::Grant { ttl_seconds } => SudoDisposition::Grant { ttl_seconds },
            AdversarialVerdict::Deny { reason } => SudoDisposition::Deny { reason },
            AdversarialVerdict::Escalate { reason } => SudoDisposition::Escalate { reason },
        }
    }
}

fn parse_verdict(raw_content: &str) -> AdversarialVerdict {
    let Some(json_str) = extract_json_object(raw_content) else {
        return AdversarialVerdict::Escalate {
            reason: "adversarial model response contained no parseable JSON".into(),
        };
    };

    let Ok(raw) = serde_json::from_str::<RawVerdict>(json_str) else {
        return AdversarialVerdict::Escalate {
            reason: "adversarial model response JSON did not match expected shape".into(),
        };
    };

    match raw.disposition.to_lowercase().as_str() {
        "grant" => AdversarialVerdict::Grant {
            ttl_seconds: raw.ttl_seconds.unwrap_or(300),
        },
        "deny" => AdversarialVerdict::Deny { reason: raw.reason },
        "escalate" => AdversarialVerdict::Escalate { reason: raw.reason },
        other => AdversarialVerdict::Escalate {
            reason: format!("adversarial model returned unrecognized disposition '{other}'"),
        },
    }
}

/// Perform the full adversarial review: build the event, query sm0l,
/// parse the verdict. Never returns Grant on any error path — falls back
/// to Escalate so a network/parse failure can't silently authorize a
/// privileged command.
pub fn adversarial_review(
    project_root: &Path,
    command: &str,
    justification: &str,
    cited_commits: &[String],
) -> Result<(SudoReviewEvent, AdversarialVerdict)> {
    let event = build_review_event(project_root, command, justification, cited_commits);
    let prompt = build_prompt(&event);

    let base_url = std::env::var("B00T_AI_SM0L_BASE").unwrap_or_else(|_| DEFAULT_SM0L_BASE.to_string());
    let api_key = std::env::var("B00T_AI_SM0L_KEY").unwrap_or_else(|_| SM0L_API_KEY.to_string());
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": "sm0l",
        "temperature": 0.0,
        "max_tokens": 200,
        "messages": [{"role": "user", "content": prompt}],
    });

    // `handle_exec` (the only caller today) is a sync fn but runs on a
    // thread owned by b00t-cli's #[tokio::main] runtime, so a plain
    // `reqwest::blocking::Client` panics ("Cannot drop a runtime in a
    // context where blocking is not allowed"). block_in_place + block_on
    // is the correct way to do blocking-style work from sync code that's
    // nested inside an already-running multi-threaded tokio runtime.
    // The whole send+parse round-trip happens in one async block so we
    // only pay the block_in_place cost once.
    let outcome: std::result::Result<ChatCompletionResponse, String> =
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let client = reqwest::Client::new();
                let resp = client
                    .post(&url)
                    .bearer_auth(&api_key)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| format!("failed to reach adversarial model endpoint: {e}"))?;

                if !resp.status().is_success() {
                    return Err(format!(
                        "adversarial model endpoint returned {}",
                        resp.status()
                    ));
                }

                resp.json::<ChatCompletionResponse>()
                    .await
                    .map_err(|e| format!("failed to parse adversarial model response: {e}"))
            })
        });

    let disposition = match outcome {
        Ok(parsed) => match parsed.choices.first() {
            Some(choice) => parse_verdict(&choice.message.content),
            None => AdversarialVerdict::Escalate {
                reason: "adversarial model returned no choices".into(),
            },
        },
        Err(reason) => AdversarialVerdict::Escalate { reason },
    };

    Ok((event, disposition))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_object_plain() {
        let text = r#"{"disposition": "grant", "reason": "ok", "ttl_seconds": 60}"#;
        assert_eq!(extract_json_object(text), Some(text));
    }

    #[test]
    fn test_extract_json_object_wrapped_in_prose() {
        let text = "Sure, here is my verdict:\n```json\n{\"disposition\": \"deny\", \"reason\": \"no\"}\n```\nHope that helps!";
        let extracted = extract_json_object(text).unwrap();
        assert!(extracted.starts_with('{') && extracted.ends_with('}'));
        let parsed: RawVerdict = serde_json::from_str(extracted).unwrap();
        assert_eq!(parsed.disposition, "deny");
    }

    #[test]
    fn test_extract_json_object_none() {
        assert_eq!(extract_json_object("no json here"), None);
    }

    #[test]
    fn test_parse_verdict_grant() {
        let v = parse_verdict(r#"{"disposition": "grant", "reason": "plausible", "ttl_seconds": 120}"#);
        assert_eq!(v, AdversarialVerdict::Grant { ttl_seconds: 120 });
    }

    #[test]
    fn test_parse_verdict_grant_default_ttl() {
        let v = parse_verdict(r#"{"disposition": "GRANT", "reason": "plausible"}"#);
        assert_eq!(v, AdversarialVerdict::Grant { ttl_seconds: 300 });
    }

    #[test]
    fn test_parse_verdict_deny() {
        let v = parse_verdict(r#"{"disposition": "deny", "reason": "vague justification"}"#);
        assert_eq!(v, AdversarialVerdict::Deny { reason: "vague justification".into() });
    }

    #[test]
    fn test_parse_verdict_malformed_json_escalates_not_grants() {
        let v = parse_verdict("not json at all");
        assert!(matches!(v, AdversarialVerdict::Escalate { .. }));
    }

    #[test]
    fn test_parse_verdict_unrecognized_disposition_escalates() {
        let v = parse_verdict(r#"{"disposition": "maybe", "reason": "unsure"}"#);
        assert!(matches!(v, AdversarialVerdict::Escalate { .. }));
    }

    #[test]
    fn test_parse_verdict_missing_field_escalates() {
        let v = parse_verdict(r#"{"foo": "bar"}"#);
        assert!(matches!(v, AdversarialVerdict::Escalate { .. }));
    }
}
