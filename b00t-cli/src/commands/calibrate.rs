//! `b00t datum calibrate` — E2: empirical tier calibration via tail-map telemetry.
//!
//! Reads `# b00t:map` tail-map comments from every datum file, extracts the declared
//! `tier:` and `complexity:` fields, correlates with timing telemetry from
//! `~/.b00t/telemetry/timings.jsonl`, and emits re-calibration suggestions where
//! the declared tier doesn't match observed evidence.
//!
//! # Usage
//! ```bash
//! b00t datum calibrate                    # scan all datums, emit suggestions
//! b00t datum calibrate --format json      # machine-readable output
//! b00t datum calibrate record --cmd "b00t learn rust" --datum-key rust.skill --duration-ms 1200
//! ```
//!
//! # Tier thresholds
//! complexity 1-3 → sm0l · 4-6 → ch0nky · 7-10 → frontier
//! timing  < 2 s  → sm0l · 2-30 s → ch0nky · > 30 s → frontier

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

// ── Tail-map parsing ──────────────────────────────────────────────────────────

/// Extracted metadata from a `# b00t:map v1` tail-map comment block.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TailMapMeta {
    pub tier: Option<String>,
    pub complexity: Option<u8>,
    pub cmds: Option<String>,
    pub summary: Option<String>,
}

/// Scan the last ≤20 lines of `content` for `# b00t:map` key-value comments.
/// Lines must have the form `# key: value` (single `#`, leading whitespace ok).
pub fn parse_tail_map(content: &str) -> TailMapMeta {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(20);
    let mut meta = TailMapMeta::default();
    let mut in_map = false;

    for line in &lines[start..] {
        let t = line.trim();
        if t == "# b00t:map v1" || t.starts_with("# b00t:map") {
            in_map = true;
            continue;
        }
        if !in_map {
            continue;
        }
        if !t.starts_with('#') {
            // non-comment after map started — keep scanning (TOML blank lines may appear)
            continue;
        }
        let kv = t.trim_start_matches('#').trim();
        if let Some((k, v)) = kv.split_once(':') {
            match k.trim() {
                "tier" => meta.tier = Some(v.trim().to_string()),
                "complexity" => meta.complexity = v.trim().parse().ok(),
                "cmds" => meta.cmds = Some(v.trim().to_string()),
                "summary" => meta.summary = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }
    meta
}

/// Map a complexity score (1-10) to the expected tier label.
pub fn complexity_to_tier(c: u8) -> &'static str {
    match c {
        1..=3 => "sm0l",
        4..=6 => "ch0nky",
        _ => "frontier",
    }
}

/// Map a measured average duration (ms) to the expected tier label.
pub fn duration_to_tier(avg_ms: u64) -> &'static str {
    if avg_ms < 2_000 {
        "sm0l"
    } else if avg_ms < 30_000 {
        "ch0nky"
    } else {
        "frontier"
    }
}

// ── Telemetry ─────────────────────────────────────────────────────────────────

/// A single command-execution timing record written to
/// `~/.b00t/telemetry/timings.jsonl`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimingRecord {
    pub datum_key: String,
    pub cmd: String,
    pub duration_ms: u64,
    pub timestamp: String,
}

fn timings_log_path() -> Result<PathBuf> {
    let dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("no home dir"))?
        .join(".b00t")
        .join("telemetry");
    std::fs::create_dir_all(&dir).context("create telemetry dir")?;
    Ok(dir.join("timings.jsonl"))
}

/// Append a timing record to the telemetry log.
pub fn record_timing(datum_key: &str, cmd: &str, duration_ms: u64) -> Result<()> {
    use std::io::Write;
    let rec = TimingRecord {
        datum_key: datum_key.to_string(),
        cmd: cmd.to_string(),
        duration_ms,
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    let path = timings_log_path()?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .context("open timings log")?;
    writeln!(file, "{}", serde_json::to_string(&rec)?).context("write timing")
}

/// Read all timing records. Returns empty vec if log doesn't exist.
pub fn read_timings() -> Result<Vec<TimingRecord>> {
    let path = timings_log_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(&path).context("read timings log")?;
    let mut out = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        match serde_json::from_str::<TimingRecord>(line) {
            Ok(r) => out.push(r),
            Err(e) => eprintln!("warn: timings line {}: {e}", i + 1),
        }
    }
    Ok(out)
}

/// Compute per-datum-key average duration from the timing log.
pub fn avg_durations_by_datum(records: &[TimingRecord]) -> HashMap<String, u64> {
    let mut sums: HashMap<String, (u64, u64)> = HashMap::new();
    for r in records {
        let e = sums.entry(r.datum_key.clone()).or_default();
        e.0 += r.duration_ms;
        e.1 += 1;
    }
    sums.into_iter().map(|(k, (sum, n))| (k, sum / n)).collect()
}

// ── Calibration ───────────────────────────────────────────────────────────────

/// One tier calibration finding for a datum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationSuggestion {
    pub datum_key: String,
    pub file: String,
    pub declared_tier: Option<String>,
    pub suggested_tier: String,
    pub reason: String,
    pub complexity: Option<u8>,
    pub avg_duration_ms: Option<u64>,
}

/// Scan all datum files under `b00t_path`, parse tail-maps, correlate with
/// telemetry, and return suggestions where declared tier doesn't match evidence.
pub fn calibrate_datums(b00t_path: &str) -> Result<Vec<CalibrationSuggestion>> {
    use crate::datum_utils::get_all_datums_with_paths;

    let datums_with_paths = get_all_datums_with_paths(b00t_path, None)?;
    let timings = read_timings().unwrap_or_default();
    let avg_dur = avg_durations_by_datum(&timings);

    let mut suggestions = Vec::new();

    for (key, (_datum, file_path)) in &datums_with_paths {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let meta = parse_tail_map(&content);

        // Skip datums with no tail-map metadata at all
        if meta.tier.is_none() && meta.complexity.is_none() {
            continue;
        }

        let avg_ms = avg_dur.get(key).copied();

        // Determine suggested tier from evidence (priority: timing > complexity)
        let suggested = if let Some(ms) = avg_ms {
            duration_to_tier(ms)
        } else if let Some(c) = meta.complexity {
            complexity_to_tier(c)
        } else {
            // Only declared tier, nothing to compare against
            continue;
        };

        let declared = meta.tier.as_deref();

        // Only emit a suggestion when declared tier diverges from evidence
        if declared.map(|d| d == suggested).unwrap_or(false) {
            continue;
        }

        let reason = match (avg_ms, meta.complexity) {
            (Some(ms), _) => format!(
                "avg timing {ms}ms → {suggested}; declared: {}",
                declared.unwrap_or("(none)")
            ),
            (None, Some(c)) => format!(
                "complexity {c} → {suggested}; declared: {}",
                declared.unwrap_or("(none)")
            ),
            _ => "insufficient evidence".to_string(),
        };

        suggestions.push(CalibrationSuggestion {
            datum_key: key.clone(),
            file: file_path.clone(),
            declared_tier: meta.tier.clone(),
            suggested_tier: suggested.to_string(),
            reason,
            complexity: meta.complexity,
            avg_duration_ms: avg_ms,
        });
    }

    suggestions.sort_by(|a, b| a.datum_key.cmp(&b.datum_key));
    Ok(suggestions)
}

// ── CLI interface ──────────────────────────────────────────────────────────────

#[derive(clap::Parser, Clone, Debug)]
pub struct CalibrateArgs {
    #[clap(subcommand)]
    pub cmd: Option<CalibrateCommand>,

    #[clap(long, default_value = "toml", help = "Output format: toml | json")]
    pub format: String,
}

#[derive(clap::Subcommand, Clone, Debug)]
pub enum CalibrateCommand {
    #[clap(about = "Record a command timing into the telemetry log")]
    Record {
        #[clap(long, help = "Datum key (e.g. rust.skill)")]
        datum_key: String,
        #[clap(long, help = "Command that was executed")]
        cmd: String,
        #[clap(long, help = "Measured wall-clock duration in milliseconds")]
        duration_ms: u64,
    },
}

pub fn handle_calibrate(b00t_path: &str, args: &CalibrateArgs) -> Result<()> {
    match &args.cmd {
        Some(CalibrateCommand::Record { datum_key, cmd, duration_ms }) => {
            record_timing(datum_key, cmd, *duration_ms)?;
            println!("recorded: {datum_key} cmd={cmd:?} duration={duration_ms}ms");
        }
        None => {
            let suggestions = calibrate_datums(b00t_path)?;
            match args.format.as_str() {
                "json" => {
                    let out = serde_json::json!({
                        "calibration_count": suggestions.len(),
                        "suggestions": suggestions,
                    });
                    println!("{}", serde_json::to_string_pretty(&out)?);
                }
                _ => {
                    println!("[calibrate]");
                    println!("suggestion_count = {}", suggestions.len());
                    println!();
                    if suggestions.is_empty() {
                        println!("# All declared tiers match empirical evidence — nothing to calibrate");
                    }
                    for s in &suggestions {
                        println!("[[calibrate.suggestion]]");
                        println!("datum_key = {:?}", s.datum_key);
                        if let Some(d) = &s.declared_tier {
                            println!("declared_tier = {d:?}");
                        }
                        println!("suggested_tier = {:?}", s.suggested_tier);
                        println!("reason = {:?}", s.reason);
                        if let Some(c) = s.complexity {
                            println!("complexity = {c}");
                        }
                        if let Some(ms) = s.avg_duration_ms {
                            println!("avg_duration_ms = {ms}");
                        }
                        println!("file = {:?}", s.file);
                        println!();
                    }
                }
            }
        }
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const TAIL_MAP_FIXTURE: &str = r#"[b00t]
name = "rust"
type = "skill"
hint = "Rust programming language"

# b00t:map v1
# summary: Rust systems programming
# tags: lang, systems, wasm
# tier: sm0l
# cmds: b00t learn rust
# complexity: 2
"#;

    #[test]
    fn parse_tail_map_extracts_all_fields() {
        let meta = parse_tail_map(TAIL_MAP_FIXTURE);
        assert_eq!(meta.tier.as_deref(), Some("sm0l"));
        assert_eq!(meta.complexity, Some(2));
        assert_eq!(meta.cmds.as_deref(), Some("b00t learn rust"));
        assert_eq!(meta.summary.as_deref(), Some("Rust systems programming"));
    }

    #[test]
    fn parse_tail_map_returns_default_when_no_map() {
        let meta = parse_tail_map("[b00t]\nname = \"empty\"\ntype = \"cli\"\nhint = \"nope\"");
        assert!(meta.tier.is_none());
        assert!(meta.complexity.is_none());
    }

    #[test]
    fn complexity_to_tier_mapping() {
        assert_eq!(complexity_to_tier(1), "sm0l");
        assert_eq!(complexity_to_tier(3), "sm0l");
        assert_eq!(complexity_to_tier(4), "ch0nky");
        assert_eq!(complexity_to_tier(6), "ch0nky");
        assert_eq!(complexity_to_tier(7), "frontier");
        assert_eq!(complexity_to_tier(10), "frontier");
    }

    #[test]
    fn duration_to_tier_mapping() {
        assert_eq!(duration_to_tier(500), "sm0l");
        assert_eq!(duration_to_tier(1999), "sm0l");
        assert_eq!(duration_to_tier(2000), "ch0nky");
        assert_eq!(duration_to_tier(29_999), "ch0nky");
        assert_eq!(duration_to_tier(30_000), "frontier");
    }

    #[test]
    fn timing_record_roundtrips_json() {
        let r = TimingRecord {
            datum_key: "rust.skill".to_string(),
            cmd: "b00t learn rust".to_string(),
            duration_ms: 1234,
            timestamp: "2026-06-24T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: TimingRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(r, back);
    }

    #[test]
    fn avg_durations_by_datum_computes_correctly() {
        let records = vec![
            TimingRecord {
                datum_key: "a.skill".into(),
                cmd: "x".into(),
                duration_ms: 1000,
                timestamp: "".into(),
            },
            TimingRecord {
                datum_key: "a.skill".into(),
                cmd: "y".into(),
                duration_ms: 3000,
                timestamp: "".into(),
            },
            TimingRecord {
                datum_key: "b.skill".into(),
                cmd: "z".into(),
                duration_ms: 500,
                timestamp: "".into(),
            },
        ];
        let avgs = avg_durations_by_datum(&records);
        assert_eq!(avgs["a.skill"], 2000);
        assert_eq!(avgs["b.skill"], 500);
    }

    #[test]
    fn calibrate_datums_detects_mismatch() {
        // Datum declares tier=frontier but complexity=2 → should suggest sm0l
        let dir = TempDir::new().unwrap();
        let p = dir.path().to_str().unwrap();
        std::fs::write(
            dir.path().join("wrong-tier.skill.toml"),
            "[b00t]\nname = \"wrong-tier\"\ntype = \"skill\"\nhint = \"test\"\n\n# b00t:map v1\n# summary: test\n# tier: frontier\n# complexity: 2\n",
        ).unwrap();
        let suggestions = calibrate_datums(p).unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].suggested_tier, "sm0l");
        assert_eq!(suggestions[0].declared_tier.as_deref(), Some("frontier"));
    }

    #[test]
    fn calibrate_datums_no_suggestion_when_correct() {
        // Datum declares tier=sm0l and complexity=1 → no suggestion
        let dir = TempDir::new().unwrap();
        let p = dir.path().to_str().unwrap();
        std::fs::write(
            dir.path().join("correct-tier.skill.toml"),
            "[b00t]\nname = \"correct-tier\"\ntype = \"skill\"\nhint = \"test\"\n\n# b00t:map v1\n# summary: test\n# tier: sm0l\n# complexity: 1\n",
        ).unwrap();
        let suggestions = calibrate_datums(p).unwrap();
        assert!(suggestions.is_empty(), "no mismatch — nothing to calibrate");
    }
}
