//! sm0l validation gate for t00n-serialized FOCUS records.
//!
//! Calls a local sm0l language model to validate FOCUS records against
//! FocusSchema::requirements() — the schema datum IS the requirements source.
//! No separate reqif.yaml file needed (YAML is not a datum invariant).
//!
//! # Usage
//! ```bash
//! b00t-cli validate --t00n records.t00n
//! b00t-cli validate --stdin < records.t00n
//! ```
//!
//! # Gate protocol
//! Output: one line per requirement
//! PASS: <requirement-id>
//! FAIL: <requirement-id>: <reason>

use crate::datum_schema::{AbDataRequirement, FocusSchema};
use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

const SM0L_MODEL: &str = "ch0nky";
const SM0L_TIMEOUT_SEC: u64 = 30;

#[derive(Parser, Clone)]
pub struct ValidateArgs {
    #[arg(long, help = "Path to t00n-encoded FOCUS record file")]
    pub t00n: Option<PathBuf>,

    #[arg(long, help = "Read t00n from stdin")]
    pub stdin: bool,

    #[arg(long, help = "Model endpoint URL (default: localhost:8001)")]
    pub endpoint: Option<String>,

    #[arg(long, help = "Output raw PASS/FAIL without explanation")]
    pub quiet: bool,
}

// ── Validation result ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ValidationResult {
    requirement_id: String,
    passed: bool,
    reason: String,
}

// ── Prompt construction ──────────────────────────────────────────────────────

fn build_validation_prompt(t00n_data: &str, reqs: &[AbDataRequirement]) -> String {
    let mut prompt = String::from(
        "You are a FOCUS v1.3 compliance validator. Given the following t00n-encoded FOCUS records, \
         verify each requirement below. Respond with EXACTLY one line per requirement in the format:\n\
         PASS: <id>\n\
         FAIL: <id>: <reason>\n\n"
    );
    prompt.push_str("--- t00n records ---\n");
    prompt.push_str(t00n_data);
    prompt.push_str("\n--- requirements ---\n");
    for req in reqs {
        prompt.push_str(&format!("[{}] {} (header={}, constraint={})\n", req.id, req.statement, req.header, req.constraint));
    }
    prompt.push_str("\n--- response ---\n");
    prompt
}

// ── Model call ───────────────────────────────────────────────────────────────

fn call_sm0l_model(prompt: &str, endpoint: &str) -> Result<String> {
    use std::io::Write;
    let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));
    let payload = serde_json::json!({
        "model": SM0L_MODEL,
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 512,
        "temperature": 0.0,
    });

    // Write payload to temp file to avoid shell escaping issues
    let tmp = std::env::temp_dir().join("b00t-validate-payload.json");
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(serde_json::to_string_pretty(&payload)?.as_bytes())?;

    let output = std::process::Command::new("curl")
        .args([
            "-s", "--max-time", &SM0L_TIMEOUT_SEC.to_string(),
            "-H", "content-type: application/json",
            "-d", &format!("@{}", tmp.display()),
            &url,
        ])
        .output()
        .context("curl sm0l model request — is ch0nky running on the endpoint?")?;

    let _ = std::fs::remove_file(&tmp);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("curl failed (exit={:?}): {stderr}", output.status.code());
    }

    let body: serde_json::Value = serde_json::from_slice(&output.stdout)
        .context("parse model response JSON")?;
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string();
    Ok(content)
}

// ── Parse model response ─────────────────────────────────────────────────────

fn parse_validation_response(response: &str, reqs: &[AbDataRequirement]) -> Vec<ValidationResult> {
    let mut results = Vec::new();
    let req_map: HashMap<&str, &AbDataRequirement> = reqs.iter().map(|r| (r.id.as_str(), r)).collect();

    for line in response.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(rest) = line.strip_prefix("PASS: ") {
            let id = rest.trim();
            if req_map.contains_key(id) {
                results.push(ValidationResult {
                    requirement_id: id.to_string(),
                    passed: true,
                    reason: String::new(),
                });
            }
        } else if let Some(rest) = line.strip_prefix("FAIL: ") {
            if let Some((id, reason)) = rest.split_once(':') {
                if req_map.contains_key(id.trim()) {
                    results.push(ValidationResult {
                        requirement_id: id.trim().to_string(),
                        passed: false,
                        reason: reason.trim().to_string(),
                    });
                }
            }
        }
    }

    // Fill in missing results as FAIL
    for req in reqs {
        if !results.iter().any(|r| r.requirement_id == req.id) {
            results.push(ValidationResult {
                requirement_id: req.id.clone(),
                passed: false,
                reason: "no response from model".to_string(),
            });
        }
    }

    results
}

// ── Main handler ─────────────────────────────────────────────────────────────

pub fn handle_validate(args: &ValidateArgs) -> Result<()> {
    let t00n_data = if args.stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else if let Some(path) = &args.t00n {
        std::fs::read_to_string(path).context("read t00n file")?
    } else {
        anyhow::bail!("provide --t00n <file> or --stdin");
    };

    // Requirements come from the schema datum itself — zero drift.
    let schema = FocusSchema::new();
    let reqs = schema.requirements();

    let endpoint = args.endpoint.clone().unwrap_or_else(|| "http://localhost:8001".to_string());
    let prompt = build_validation_prompt(&t00n_data, &reqs);
    let response = call_sm0l_model(&prompt, &endpoint).context("sm0l model call failed — is ch0nky running on :8001?")?;
    let results = parse_validation_response(&response, &reqs);

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();

    for r in &results {
        if r.passed {
            println!("PASS: {}", r.requirement_id);
        } else if args.quiet {
            println!("FAIL: {}", r.requirement_id);
        } else {
            println!("FAIL: {}: {}", r.requirement_id, r.reason);
        }
    }

    eprintln!("validation complete: {passed} passed, {failed} failed");
    if failed > 0 { std::process::exit(1); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_reqs() -> Vec<AbDataRequirement> {
        vec![
            AbDataRequirement {
                id: "REQ-FOCUS-001".into(),
                statement: "Every record MUST have a BillingAccountId".into(),
                header: "BillingAccountId".into(),
                constraint: "required".into(),
            },
            AbDataRequirement {
                id: "REQ-FOCUS-002".into(),
                statement: "Every record MUST have a non-null BilledCost".into(),
                header: "BilledCost".into(),
                constraint: "required".into(),
            },
        ]
    }

    #[test]
    fn test_build_validation_prompt_includes_reqs() {
        let reqs = sample_reqs();
        let prompt = build_validation_prompt("data[1]{a}: 1", &reqs);
        assert!(prompt.contains("REQ-FOCUS-001"));
        assert!(prompt.contains("BillingAccountId"));
        assert!(prompt.contains("PASS: <id>"));
    }

    #[test]
    fn test_parse_validation_response_all_pass() {
        let reqs = sample_reqs();
        let response = "PASS: REQ-FOCUS-001\nPASS: REQ-FOCUS-002\n";
        let results = parse_validation_response(response, &reqs);
        assert_eq!(results.len(), 2);
        assert!(results[0].passed);
        assert!(results[1].passed);
    }

    #[test]
    fn test_parse_validation_response_some_fail() {
        let reqs = sample_reqs();
        let response = "PASS: REQ-FOCUS-001\nFAIL: REQ-FOCUS-002: missing BilledCost field\n";
        let results = parse_validation_response(response, &reqs);
        assert_eq!(results.len(), 2);
        assert!(results[0].passed);
        assert!(!results[1].passed);
        assert_eq!(results[1].reason, "missing BilledCost field");
    }

    #[test]
    fn test_parse_validation_response_fills_missing() {
        let reqs = sample_reqs();
        let response = "PASS: REQ-FOCUS-001\n";
        let results = parse_validation_response(response, &reqs);
        assert_eq!(results.len(), 2);
        assert!(!results[1].passed);
        assert_eq!(results[1].reason, "no response from model");
    }

    #[test]
    fn test_parse_validation_response_ignores_unrecognized_ids() {
        let reqs = sample_reqs();
        let response = "PASS: REQ-FOCUS-001\nPASS: REQ-FOCUS-999\n";
        let results = parse_validation_response(response, &reqs);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_call_sm0l_model_returns_error_when_curl_fails() {
        let result = call_sm0l_model("test", "http://localhost:1");
        assert!(result.is_err());
    }
}
