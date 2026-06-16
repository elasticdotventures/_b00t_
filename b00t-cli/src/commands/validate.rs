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

use crate::datum_schema::{AbDataRequirement, AbDataSchema, FocusJsonlSequence, FocusSchema, MatchMode, SchemaError};
use anyhow::{Context, Result};
use clap::Parser;
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

const SM0L_MODEL: &str = "ch0nky";
const SM0L_TIMEOUT_SEC: u64 = 30;
const FSL_STORE: &str = ".b00t/fsl/focus-examples.jsonl";

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

    #[arg(long, help = "Read FOCUS records from JSONL file and validate against FocusSchema")]
    pub jsonl: Option<PathBuf>,

    #[arg(long, help = "Skip the sm0l model call, only validate schema conformance")]
    pub skip_model: bool,

    #[arg(long, help = "Auto-train model when FSL failures exceed threshold (default: 10)")]
    pub auto_train: bool,

    #[arg(long, help = "Training datum name for --auto-train (default: focus-validator)")]
    pub train_name: Option<String>,

    #[arg(long, help = "FSL failure threshold to trigger auto-train (default: 10)")]
    pub train_threshold: Option<usize>,

    #[arg(long, help = "Schema datum name (default: focus). Loads _b00t_/<name>.schema.tomllmd")]
    pub schema: Option<String>,
}

// ── Validation result ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct ValidationResult {
    requirement_id: String,
    passed: bool,
    reason: String,
}

// ── Prompt construction ──────────────────────────────────────────────────────

/// Load prior failure examples from the few-shot learning store.
/// Returns up to `max` examples, newest first.
fn load_fsl_examples(max: usize) -> Vec<String> {
    let path = PathBuf::from(FSL_STORE);
    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content.lines().rev().take(max).map(|l| l.to_string()).collect()
}

/// Save a failed validation as a new few-shot example.
/// Format: `t00n_input|requirement_id|reason`
fn save_fsl_failure(t00n_data: &str, req_id: &str, reason: &str) {
    let path = PathBuf::from(FSL_STORE);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let line = format!("{}|{}|{}\n", t00n_data.lines().next().unwrap_or(""), req_id, reason);
    let _ = OpenOptions::new().create(true).append(true).open(&path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

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
    // 🤓 model name resolved from: B00T_SM0L_MODEL env > registry (tier=ch0nky) > hardcoded
    let model = std::env::var("B00T_SM0L_MODEL")
        .ok()
        .or_else(|| crate::model_registry::resolve_tier_endpoint("ch0nky")
            .map(|(_, m)| m))
        .unwrap_or_else(|| SM0L_MODEL.to_string());
    // 🤓 model name + endpoint are env-configurable so any OpenAI-compatible
    //    peer (llamacpp, vLLM, mistral.rs, …) can serve as the sm0l validator.
    let model = std::env::var("B00T_SM0L_MODEL").unwrap_or_else(|_| SM0L_MODEL.to_string());
    let base = endpoint.trim_end_matches('/').trim_end_matches("/v1");
    let url = format!("{}/v1/chat/completions", base);
    let payload = serde_json::json!({
        "model": model,
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

// ── Schema resolution ─────────────────────────────────────────────────────────

/// Resolve a `FocusSchema` from a schema datum name.
/// - `None` or `"focus"` → `FocusSchema::new()` (built-in default)
/// - Other names → load `_b00t_/<name>.schema.tomllmd` from project or home
fn resolve_schema(name: Option<&str>) -> Result<FocusSchema> {
    match name {
        None | Some("focus") => Ok(FocusSchema::new()),
        Some(schema_name) => {
            // Search: cwd/_b00t_/, then ~/.dotfiles/_b00t_/
            let candidates = [
                std::env::current_dir()
                    .ok()
                    .map(|d| d.join("_b00t_").join(format!("{schema_name}.schema.tomllmd"))),
                dirs::home_dir()
                    .map(|h| h.join(".dotfiles").join("_b00t_").join(format!("{schema_name}.schema.tomllmd"))),
            ];

            for candidate in candidates.iter().flatten() {
                if candidate.exists() {
                    return FocusSchema::load(&candidate.to_string_lossy())
                        .with_context(|| format!("load schema datum from {candidate:?}"));
                }
            }

            anyhow::bail!(
                "Schema '{schema_name}' not found at _b00t_/{schema_name}.schema.tomllmd \
                 (searched cwd/_b00t_/ and ~/.dotfiles/_b00t_/)"
            );
        }
    }
}

// ── Main handler ─────────────────────────────────────────────────────────────

pub fn handle_validate(args: &ValidateArgs) -> Result<()> {
    // ── JSONL branch ──────────────────────────────────────────────────────────
    if let Some(path) = &args.jsonl {
        return validate_jsonl(args, path);
    }

    let t00n_data = if args.stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)?;
        buf
    } else if let Some(path) = &args.t00n {
        std::fs::read_to_string(path).context("read t00n file")?
    } else {
        anyhow::bail!("provide --t00n <file>, --stdin, or --jsonl <file>");
    };

    // Requirements come from the schema datum itself — zero drift.
    let schema = resolve_schema(args.schema.as_deref())?;
    let reqs = schema.requirements();

    // Build prompt with few-shot learning examples from prior failures
    let fsl_examples = load_fsl_examples(5);
    let extended_input = if fsl_examples.is_empty() {
        t00n_data.clone()
    } else {
        format!("Prior validation failures (learn from these):\n{}\n\nNew records:\n{}",
            fsl_examples.join("\n"), t00n_data)
    };

    // 🤓 endpoint resolution: --endpoint flag > local registry (tier=ch0nky/sm0l)
    //    > B00T_AI_SM0L_BASE env > localhost:8001
    let endpoint = args.endpoint.clone()
        .or_else(|| crate::model_registry::resolve_tier_endpoint("ch0nky")
            .map(|(base, _)| base))
    // 🤓 endpoint resolution: --endpoint flag > B00T_AI_SM0L_BASE env > localhost:8001
    let endpoint = args.endpoint.clone()
        .or_else(|| std::env::var("B00T_AI_SM0L_BASE").ok())
        .unwrap_or_else(|| "http://localhost:8001".to_string());
    let prompt = build_validation_prompt(&extended_input, &reqs);
    let response = call_sm0l_model(&prompt, &endpoint).context("sm0l model call failed — is ch0nky running on :8001?")?;
    let results = parse_validation_response(&response, &reqs);

    let passed = results.iter().filter(|r| r.passed).count();
    let failed = results.iter().filter(|r| !r.passed).count();

    for r in &results {
        if r.passed {
            println!("PASS: {}", r.requirement_id);
        } else {
            // Save failures as few-shot examples for next validation
            save_fsl_failure(&t00n_data, &r.requirement_id, &r.reason);
            if args.quiet {
                println!("FAIL: {}", r.requirement_id);
            } else {
                println!("FAIL: {}: {}", r.requirement_id, r.reason);
            }
        }
    }

    eprintln!("validation complete: {passed} passed, {failed} failed (fsl examples: {})", fsl_examples.len());

    // Auto-train trigger: when failures exceed threshold, kick off `b00t model train`
    if failed > 0 && args.auto_train {
        let threshold = args.train_threshold.unwrap_or(10);
        let current = load_fsl_examples(usize::MAX).len();
        if current >= threshold {
            let train_name = args.train_name.as_deref().unwrap_or("focus-validator");
            eprintln!("   FSL store ({current}) ≥ threshold ({threshold}) — triggering auto-train for '{train_name}'...");
            let status = std::process::Command::new("b00t-cli")
                .args(["model", "train", train_name])
                .status();
            match status {
                Ok(s) if s.success() => eprintln!("   ✅ auto-train complete"),
                Ok(s) => eprintln!("   ⚠️  auto-train failed (exit={:?})", s.code()),
                Err(e) => eprintln!("   ⚠️  auto-train error: {e}"),
            }
        } else {
            eprintln!("   FSL store ({current}) below threshold ({threshold}) — no auto-train");
        }
    }

    if failed > 0 { std::process::exit(1); }
    Ok(())
}

// ── JSONL validation ─────────────────────────────────────────────────────────

/// Validate a JSONL file against FocusSchema, then optionally pass to sm0l model.
fn validate_jsonl(args: &ValidateArgs, path: &PathBuf) -> Result<()> {
    let schema = resolve_schema(args.schema.as_deref())?;
    let mut seq = FocusJsonlSequence::open(&path.to_string_lossy())
        .with_context(|| format!("open JSONL file '{}'", path.display()))?;

    let mut errors: Vec<SchemaError> = Vec::new();
    let mut frame_count = 0;

    for result in &mut seq {
        match result {
            Ok(frame) => {
                frame_count += 1;
                if let Err(frame_errors) = schema.validate(frame, MatchMode::ByName) {
                    errors.extend(frame_errors);
                }
            }
            Err(e) => {
                errors.push(e.clone());
            }
        }
    }

    // Report schema validation results
    if errors.is_empty() {
        println!("PASS: {} frames validated against FocusSchema", frame_count);
    } else {
        for err in &errors {
            println!("FAIL: SchemaError: {}", err.0);
        }
    }

    eprintln!("schema validation complete: {} frames, {} errors", frame_count, errors.len());

    if !errors.is_empty() {
        std::process::exit(1);
    }

    // ── Skip model if requested ───────────────────────────────────────────────
    if args.skip_model {
        return Ok(());
    }

    // ── Proceed to sm0l model call ────────────────────────────────────────────
    let jsonl_data = std::fs::read_to_string(path)
        .with_context(|| format!("read JSONL file '{}'", path.display()))?;
    let reqs = schema.requirements();
    // 🤓 endpoint resolution: --endpoint flag > registry > env > localhost
    let endpoint = args.endpoint.clone()
        .or_else(|| crate::model_registry::resolve_tier_endpoint("ch0nky")
            .map(|(base, _)| base))
    // 🤓 endpoint resolution: --endpoint flag > B00T_AI_SM0L_BASE env > localhost:8001
    let endpoint = args.endpoint.clone()
        .or_else(|| std::env::var("B00T_AI_SM0L_BASE").ok())
        .unwrap_or_else(|| "http://localhost:8001".to_string());
    let prompt = build_validation_prompt(&jsonl_data, &reqs);
    let response = call_sm0l_model(&prompt, &endpoint)
        .context("sm0l model call failed — is ch0nky running on :8001?")?;
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

    eprintln!("model validation complete: {passed} passed, {failed} failed");
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

    #[test]
    fn test_validate_jsonl_valid_file() {
        let dir = std::env::temp_dir().join("b00t-test-validate-jsonl");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("valid.jsonl");

        // A valid JSONL record: all required (non-nullable) FocusSchema fields present
        let record = r#"{"BillingAccountId":"acct1","BillingCurrency":"USD","ServiceProviderName":"AWS","ServiceName":"EC2","SkuId":"sku-1","BilledCost":10.0,"EffectiveCost":9.0,"ChargeCategory":"Usage","ChargeFrequency":"UsageBased","ChargePeriodStart":"2024-01-01","ChargePeriodEnd":"2024-01-02","BillingPeriodStart":"2024-01-01","BillingPeriodEnd":"2024-02-01"}"#;
        std::fs::write(&path, record).unwrap();

        let args = ValidateArgs {
            t00n: None,
            stdin: false,
            endpoint: None,
            quiet: false,
            jsonl: Some(path),
            skip_model: true,
            auto_train: false,
            train_name: None,
            train_threshold: None,
            schema: None,
        };

        let result = handle_validate(&args);
        assert!(result.is_ok(), "valid JSONL should pass: {result:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_validate_jsonl_missing_file() {
        let args = ValidateArgs {
            t00n: None,
            stdin: false,
            endpoint: None,
            quiet: false,
            jsonl: Some(PathBuf::from("/tmp/b00t-test-nonexistent-file.jsonl")),
            skip_model: true,
            auto_train: false,
            train_name: None,
            train_threshold: None,
            schema: None,
        };

        let result = handle_validate(&args);
        assert!(result.is_err(), "missing JSONL should fail: {result:?}");
    }
}
