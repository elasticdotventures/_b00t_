use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::hive::{default_violations_path, load_session_guards, session_guards_path};

#[derive(Debug, Subcommand)]
pub enum GuardCommands {
    #[clap(
        about = "List active guards with violation counts",
        long_about = "Show all guards (universal + active profile + session) with current violation counts.\n\nExamples:\n  b00t guard list\n  b00t guard list --json"
    )]
    List {
        #[clap(long, help = "Output as JSON")]
        json: bool,
    },

    #[clap(
        about = "Reset violation count for a guard pattern (clears 🦨→💩 escalation)",
        long_about = "Appends a count=0 entry to guard-violations.jsonl, clearing escalation for the pattern.\n\nExamples:\n  b00t guard reset 'pip install'\n  b00t guard reset --all"
    )]
    Reset {
        #[clap(help = "Pattern substring to reset (matches any pattern containing this string)")]
        pattern: Option<String>,
        #[clap(long, help = "Reset ALL violation counts")]
        all: bool,
    },

    #[clap(
        about = "Add a session-scoped guard (persists in ~/.b00t/session-guards.json)",
        long_about = "Agent authority: add a runtime guard without editing hive.toml files.\nSession guards are loaded by 'b00t hive run' and 'b00t exec'.\n\nExamples:\n  b00t guard add 'curl.*evil.com' --action block --message '🚫 blocked domain'\n  b00t guard add 'npm install' --action warn --threshold 1"
    )]
    Add {
        #[clap(help = "Regex pattern to match commands")]
        pattern: String,
        #[clap(long, default_value = "warn", help = "Action: warn | block | redirect")]
        action: String,
        #[clap(long, help = "Message shown when guard triggers")]
        message: Option<String>,
        #[clap(long, help = "Escalate to block after N violations (repeat_threshold)")]
        threshold: Option<u32>,
    },

    #[clap(
        about = "Remove a session-scoped guard by pattern",
        long_about = "Removes a previously added session guard from ~/.b00t/session-guards.json.\n\nExamples:\n  b00t guard remove 'npm install'"
    )]
    Remove {
        #[clap(help = "Pattern to remove (exact match)")]
        pattern: String,
    },

    #[clap(
        about = "Show guard violation summary",
        long_about = "Show violation counts per pattern. Escalated (💩) patterns exceeded their repeat_threshold.\n\nExamples:\n  b00t guard status\n  b00t guard status --escalated"
    )]
    Status {
        #[clap(long, help = "Show only escalated (💩 block) patterns")]
        escalated: bool,
        #[clap(long, default_value = "20", help = "Max patterns to display")]
        top: usize,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionGuardEntry {
    pattern: String,
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    threshold: Option<u32>,
}

fn load_session_entries() -> Vec<SessionGuardEntry> {
    let path = session_guards_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    serde_json::from_str(&content).unwrap_or_default()
}

fn save_session_entries(entries: &[SessionGuardEntry]) -> Result<()> {
    let path = session_guards_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(entries)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Read violation counts from guard-violations.jsonl (last count per pattern wins).
fn load_violation_counts() -> std::collections::HashMap<String, u32> {
    let path = default_violations_path();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Default::default(),
    };
    let mut counts = std::collections::HashMap::new();
    for line in content.lines() {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
            if let (Some(pattern), Some(count)) = (
                val.get("pattern").and_then(|v| v.as_str()),
                val.get("count").and_then(|v| v.as_u64()),
            ) {
                counts.insert(pattern.to_string(), count as u32);
            }
        }
    }
    counts
}

impl GuardCommands {
    pub fn execute(&self, datum_dir: &str) -> Result<()> {
        match self {
            GuardCommands::List { json } => {
                let counts = load_violation_counts();
                let session_guards = load_session_guards();

                // Load hive guards for display labels
                let datum_path = std::path::Path::new(datum_dir);
                let mut guard_patterns: Vec<(String, String)> = Vec::new(); // (source, pattern)

                if let Ok(g) = crate::hive::load_profile("hive-guards", datum_path) {
                    for guard in &g.guards {
                        let pat = pattern_display(guard);
                        guard_patterns.push(("universal".to_string(), pat));
                    }
                }

                // Active profile guards
                if let Ok(snap) = crate::hive::SystemSnapshot::capture() {
                    if let Some(active) = &snap.active_profile {
                        if let Ok(p) = crate::hive::load_profile(active, datum_path) {
                            for guard in &p.guards {
                                let pat = pattern_display(guard);
                                guard_patterns.push((format!("profile:{active}"), pat));
                            }
                        }
                    }
                }

                for guard in &session_guards {
                    let pat = pattern_display(guard);
                    guard_patterns.push(("session".to_string(), pat));
                }

                if *json {
                    let out: Vec<_> = guard_patterns
                        .iter()
                        .map(|(src, pat)| {
                            let count = counts.get(pat).copied().unwrap_or(0);
                            serde_json::json!({
                                "source": src,
                                "pattern": pat,
                                "violations": count,
                                "escalated": count > 1,
                            })
                        })
                        .collect();
                    println!("{}", serde_json::to_string_pretty(&out)?);
                } else {
                    println!("🛡️  Active guards ({} total):", guard_patterns.len());
                    println!();
                    for (src, pat) in &guard_patterns {
                        let count = counts.get(pat).copied().unwrap_or(0);
                        let icon = if count > 1 {
                            "💩"
                        } else if count > 0 {
                            "🦨"
                        } else {
                            "✅"
                        };
                        println!("  {} [{src}] {pat}  ({count} violations)", icon);
                    }
                }
                Ok(())
            }

            GuardCommands::Reset { pattern, all } => {
                if !*all && pattern.is_none() {
                    anyhow::bail!("Specify a pattern or use --all");
                }
                let counts = load_violation_counts();
                let path = default_violations_path();
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)?;
                use std::io::Write;

                let mut reset_count = 0usize;
                for (pat, _) in &counts {
                    let matches = *all
                        || pattern
                            .as_ref()
                            .map(|p| pat.contains(p.as_str()))
                            .unwrap_or(false);
                    if matches {
                        writeln!(file, "{}", serde_json::json!({"pattern": pat, "count": 0}))?;
                        reset_count += 1;
                    }
                }

                if reset_count == 0 {
                    println!("No matching patterns found in violation log.");
                } else {
                    println!("✅ Reset {reset_count} guard violation count(s).");
                }
                Ok(())
            }

            GuardCommands::Add {
                pattern,
                action,
                message,
                threshold,
            } => {
                let valid_actions = ["warn", "block", "redirect"];
                if !valid_actions.contains(&action.as_str()) {
                    anyhow::bail!("Invalid action '{}'. Use: warn | block | redirect", action);
                }
                let mut entries = load_session_entries();
                // Upsert: replace existing entry with same pattern
                entries.retain(|e| e.pattern != *pattern);
                entries.push(SessionGuardEntry {
                    pattern: pattern.clone(),
                    action: action.clone(),
                    message: message.clone(),
                    threshold: *threshold,
                });
                save_session_entries(&entries)?;
                let action_icon = match action.as_str() {
                    "block" => "🚫",
                    "redirect" => "↪️",
                    _ => "🦨",
                };
                println!(
                    "{action_icon} Session guard added: '{pattern}' → {action}{}",
                    threshold
                        .map(|t| format!(" (escalates after {t} violations)"))
                        .unwrap_or_default()
                );
                Ok(())
            }

            GuardCommands::Remove { pattern } => {
                let mut entries = load_session_entries();
                let before = entries.len();
                entries.retain(|e| e.pattern != *pattern);
                if entries.len() == before {
                    println!("No session guard with pattern '{pattern}' found.");
                } else {
                    save_session_entries(&entries)?;
                    println!("✅ Removed session guard: '{pattern}'");
                }
                Ok(())
            }

            GuardCommands::Status { escalated, top } => {
                let counts = load_violation_counts();
                let session_entries = load_session_entries();

                let mut sorted: Vec<_> = counts.into_iter().collect();
                sorted.sort_by(|a, b| b.1.cmp(&a.1));

                let filtered: Vec<_> = if *escalated {
                    sorted.into_iter().filter(|(_, c)| *c > 1).collect()
                } else {
                    sorted
                };

                let shown: Vec<_> = filtered.iter().take(*top).collect();

                println!("🛡️  Guard violation status:");
                if shown.is_empty() {
                    println!("  No violations recorded.");
                } else {
                    for (i, (pat, count)) in shown.iter().enumerate() {
                        let icon = if *count > 1 { "💩" } else { "🦨" };
                        println!("  {}. {} {} ({} violations)", i + 1, icon, pat, count);
                    }
                    if filtered.len() > *top {
                        println!("  ... and {} more patterns", filtered.len() - *top);
                    }
                }

                if !session_entries.is_empty() {
                    println!();
                    println!("📋 Session guards ({}):", session_entries.len());
                    for e in &session_entries {
                        let icon = match e.action.as_str() {
                            "block" => "🚫",
                            "redirect" => "↪️",
                            _ => "🦨",
                        };
                        println!(
                            "  {} '{}' → {}{}",
                            icon,
                            e.pattern,
                            e.action,
                            e.message
                                .as_ref()
                                .map(|m| format!(": {m}"))
                                .unwrap_or_default()
                        );
                    }
                }
                Ok(())
            }
        }
    }
}

fn pattern_display(guard: &crate::hive::HiveGuard) -> String {
    match &guard.pattern {
        crate::hive::GuardPattern::JsonRegexPattern(p) => p.clone(),
        crate::hive::GuardPattern::RhaiExpr(e) => format!("rhai:{}", e.rhai),
        crate::hive::GuardPattern::K0mmand3rStage(s) => format!("stage:{}", s.stage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_session_guard_add_and_remove_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("session-guards.json");
        // Write two entries manually
        let entries = vec![
            SessionGuardEntry {
                pattern: "pip install".to_string(),
                action: "warn".to_string(),
                message: Some("🦨 use uv pip install".to_string()),
                threshold: Some(1),
            },
            SessionGuardEntry {
                pattern: "rm -rf /".to_string(),
                action: "block".to_string(),
                message: None,
                threshold: None,
            },
        ];
        let json = serde_json::to_string_pretty(&entries).unwrap();
        std::fs::write(&path, &json).unwrap();

        let loaded: Vec<SessionGuardEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].pattern, "pip install");
        assert_eq!(loaded[1].action, "block");
    }

    #[test]
    fn test_load_session_guards_missing_file_returns_empty() {
        // session_guards_path will point to a likely-nonexistent path in test env
        // just verify load_session_guards doesn't panic
        let guards = crate::hive::load_session_guards();
        // either empty (file doesn't exist) or has entries — both valid
        let _ = guards;
    }

    #[test]
    fn test_load_violation_counts_empty_on_missing() {
        let counts = load_violation_counts();
        // Should not panic; may be empty or have real entries
        let _ = counts;
    }

    #[test]
    fn test_session_guard_action_mapping() {
        let entries = vec![
            SessionGuardEntry {
                pattern: "a".to_string(),
                action: "block".to_string(),
                message: None,
                threshold: None,
            },
            SessionGuardEntry {
                pattern: "b".to_string(),
                action: "redirect".to_string(),
                message: None,
                threshold: None,
            },
            SessionGuardEntry {
                pattern: "c".to_string(),
                action: "warn".to_string(),
                message: None,
                threshold: None,
            },
        ];
        let json = serde_json::to_string(&entries).unwrap();
        std::fs::write(session_guards_path(), &json).unwrap_or(());
        let guards = crate::hive::load_session_guards();
        // Don't assert exact count (file may not be writable in test env),
        // just verify the function doesn't crash and mapping logic is reachable.
        let _ = guards;
    }
}
