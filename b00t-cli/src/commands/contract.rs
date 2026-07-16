//! `b00t contract run` — deterministic service-contract reward path.

use anyhow::{Result, anyhow, bail};
use chrono::Utc;
use clap::Parser;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crate::ServiceContract;

const DEFAULT_EVIDENCE_LOG: &str = ".b00t/evidence.jsonl";

#[derive(Parser, Debug)]
pub enum ContractCommands {
    #[clap(about = "Execute a datum [[service_contract]] handler and append EvidenceRecord")]
    Run {
        #[clap(help = "Datum key or base name (e.g. qwen3-coder-local)")]
        datum: String,

        #[clap(long, help = "Select a service_contract by capability")]
        capability: Option<String>,

        #[clap(
            long,
            help = "Override evidence JSONL path (default: .b00t/evidence.jsonl)"
        )]
        evidence_log: Option<PathBuf>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EvidenceRecord {
    pub contract_id: String,
    pub handler: String,
    pub exit_code: i32,
    pub evidence_match: bool,
    pub duration_ms: u128,
    pub tokens: u64,
    pub git_sha: String,
    pub ts: String,
}

pub fn handle_contract_command(args: &ContractCommands, datum_path: &str) -> Result<()> {
    match args {
        ContractCommands::Run {
            datum,
            capability,
            evidence_log,
        } => {
            let evidence_path = evidence_log
                .clone()
                .unwrap_or_else(|| PathBuf::from(DEFAULT_EVIDENCE_LOG));
            let record = run_contract(datum_path, datum, capability.as_deref(), &evidence_path)?;
            println!(
                "{}: contract_id={} exit_code={} evidence_match={} duration_ms={}",
                if record.exit_code == 0 && record.evidence_match {
                    "PASS"
                } else {
                    "FAIL"
                },
                record.contract_id,
                record.exit_code,
                record.evidence_match,
                record.duration_ms
            );
            if record.exit_code != 0 || !record.evidence_match {
                bail!("contract failed: {}", record.contract_id);
            }
            Ok(())
        }
    }
}

pub fn run_contract(
    datum_path: &str,
    datum: &str,
    capability: Option<&str>,
    evidence_log: &Path,
) -> Result<EvidenceRecord> {
    let (config, filename) = crate::get_config(datum, datum_path)
        .map_err(|e| anyhow!("datum '{}' not found at {}: {}", datum, datum_path, e))?;
    let datum_key = datum_key_from_filename(&filename);
    let contract = select_contract(&config.service_contract, capability)?;
    let contract_id = format!("{}:{}", datum_key, contract.capability);
    let evidence_re = Regex::new(&contract.evidence)
        .map_err(|e| anyhow!("invalid evidence regex for {contract_id}: {e}"))?;
    let argv = shlex::split(&contract.handler).ok_or_else(|| {
        anyhow!(
            "cannot parse handler for {contract_id}: {}",
            contract.handler
        )
    })?;
    if argv.is_empty() {
        bail!("empty handler for {contract_id}");
    }

    let start = Instant::now();
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|e| anyhow!("handler spawn failed for {contract_id}: {e}"))?;
    let duration_ms = start.elapsed().as_millis();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let exit_code = output.status.code().unwrap_or(1);
    let evidence_match = evidence_re.is_match(&stdout);
    let record = EvidenceRecord {
        contract_id,
        handler: contract.handler.clone(),
        exit_code,
        evidence_match,
        duration_ms,
        tokens: 0,
        git_sha: current_git_sha(),
        ts: Utc::now().to_rfc3339(),
    };
    append_evidence_record(evidence_log, &record)?;
    Ok(record)
}

fn select_contract<'a>(
    contracts: &'a [ServiceContract],
    capability: Option<&str>,
) -> Result<&'a ServiceContract> {
    if contracts.is_empty() {
        bail!("datum has no [[service_contract]] entries");
    }
    if let Some(capability) = capability {
        return contracts
            .iter()
            .find(|contract| contract.capability == capability)
            .ok_or_else(|| anyhow!("no service_contract capability '{}'", capability));
    }
    if contracts.len() == 1 {
        Ok(&contracts[0])
    } else {
        bail!(
            "datum has {} service_contract entries; specify --capability",
            contracts.len()
        );
    }
}

fn append_evidence_record(path: &Path, record: &EvidenceRecord) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    serde_json::to_writer(&mut file, record)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn datum_key_from_filename(filename: &str) -> String {
    filename
        .trim_end_matches(".tomllmd")
        .trim_end_matches(".tomllm")
        .trim_end_matches(".toml")
        .to_string()
}

fn current_git_sha() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_datum(dir: &TempDir, body: &str) {
        std::fs::write(dir.path().join("fixture.cli.toml"), body).unwrap();
    }

    #[test]
    fn contract_run_appends_passing_evidence_record() {
        let dir = TempDir::new().unwrap();
        write_datum(
            &dir,
            r#"
[b00t]
name = "fixture"
type = "cli"
hint = "fixture"

[[service_contract]]
capability = "smoke"
handler = "printf 'PASS: smoke\n'"
evidence = "PASS: smoke"
"#,
        );
        let evidence_log = dir.path().join(".b00t/evidence.jsonl");

        let record =
            run_contract(dir.path().to_str().unwrap(), "fixture", None, &evidence_log).unwrap();

        assert_eq!(record.contract_id, "fixture.cli:smoke");
        assert_eq!(record.exit_code, 0);
        assert!(record.evidence_match);
        let line = std::fs::read_to_string(evidence_log).unwrap();
        let parsed: EvidenceRecord = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed.contract_id, "fixture.cli:smoke");
        assert!(parsed.evidence_match);
    }

    #[test]
    fn contract_run_records_missing_evidence_match() {
        let dir = TempDir::new().unwrap();
        write_datum(
            &dir,
            r#"
[b00t]
name = "fixture"
type = "cli"
hint = "fixture"

[[service_contract]]
capability = "smoke"
handler = "printf 'NOPE\n'"
evidence = "PASS: smoke"
"#,
        );
        let evidence_log = dir.path().join(".b00t/evidence.jsonl");

        let err = handle_contract_command(
            &ContractCommands::Run {
                datum: "fixture".to_string(),
                capability: None,
                evidence_log: Some(evidence_log.clone()),
            },
            dir.path().to_str().unwrap(),
        )
        .unwrap_err();

        assert!(err.to_string().contains("contract failed"));
        let line = std::fs::read_to_string(evidence_log).unwrap();
        let parsed: EvidenceRecord = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed.exit_code, 0);
        assert!(!parsed.evidence_match);
    }

    #[test]
    fn contract_run_requires_capability_for_multiple_contracts() {
        let dir = TempDir::new().unwrap();
        write_datum(
            &dir,
            r#"
[b00t]
name = "fixture"
type = "cli"
hint = "fixture"

[[service_contract]]
capability = "a"
handler = "printf a"
evidence = "a"

[[service_contract]]
capability = "b"
handler = "printf b"
evidence = "b"
"#,
        );
        let evidence_log = dir.path().join(".b00t/evidence.jsonl");

        let err =
            run_contract(dir.path().to_str().unwrap(), "fixture", None, &evidence_log).unwrap_err();

        assert!(err.to_string().contains("specify --capability"));
        assert!(!evidence_log.exists());
    }
}
