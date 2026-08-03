//! # LFMF Telemetry — miss-frequency accounting for the lfmf idiom
//!
//! Meta-pattern: **never lose the payload**. Outer layers (CLI, MCP) MUST NOT
//! validate stricter than the storage layer they front; malformed input degrades
//! to salvage + a telemetry event, never bail-and-discard. Every lfmf invocation
//! appends one JSONL event here so `b00t-cli lfmf stats` can report how often the
//! idiom hits, salvages, or misses — a tool that fails silently at the moment of
//! insight loses knowledge that cost real money to produce.
//!
//! Storage: `$B00T_LFMF_TELEMETRY` override, else `~/.b00t/lfmf-telemetry.jsonl`
//! (global, matching lfmf doctrine: lessons are tool wisdom, never repo-specific).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt, fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const TELEMETRY_ENV: &str = "B00T_LFMF_TELEMETRY";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfmfAction {
    /// `b00t lfmf <tool> <lesson>` — write path
    Record,
    /// `b00t lfmf advice <tool>` — read path
    Advice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfmfOutcome {
    /// Clean hit — well-formed record or non-empty advice
    Ok,
    /// Payload recovered from malformed input (detail = salvage kind)
    Salvaged,
    /// Command failed (detail = error kind) — a write miss
    Error,
    /// Advice returned zero lessons — a read miss
    NoResults,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LfmfTelemetryEvent {
    pub ts: DateTime<Utc>,
    pub action: LfmfAction,
    pub tool: String,
    pub outcome: LfmfOutcome,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl LfmfTelemetryEvent {
    pub fn now(action: LfmfAction, tool: &str, outcome: LfmfOutcome, detail: Option<String>) -> Self {
        Self {
            ts: Utc::now(),
            action,
            tool: tool.to_string(),
            outcome,
            detail,
        }
    }
}

/// Resolve the telemetry JSONL path: env override, else ~/.b00t/lfmf-telemetry.jsonl
pub fn telemetry_path() -> PathBuf {
    if let Ok(p) = std::env::var(TELEMETRY_ENV) {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t")
        .join("lfmf-telemetry.jsonl")
}

/// Best-effort JSONL append. Telemetry MUST NEVER break the command it observes,
/// so all failures are swallowed (stderr note only).
pub fn log_event(event: &LfmfTelemetryEvent) {
    log_event_to(&telemetry_path(), event);
}

/// Append to an explicit path — the testable core of `log_event`.
pub fn log_event_to(path: &Path, event: &LfmfTelemetryEvent) {
    let write = || -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(event).map_err(std::io::Error::other)?;
        let mut f = fs::OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(f, "{}", line)
    };
    if let Err(e) = write() {
        eprintln!("⚠️ lfmf telemetry unavailable ({}): {}", path.display(), e);
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct LfmfStats {
    pub total: usize,
    pub ok: usize,
    pub salvaged: usize,
    pub error: usize,
    pub no_results: usize,
    pub by_detail: BTreeMap<String, usize>,
    pub by_tool: BTreeMap<String, usize>,
}

impl LfmfStats {
    /// Misses = command failed or advice came back empty.
    pub fn miss_rate(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.error + self.no_results) as f64 / self.total as f64
    }
}

impl fmt::Display for LfmfStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "📊 lfmf telemetry — {} events", self.total)?;
        writeln!(
            f,
            "   ok: {}  salvaged: {}  error: {}  no_results: {}",
            self.ok, self.salvaged, self.error, self.no_results
        )?;
        writeln!(f, "   miss rate: {:.1}%", self.miss_rate() * 100.0)?;
        if !self.by_detail.is_empty() {
            writeln!(f, "   by detail:")?;
            for (k, v) in &self.by_detail {
                writeln!(f, "     {}: {}", k, v)?;
            }
        }
        if !self.by_tool.is_empty() {
            writeln!(f, "   by tool:")?;
            for (k, v) in &self.by_tool {
                writeln!(f, "     {}: {}", k, v)?;
            }
        }
        Ok(())
    }
}

/// Aggregate stats from JSONL content, optionally filtered by tool.
/// Unparseable lines are skipped (forward-compatible with schema growth).
pub fn compute_stats(jsonl: &str, tool_filter: Option<&str>) -> LfmfStats {
    let mut stats = LfmfStats::default();
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<LfmfTelemetryEvent>(line) else {
            continue;
        };
        if let Some(filter) = tool_filter {
            if event.tool != filter {
                continue;
            }
        }
        stats.total += 1;
        match event.outcome {
            LfmfOutcome::Ok => stats.ok += 1,
            LfmfOutcome::Salvaged => stats.salvaged += 1,
            LfmfOutcome::Error => stats.error += 1,
            LfmfOutcome::NoResults => stats.no_results += 1,
        }
        if let Some(detail) = &event.detail {
            *stats.by_detail.entry(detail.clone()).or_insert(0) += 1;
        }
        *stats.by_tool.entry(event.tool).or_insert(0) += 1;
    }
    stats
}

/// Read + aggregate from a telemetry file. Missing file → empty stats (not an error).
pub fn read_stats(path: &Path, tool_filter: Option<&str>) -> LfmfStats {
    match fs::read_to_string(path) {
        Ok(content) => compute_stats(&content, tool_filter),
        Err(_) => LfmfStats::default(),
    }
}

/// Per-tool lesson-store health snapshot -- the cross-reference signal a gate
/// or `lfmf status <tool>` needs: how often this tool's lfmf invocations
/// failed outright vs. came back empty, and how recently a failure landed.
/// Unlike `LfmfStats` (repo-wide, optionally tool-filtered aggregate),
/// `LfmfHealth` is always scoped to exactly one tool.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LfmfHealth {
    pub tool: String,
    pub fail_count: usize,
    pub skip_count: usize,
    pub total: usize,
    pub error_rate: f64,
    pub latest_failure: Option<DateTime<Utc>>,
}

/// Aggregate telemetry for a single `tool`. `fail_count` counts
/// `LfmfOutcome::Error` (write/read attempts that errored outright);
/// `skip_count` counts `LfmfOutcome::NoResults` (advice queries that came
/// back empty -- the lesson store had nothing to cross-reference against).
/// `Salvaged`/`Ok` events count toward `total` but neither bucket.
/// Unparseable lines are skipped, matching `compute_stats`.
pub fn compute_health(jsonl: &str, tool: &str) -> LfmfHealth {
    let mut health = LfmfHealth {
        tool: tool.to_string(),
        fail_count: 0,
        skip_count: 0,
        total: 0,
        error_rate: 0.0,
        latest_failure: None,
    };
    for line in jsonl.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(event) = serde_json::from_str::<LfmfTelemetryEvent>(line) else {
            continue;
        };
        if event.tool != tool {
            continue;
        }
        health.total += 1;
        match event.outcome {
            LfmfOutcome::Error => {
                health.fail_count += 1;
                health.latest_failure = Some(match health.latest_failure {
                    Some(prev) if prev >= event.ts => prev,
                    _ => event.ts,
                });
            }
            LfmfOutcome::NoResults => health.skip_count += 1,
            LfmfOutcome::Ok | LfmfOutcome::Salvaged => {}
        }
    }
    if health.total > 0 {
        health.error_rate = health.fail_count as f64 / health.total as f64;
    }
    health
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(action: LfmfAction, tool: &str, outcome: LfmfOutcome, detail: Option<&str>) -> LfmfTelemetryEvent {
        LfmfTelemetryEvent::now(action, tool, outcome, detail.map(String::from))
    }

    #[test]
    fn append_and_aggregate_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("t.jsonl");

        log_event_to(&path, &event(LfmfAction::Record, "just", LfmfOutcome::Ok, None));
        log_event_to(&path, &event(LfmfAction::Record, "just", LfmfOutcome::Salvaged, Some("no_colon")));
        log_event_to(&path, &event(LfmfAction::Advice, "podman", LfmfOutcome::NoResults, None));
        log_event_to(&path, &event(LfmfAction::Record, "podman", LfmfOutcome::Error, Some("empty_lesson")));

        let stats = read_stats(&path, None);
        assert_eq!(stats.total, 4);
        assert_eq!(stats.ok, 1);
        assert_eq!(stats.salvaged, 1);
        assert_eq!(stats.error, 1);
        assert_eq!(stats.no_results, 1);
        assert_eq!(stats.miss_rate(), 0.5);
        assert_eq!(stats.by_detail.get("no_colon"), Some(&1));
        assert_eq!(stats.by_tool.get("just"), Some(&2));
    }

    #[test]
    fn tool_filter_and_garbage_lines() {
        let jsonl = [
            serde_json::to_string(&event(LfmfAction::Record, "git", LfmfOutcome::Ok, None)).unwrap(),
            "not json at all".to_string(),
            serde_json::to_string(&event(LfmfAction::Advice, "git", LfmfOutcome::NoResults, None)).unwrap(),
            serde_json::to_string(&event(LfmfAction::Record, "just", LfmfOutcome::Ok, None)).unwrap(),
        ]
        .join("\n");

        let stats = compute_stats(&jsonl, Some("git"));
        assert_eq!(stats.total, 2);
        assert_eq!(stats.no_results, 1);
        assert_eq!(stats.miss_rate(), 0.5);
    }

    #[test]
    fn missing_file_is_empty_stats() {
        let stats = read_stats(Path::new("/nonexistent/lfmf-telemetry.jsonl"), None);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.miss_rate(), 0.0);
    }

    #[test]
    fn compute_health_counts_failures_for_tool() {
        let jsonl = [
            serde_json::to_string(&event(LfmfAction::Record, "just", LfmfOutcome::Error, Some("timeout"))).unwrap(),
            serde_json::to_string(&event(LfmfAction::Record, "just", LfmfOutcome::Error, Some("timeout"))).unwrap(),
            serde_json::to_string(&event(LfmfAction::Record, "just", LfmfOutcome::Ok, None)).unwrap(),
        ]
        .join("\n");

        let health = compute_health(&jsonl, "just");
        assert_eq!(health.tool, "just");
        assert_eq!(health.total, 3);
        assert_eq!(health.fail_count, 2);
        assert_eq!(health.skip_count, 0);
        assert!((health.error_rate - (2.0 / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn compute_health_latest_failure_is_max_error_ts() {
        let older = Utc::now() - chrono::Duration::minutes(10);
        let newer = Utc::now() - chrono::Duration::minutes(1);
        let earlier_error = LfmfTelemetryEvent {
            ts: older,
            action: LfmfAction::Record,
            tool: "just".to_string(),
            outcome: LfmfOutcome::Error,
            detail: None,
        };
        let later_error = LfmfTelemetryEvent {
            ts: newer,
            action: LfmfAction::Record,
            tool: "just".to_string(),
            outcome: LfmfOutcome::Error,
            detail: None,
        };
        // Deliberately appended out of chronological order to prove we take
        // the max ts, not merely the last Error line seen.
        let jsonl = [
            serde_json::to_string(&later_error).unwrap(),
            serde_json::to_string(&earlier_error).unwrap(),
        ]
        .join("\n");

        let health = compute_health(&jsonl, "just");
        assert_eq!(health.fail_count, 2);
        assert_eq!(health.latest_failure, Some(newer));
    }

    #[test]
    fn compute_health_zero_events_is_all_zero() {
        let health = compute_health("", "ghost-tool");
        assert_eq!(health.tool, "ghost-tool");
        assert_eq!(health.fail_count, 0);
        assert_eq!(health.skip_count, 0);
        assert_eq!(health.total, 0);
        assert_eq!(health.error_rate, 0.0);
        assert_eq!(health.latest_failure, None);
    }
}
