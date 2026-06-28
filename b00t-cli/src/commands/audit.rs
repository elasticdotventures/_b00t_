//! `b00t audit` — audit trail reader for `~/.b00t/exec-log.jsonl`
//!
//! # Usage
//! ```bash
//! b00t-cli audit trail                          # last 20 entries
//! b00t-cli audit trail --stage block-rejected   # filter by stage/result/event
//! b00t-cli audit trail --limit 5                # fewer entries
//! b00t-cli audit trail --path /tmp/audit.jsonl  # custom path
//! b00t-cli audit trail --json                   # raw JSON output
//! ```

use anyhow::Result;
use clap::Parser;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

#[derive(Parser, Clone)]
pub enum AuditCommands {
    #[clap(about = "Read audit trail from ~/.b00t/exec-log.jsonl")]
    Trail {
        #[clap(
            long,
            help = "Path to audit JSONL file",
            default_value = "~/.b00t/exec-log.jsonl"
        )]
        path: PathBuf,
        #[clap(long, help = "Filter by stage/event/result")]
        stage: Option<String>,
        #[clap(long, help = "Number of recent entries", default_value_t = 20)]
        limit: usize,
        #[clap(long, help = "Emit as JSON")]
        json: bool,
    },
}

fn expand_audit_path(path: &PathBuf) -> PathBuf {
    PathBuf::from(shellexpand::tilde(&path.to_string_lossy()).to_string())
}

fn entry_kind(entry: &serde_json::Value) -> &str {
    entry
        .get("stage")
        .or_else(|| entry.get("event"))
        .or_else(|| entry.get("result"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
}

fn entry_timestamp(entry: &serde_json::Value) -> &str {
    entry
        .get("timestamp")
        .or_else(|| entry.get("ts"))
        .and_then(|v| v.as_str())
        .unwrap_or("?")
}

fn entry_result(entry: &serde_json::Value) -> &str {
    entry
        .get("result")
        .or_else(|| entry.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("?")
}

fn entry_args_summary(entry: &serde_json::Value) -> String {
    if let Some(args) = entry.get("args").and_then(|v| v.as_array()) {
        let parts: Vec<&str> = args.iter().filter_map(|v| v.as_str()).collect();
        if !parts.is_empty() {
            return format!(" ({})", parts.join(" "));
        }
    }

    entry
        .get("cmd")
        .and_then(|v| v.as_str())
        .map(|cmd| format!(" ({cmd})"))
        .unwrap_or_default()
}

fn load_audit_entries(path: &PathBuf, stage: &Option<String>) -> Result<Vec<serde_json::Value>> {
    let path = expand_audit_path(path);
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };

    let mut entries = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some(stage_filter) = stage {
            if entry_kind(&entry) != stage_filter.as_str() {
                continue;
            }
        }
        entries.push(entry);
    }

    Ok(entries)
}

pub fn handle_audit_command(args: &AuditCommands) -> Result<()> {
    match args {
        AuditCommands::Trail {
            path,
            stage,
            limit,
            json,
        } => {
            let entries = load_audit_entries(path, stage)?;

            let total = entries.len();
            let start = total.saturating_sub(*limit);
            let shown: Vec<&serde_json::Value> = entries.iter().skip(start).collect();

            let mut stage_counts: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for entry in &entries {
                *stage_counts
                    .entry(entry_kind(entry).to_string())
                    .or_insert(0) += 1;
            }

            if *json {
                let output: Vec<&serde_json::Value> = shown.iter().copied().collect();
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                let display_path = expand_audit_path(path);
                println!(
                    "🥾 Audit trail: {} ({} total, showing last {})",
                    display_path.display(),
                    total,
                    shown.len()
                );
                if let Some(stage_filter) = stage {
                    println!("   (filtered by stage: {})", stage_filter);
                }
                println!();

                println!("   Stages:");
                if stage_counts.is_empty() {
                    println!("     none.................... 0");
                } else {
                    for (s, count) in &stage_counts {
                        println!("     {:.<24} {}", s, count);
                    }
                }
                println!();

                for entry in &shown {
                    let timestamp = entry_timestamp(entry);
                    let kind = entry_kind(entry);
                    let result = entry_result(entry);
                    let args_summary = entry_args_summary(entry);

                    println!("  [{}] {} | {}{}", timestamp, kind, result, args_summary);
                }
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_audit_file_loads_as_empty_entries() {
        let path = PathBuf::from("/tmp/definitely-missing-b00t-audit.jsonl");
        let entries = load_audit_entries(&path, &None).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn exec_log_entries_filter_by_result_alias() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exec-log.jsonl");
        fs::write(
            &path,
            r#"{"ts":"2026-06-21T00:00:00Z","cmd":"git status","result":"allow:direct","guard_msg":null,"pid":123}
{"ts":"2026-06-21T00:00:01Z","cmd":"pip install flask","result":"block-rejected","guard_msg":"use uv","pid":null}
"#,
        )
        .unwrap();

        let entries = load_audit_entries(&path, &Some("block-rejected".to_string())).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entry_kind(&entries[0]), "block-rejected");
        assert_eq!(entry_timestamp(&entries[0]), "2026-06-21T00:00:01Z");
        assert_eq!(entry_args_summary(&entries[0]), " (pip install flask)");
    }
}
