//! `b00t audit` — audit trail reader for `.b00t/audit.jsonl`
//!
//! # Usage
//! ```bash
//! b00t-cli audit trail                          # last 20 entries
//! b00t-cli audit trail --stage fitness          # filter by stage
//! b00t-cli audit trail --limit 5               # fewer entries
//! b00t-cli audit trail --path /tmp/audit.jsonl  # custom path
//! b00t-cli audit trail --json                  # raw JSON output
//! ```

use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Clone)]
pub enum AuditCommands {
    #[clap(about = "Read audit trail from .b00t/audit.jsonl")]
    Trail {
        #[clap(
            long,
            help = "Path to audit JSONL file",
            default_value = ".b00t/audit.jsonl"
        )]
        path: PathBuf,
        #[clap(long, help = "Filter by stage (fitness, merge, etc.)")]
        stage: Option<String>,
        #[clap(long, help = "Number of recent entries", default_value_t = 20)]
        limit: usize,
        #[clap(long, help = "Emit as JSON")]
        json: bool,
    },
}

pub fn handle_audit_command(args: &AuditCommands) -> Result<()> {
    match args {
        AuditCommands::Trail {
            path,
            stage,
            limit,
            json,
        } => {
            let content = fs::read_to_string(path)
                .with_context(|| format!("failed to read audit file: {}", path.display()))?;

            // Parse each JSON line, collect valid entries
            let mut entries: Vec<serde_json::Value> = Vec::new();
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                match serde_json::from_str::<serde_json::Value>(line) {
                    Ok(entry) => {
                        // Apply stage filter if specified
                        if let Some(stage_filter) = stage {
                            let entry_stage = entry
                                .get("stage")
                                .or_else(|| entry.get("event"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            if entry_stage != stage_filter.as_str() {
                                continue;
                            }
                        }
                        entries.push(entry);
                    }
                    Err(_) => {
                        // Skip malformed lines
                        continue;
                    }
                }
            }

            // Apply limit (most recent)
            let total = entries.len();
            let start = total.saturating_sub(*limit);
            let shown: Vec<&serde_json::Value> = entries.iter().skip(start).collect();

            // Also collect stage counts for summary
            let mut stage_counts: std::collections::BTreeMap<String, usize> =
                std::collections::BTreeMap::new();
            for entry in &entries {
                let s = entry
                    .get("stage")
                    .or_else(|| entry.get("event"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                *stage_counts.entry(s).or_insert(0) += 1;
            }

            if *json {
                // Emit raw JSON array of filtered entries
                let output: Vec<&serde_json::Value> = shown.iter().copied().collect();
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "🥾 Audit trail: {} ({} total, showing last {})",
                    path.display(),
                    total,
                    shown.len()
                );
                if let Some(stage_filter) = stage {
                    println!("   (filtered by stage: {})", stage_filter);
                }
                println!();

                // Stage summary
                println!("   Stages:");
                for (s, count) in &stage_counts {
                    println!("     {:.<24} {}", s, count);
                }
                println!();

                // Entry details
                for entry in &shown {
                    let timestamp = entry
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let entry_stage = entry
                        .get("stage")
                        .or_else(|| entry.get("event"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");
                    let result = entry
                        .get("result")
                        .or_else(|| entry.get("status"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("?");

                    // Show brief args summary if present
                    let args_summary = entry
                        .get("args")
                        .and_then(|v| v.as_array())
                        .map(|a| {
                            let s: Vec<&str> = a.iter().filter_map(|v| v.as_str()).collect();
                            if s.is_empty() {
                                String::new()
                            } else {
                                format!(" ({})", s.join(" "))
                            }
                        })
                        .unwrap_or_default();

                    println!(
                        "  [{}] {} | {} | {}{}",
                        timestamp, entry_stage, result, entry_stage, args_summary
                    );
                }
            }

            Ok(())
        }
    }
}
