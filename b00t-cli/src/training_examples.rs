//! Convert evidence/satisfies.jsonl PASS entries into verified training examples (#595).
//!
//! Each PASS entry becomes a fine-tuning example demonstrating the verify→accept loop.
//! These are ground-truth behaviors — the model should learn from them because
//! they already passed formal gate checks.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Raw evidence entry from satisfies.jsonl.
#[derive(Debug, Deserialize)]
struct EvidenceEntry {
    subject: String,
    predicate: String,
    object: EvidenceObject,
    timestamp: String,
}

#[derive(Debug, Deserialize)]
struct EvidenceObject {
    result: String,
    sha: Option<String>,
    file: Option<String>,
    rule: Option<String>,
    assertion: Option<String>,
}

/// Training example for fine-tuning.
#[derive(Debug, Serialize)]
struct TrainingExample {
    instruction: String,
    response: String,
    source: String,
    verified_at: String,
}

/// Convert satisfies.jsonl to training examples, keeping only PASS entries.
pub fn convert_to_training(
    evidence_path: &Path,
    output_path: &Path,
) -> Result<(usize, usize)> {
    let content = std::fs::read_to_string(evidence_path)
        .with_context(|| format!("read {}", evidence_path.display()))?;

    let mut total = 0usize;
    let mut pass_count = 0usize;
    let mut examples = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        total += 1;

        let entry: EvidenceEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue,
        };

        if entry.object.result != "PASS" {
            continue;
        }
        pass_count += 1;

        let subject_name = entry.subject.replace(".toml", "").replace(".gate", "");
        let _rule = entry.object.rule.as_deref().unwrap_or("gate-check");
        let assertion = entry.object.assertion.as_deref().unwrap_or("validates");
        let file = entry.object.file.as_deref().unwrap_or("unknown");

        let instruction = format!(
            "Verify that {} {} the constraint {}",
            subject_name, entry.predicate, assertion
        );

        let response = format!(
            "[verify: {}] → [result: PASS, sha: {}] → Gate satisfied. File: {}",
            assertion,
            entry.object.sha.as_deref().unwrap_or("n/a"),
            file
        );

        examples.push(TrainingExample {
            instruction,
            response,
            source: entry.subject.clone(),
            verified_at: entry.timestamp.clone(),
        });
    }

    let out = std::fs::File::create(output_path)
        .with_context(|| format!("create {}", output_path.display()))?;
    let mut writer = std::io::BufWriter::new(out);

    for ex in &examples {
        serde_json::to_writer(&mut writer, ex)?;
        use std::io::Write;
        writeln!(writer)?;
    }

    Ok((total, pass_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn converts_pass_entries() {
        let tmp = std::env::temp_dir().join("b00t-test-convert");
        std::fs::create_dir_all(&tmp).unwrap();

        let evidence = tmp.join("satisfies.jsonl");
        let mut f = std::fs::File::create(&evidence).unwrap();
        writeln!(f, r#"{{"subject":"write-guard","predicate":"validates","object":{{"result":"PASS","sha":"abc123","file":"guard.toml","rule":"no-shell-injection","assertion":"safe"}},"timestamp":"2026-01-01T00:00:00Z"}}"#).unwrap();
        writeln!(f, r#"{{"subject":"bad-gate","predicate":"validates","object":{{"result":"FAIL","sha":"bad"}},"timestamp":"2026-01-01T00:00:00Z"}}"#).unwrap();

        let output = tmp.join("train.jsonl");
        let (total, pass) = convert_to_training(&evidence, &output).unwrap();
        assert_eq!(total, 2);
        assert_eq!(pass, 1);

        let content = std::fs::read_to_string(&output).unwrap();
        assert!(content.contains("verify"));
        assert!(content.contains("PASS"));
        assert!(content.contains("write-guard"));
        assert!(!content.contains("bad-gate"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn empty_file_returns_zero() {
        let tmp = std::env::temp_dir().join("b00t-test-empty");
        std::fs::create_dir_all(&tmp).unwrap();
        let evidence = tmp.join("satisfies.jsonl");
        std::fs::write(&evidence, "").unwrap();
        let output = tmp.join("train.jsonl");
        let (total, pass) = convert_to_training(&evidence, &output).unwrap();
        assert_eq!(total, 0);
        assert_eq!(pass, 0);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
