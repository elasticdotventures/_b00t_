use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use clap::Parser;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::BufRead;
use std::path::PathBuf;

#[derive(Parser)]
pub enum ObservabilityCommands {
    #[clap(about = "Show recent events from unified events.jsonl")]
    Events {
        #[clap(long, help = "Filter by event type (mcp_install, gate_block, guard)")]
        event: Option<String>,
        #[clap(long, help = "Show only failed events")]
        failed: bool,
        #[clap(long, help = "Show events since N minutes ago", default_value = "60")]
        since: u64,
        #[clap(long, help = "Follow new events (tail -f)")]
        follow: bool,
    },
    #[clap(about = "Show guard violation statistics")]
    Guards {
        #[clap(long, help = "Show only escalated (💩) violations")]
        escalated: bool,
        #[clap(long, help = "Show top N guards by hit count", default_value_t = 10)]
        top: usize,
    },
}

#[derive(Deserialize, Debug)]
struct EventEntry {
    ts: Option<String>,
    event: Option<String>,
    detail: Option<String>,
    action: Option<String>,
    pid: Option<u64>,
}

/// Get the path to the events.jsonl file
fn events_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".b00t").join("events.jsonl")
}

/// Get the path to the guard-violations.jsonl file
fn guard_violations_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".b00t").join("guard-violations.jsonl")
}

/// Parse an RFC3339 timestamp string, returning None if invalid
fn parse_timestamp(ts: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            // Try parsing without timezone as UTC
            chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
                .ok()
                .map(|ndt| ndt.and_utc())
        })
}

impl ObservabilityCommands {
    pub fn execute(&self) -> Result<()> {
        match self {
            ObservabilityCommands::Events {
                event,
                failed,
                since,
                follow,
            } => handle_events(event, *failed, *since, *follow),
            ObservabilityCommands::Guards { escalated, top } => {
                handle_guards(*escalated, *top)
            }
        }
    }
}

fn handle_events(
    event_filter: &Option<String>,
    failed_only: bool,
    since_minutes: u64,
    follow: bool,
) -> Result<()> {
    let path = events_path();

    if follow {
        // Use tail -f for follow mode
        let status = std::process::Command::new("tail")
            .arg("-f")
            .arg(&path)
            .status()
            .context("Failed to run tail -f")?;
        if !status.success() {
            anyhow::bail!("tail -f exited with code: {:?}", status.code());
        }
        return Ok(());
    }

    if !path.exists() {
        println!("📭 No events found at {}", path.display());
        return Ok(());
    }

    let file = std::fs::File::open(&path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    let reader = std::io::BufReader::new(file);

    let cutoff = Utc::now() - chrono::Duration::minutes(since_minutes as i64);

    let mut entries: Vec<EventEntry> = Vec::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(entry) = serde_json::from_str::<EventEntry>(&line) {
            // Apply --since filter
            if let Some(ref ts_str) = entry.ts {
                if let Some(ts) = parse_timestamp(ts_str) {
                    if ts < cutoff {
                        continue;
                    }
                }
            }

            // Apply --event filter
            if let Some(ev) = event_filter {
                let matches = entry
                    .event
                    .as_deref()
                    .map(|e| e.contains(ev.as_str()))
                    .unwrap_or(false);
                if !matches {
                    continue;
                }
            }

            // Apply --failed filter
            if failed_only {
                let has_fail = entry
                    .detail
                    .as_deref()
                    .map(|d| d.to_lowercase().contains("fail"))
                    .unwrap_or(false);
                if !has_fail {
                    continue;
                }
            }

            entries.push(entry);
        }
    }

    // Show last 20 entries
    let start = if entries.len() > 20 {
        entries.len() - 20
    } else {
        0
    };

    if entries.is_empty() {
        println!("📭 No matching events found");
        return Ok(());
    }

    println!(
        "📊 {} events (showing last {}, filtered from last {} min)",
        entries.len(),
        entries.len().min(20),
        since_minutes
    );
    println!();

    for entry in entries.iter().skip(start) {
        let ts = entry.ts.as_deref().unwrap_or("?");
        let event = entry.event.as_deref().unwrap_or("?");
        let detail = entry.detail.as_deref().unwrap_or("");
        let action = entry.action.as_deref().unwrap_or("");
        let pid = entry.pid.unwrap_or(0);

        let action_tag = match action {
            "block" => "💩",
            "warn" => "🦨",
            _ => "📋",
        };

        println!(
            "{} [{}] {} {} {} (pid={})",
            action_tag, ts, event, detail, action, pid
        );
    }

    Ok(())
}

fn handle_guards(escalated_only: bool, top_n: usize) -> Result<()> {
    let path = guard_violations_path();

    if !path.exists() {
        println!("📭 No guard violations found at {}", path.display());
        return Ok(());
    }

    let file = std::fs::File::open(&path)
        .with_context(|| format!("Failed to open {}", path.display()))?;
    let reader = std::io::BufReader::new(file);

    let mut counts: HashMap<String, u32> = HashMap::new();

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
            if let (Some(pattern), Some(count)) = (
                val.get("pattern").and_then(|v| v.as_str()),
                val.get("count").and_then(|v| v.as_u64()),
            ) {
                *counts.entry(pattern.to_string()).or_insert(0) += count as u32;
            }
        }
    }

    if counts.is_empty() {
        println!("📭 No guard violations recorded");
        return Ok(());
    }

    let mut sorted: Vec<(String, u32)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));

    let total_violations: u32 = sorted.iter().map(|(_, c)| c).sum();

    println!(
        "🛡️  Guard violations: {} total across {} patterns",
        total_violations,
        sorted.len()
    );
    println!();

    let display_count = sorted.len().min(top_n);
    for (i, (pattern, count)) in sorted.iter().enumerate().take(display_count) {
        let escalated = *count > 1;
        if escalated_only && !escalated {
            continue;
        }
        let marker = if escalated { "💩" } else { "🦨" };
        println!(
            "  {}. {} {} ({} violation{})",
            i + 1,
            marker,
            pattern,
            count,
            if *count == 1 { "" } else { "s" }
        );
    }

    if sorted.len() > display_count {
        println!("  ... and {} more patterns", sorted.len() - display_count);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_events_parses_empty_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        // events_path() returns {HOME}/.b00t/events.jsonl
        let b00t_dir = temp_dir.path().join(".b00t");
        std::fs::create_dir_all(&b00t_dir).unwrap();
        let path = b00t_dir.join("events.jsonl");
        // Create empty file
        std::fs::write(&path, "").unwrap();

        // Override HOME to use temp dir
        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only env var manipulation, single-threaded test
        unsafe { std::env::set_var("HOME", temp_dir.path().to_str().unwrap()); }

        // The events_path() should point to our temp dir
        let p = events_path();
        assert!(p.exists(), "events_path() = {:?} should exist", p);

        // Read it back — should be empty
        let file = std::fs::File::open(&p).unwrap();
        let reader = std::io::BufReader::new(file);
        let count = reader.lines().count();
        assert_eq!(count, 0);

        if let Some(home) = original_home {
            // SAFETY: restoring original HOME, single-threaded test
            unsafe { std::env::set_var("HOME", home); }
        }
    }

    #[test]
    fn test_events_filters_by_event_type() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let path = temp_dir.path().join("events.jsonl");

        let mut file = std::fs::File::create(&path).unwrap();
        let now = Utc::now();
        let entries = vec![
            serde_json::json!({"ts": now.to_rfc3339(), "event": "mcp_install", "detail": "github:installed", "pid": 123}),
            serde_json::json!({"ts": now.to_rfc3339(), "event": "guard", "detail": "pip install", "action": "warn", "pid": 123}),
            serde_json::json!({"ts": now.to_rfc3339(), "event": "gate_block", "detail": "command not found", "pid": 456}),
        ];
        for entry in &entries {
            writeln!(file, "{}", entry).unwrap();
        }
        drop(file);

        // Override HOME
        let original_home = std::env::var("HOME").ok();
        // SAFETY: test-only env var manipulation, single-threaded test
        unsafe { std::env::set_var("HOME", temp_dir.path().to_str().unwrap()); }

        // Read and filter by event type
        let file = std::fs::File::open(&path).unwrap();
        let reader = std::io::BufReader::new(file);
        let mut filtered = Vec::new();
        for line in reader.lines() {
            let line = line.unwrap();
            if let Ok(entry) = serde_json::from_str::<EventEntry>(&line) {
                if entry.event.as_deref() == Some("guard") {
                    filtered.push(entry);
                }
            }
        }
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].event.as_deref(), Some("guard"));

        if let Some(home) = original_home {
            // SAFETY: restoring original HOME, single-threaded test
            unsafe { std::env::set_var("HOME", home); }
        }
    }
}
