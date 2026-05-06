use anyhow::Result;
use clap::Parser;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

#[derive(Parser)]
pub enum ObservabilityCommands {
    #[clap(
        about = "Show recent events from unified events.jsonl",
        long_about = "Read ~/.b00t/events.jsonl and display recent events.\n\nExamples:\n  b00t observability events\n  b00t observability events --since 5\n  b00t observability events --event mcp_install\n  b00t observability events --event guard --failed\n  b00t observability events --follow"
    )]
    Events {
        #[clap(long, help = "Filter by event type (mcp_install, gate_block, guard)")]
        event: Option<String>,
        #[clap(long, help = "Show only failed events (detail contains 'fail')")]
        failed: bool,
        #[clap(long, help = "Show events since N minutes ago", default_value = "60")]
        since: u64,
        #[clap(long, help = "Follow new events (tail -f)")]
        follow: bool,
    },
    #[clap(
        about = "Show guard violation statistics",
        long_about = "Read ~/.b00t/guard-violations.jsonl and show top N patterns.\n\nExamples:\n  b00t observability guards\n  b00t observability guards --escalated\n  b00t observability guards --top 20"
    )]
    Guards {
        #[clap(long, help = "Show only escalated violations (action=block after warn)")]
        escalated: bool,
        #[clap(long, help = "Show top N patterns by hit count", default_value_t = 10)]
        top: usize,
    },
}

fn events_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".b00t")
        .join("events.jsonl")
}

fn guards_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".b00t")
        .join("guard-violations.jsonl")
}

fn parse_ts(ts: &str) -> Option<SystemTime> {
    // Try RFC3339 first, then simple ISO-like formats
    chrono::DateTime::parse_from_rfc3339(ts)
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.f")
            .map(|d| d.and_utc().into()))
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S")
            .map(|d| d.and_utc().into()))
        .ok()
        .map(|dt: chrono::DateTime<chrono::FixedOffset>| -> SystemTime { dt.into() })
}

impl ObservabilityCommands {
    pub fn execute(&self) -> Result<()> {
        match self {
            ObservabilityCommands::Events { event, failed, since, follow } => {
                let path = events_path();
                if !path.exists() {
                    println!("No events yet — run some commands first.");
                    return Ok(());
                }

                if *follow {
                    // tail -f mode: read existing, then follow
                    let cutoff = SystemTime::now() - Duration::from_secs(*since * 60);
                    let mut file = std::fs::File::open(&path)?;

                    // Read and display existing content within the time window
                    let lines: Vec<String> = BufReader::new(&file)
                        .lines()
                        .flatten()
                        .collect();

                    let filtered: Vec<&String> = lines.iter().filter(|l| {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(l) {
                            let ts = val.get("ts").and_then(|v| v.as_str()).unwrap_or("");
                            let ev = val.get("event").and_then(|v| v.as_str()).unwrap_or("");
                            let detail = val.get("detail").and_then(|v| v.as_str()).unwrap_or("");
                            let time_ok = parse_ts(ts).map(|t| t >= cutoff).unwrap_or(true);
                            let event_ok = event.as_ref().map(|e| ev == e).unwrap_or(true);
                            let fail_ok = !failed || detail.to_lowercase().contains("fail");
                            time_ok && event_ok && fail_ok
                        } else { false }
                    }).collect();

                    if !filtered.is_empty() {
                        println!("📊 {} recent events:", filtered.len());
                        for line in &filtered {
                            display_event(line);
                        }
                    }

                    // Seek to end-of-file, then poll for new appended lines
                    println!("👀 Following events (Ctrl+C to stop)...");
                    file.seek(SeekFrom::End(0))?;
                    let mut reader = BufReader::new(file);
                    let mut partial = String::new();
                    loop {
                        let n = reader.read_line(&mut partial)?;
                        if n == 0 {
                            std::thread::sleep(Duration::from_millis(250));
                            continue;
                        }
                        let l = partial.trim_end_matches('\n').trim_end_matches('\r').to_string();
                        partial.clear();
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&l) {
                            let ev = val.get("event").and_then(|v| v.as_str()).unwrap_or("");
                            let event_ok = event.as_ref().map(|e| ev == e).unwrap_or(true);
                            let detail = val.get("detail").and_then(|v| v.as_str()).unwrap_or("");
                            let fail_ok = !failed || detail.to_lowercase().contains("fail");
                            if event_ok && fail_ok {
                                display_event(&l);
                            }
                        }
                    }
                } else {
                    // Static view: read all lines, filter, show last 20
                    let content = std::fs::read_to_string(&path)?;
                    let all: Vec<&str> = content.lines().filter(|l| {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(l) {
                            let ts = val.get("ts").and_then(|v| v.as_str()).unwrap_or("");
                            let ev = val.get("event").and_then(|v| v.as_str()).unwrap_or("");
                            let detail = val.get("detail").and_then(|v| v.as_str()).unwrap_or("");
                            let time_ok = parse_ts(ts).map(|t| {
                                t >= (SystemTime::now() - Duration::from_secs(*since * 60))
                            }).unwrap_or(true);
                            let event_ok = event.as_ref().map(|e| ev == e).unwrap_or(true);
                            let fail_ok = !failed || detail.to_lowercase().contains("fail");
                            time_ok && event_ok && fail_ok
                        } else { false }
                    }).collect();

                    let total = all.len();
                    let shown = all.iter().rev().take(20).rev();
                    if total == 0 {
                        println!("No events match filters.");
                    } else {
                        println!("📊 {} events (showing last {}, filtered from last {} min)", total, shown.len().min(20), since);
                        for line in shown {
                            display_event(line);
                        }
                    }
                }
                Ok(())
            }
            ObservabilityCommands::Guards { escalated, top } => {
                let path = guards_path();
                if !path.exists() {
                    println!("No guard violations recorded yet.");
                    return Ok(());
                }

                let content = std::fs::read_to_string(&path)?;
                let mut pattern_counts: std::collections::HashMap<String, (usize, bool)> = std::collections::HashMap::new();
                let total_lines = content.lines().count();

                for line in content.lines() {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                        let pattern = val.get("pattern")
                            .and_then(|v| v.as_str())
                            .or_else(|| val.get("detail").and_then(|v| v.as_str()))
                            .unwrap_or("unknown")
                            .to_string();
                        let is_block = val.get("action")
                            .and_then(|v| v.as_str())
                            .map(|a| a == "block")
                            .unwrap_or(false);
                        let entry = pattern_counts.entry(pattern).or_insert((0, false));
                        entry.0 += 1;
                        if is_block { entry.1 = true; }
                    }
                }

                let mut sorted: Vec<_> = pattern_counts.into_iter().collect();
                sorted.sort_by(|a, b| b.1.0.cmp(&a.1.0));

                let filtered: Vec<_> = if *escalated {
                    sorted.into_iter().filter(|(_, (_, block))| *block).collect()
                } else {
                    sorted
                };

                let shown = filtered.iter().take(*top);

                println!("🛡️  Guard violations: {} total across {} patterns", total_lines, filtered.len());
                if *escalated {
                    println!("   (showing only escalated → block patterns)");
                }
                println!();

                for (i, (pattern, (count, block))) in shown.enumerate() {
                    let icon = if *block { "💩" } else { "🦨" };
                    println!("  {}. {} {} ({} violations)", i + 1, icon, pattern, count);
                }

                if filtered.len() > *top {
                    println!("  ... and {} more patterns", filtered.len() - *top);
                }
                Ok(())
            }
        }
    }
}

fn display_event(line: &str) {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
        let ts = val.get("ts").and_then(|v| v.as_str()).unwrap_or("?");
        let ev = val.get("event").and_then(|v| v.as_str()).unwrap_or("?");
        let detail = val.get("detail").and_then(|v| v.as_str()).unwrap_or("");
        let pid = val.get("pid").and_then(|v| v.as_i64()).unwrap_or(0);

        // Trim TS to HH:MM:SS
        let short_ts = if ts.len() > 19 {
            let mut s = ts.to_string();
            s.truncate(19);
            s.replace("T", " ")
        } else {
            ts.to_string()
        };

        let icon = match ev {
            "gate_block" => "⏳",
            "guard" => if detail.contains("block") { "💩" } else { "🦨" },
            "mcp_install" => if detail.contains("fail") { "❌" } else { "✅" },
            _ => "📌",
        };

        // Show detail smartly — truncate very long guard patterns
        let short_detail = if detail.len() > 120 {
            format!("{}...", &detail[..120])
        } else {
            detail.to_string()
        };

        println!("{} [{}] {} {} (pid={})", icon, short_ts, ev, short_detail, pid);
    } else {
        eprintln!("⚠️  Cannot parse event line: {}", line);
    }
}
