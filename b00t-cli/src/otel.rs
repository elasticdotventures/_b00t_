//! Session-scoped OTEL metrics collector for b00t.
//!
//! Provides lightweight counters for guard hits, command runs, unknown datum types,
//! and Postel normalizations. Exposes Prometheus text format via `scrape()` and
//! OTEL FOCUS JSON via `focus_json()`.
//!
//! No external OTEL dependency yet — pure Rust atomics. Upgrade path:
//! add `opentelemetry-sdk` feature flag when OTLP push is needed.
//!
//! # Usage
//! ```
//! crate::otel::record(MetricEvent::GuardHit { guard: "find_guard".into(), action: "warn".into() });
//! let prometheus_text = crate::otel::scrape();
//! ```

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Metric event types emitted by b00t subsystems.
#[derive(Debug, Clone)]
pub enum MetricEvent {
    /// A guard pattern matched and fired.
    GuardHit { guard: String, action: String },
    /// A CLI command was invoked via b00t hive run.
    CommandRun {
        cmd: String,
        tier: Option<String>,
        duration_ms: Option<u64>,
    },
    /// A `b00t.type` string was not recognized as a DatumType enum variant.
    DatumTypeUnknown { type_str: String },
    /// A datum filter ran with a content-tag query.
    DatumFilterRun {
        type_tag: Option<String>,
        result_count: usize,
    },
    /// A Postel normalization occurred (non-canonical → canonical).
    PostelNormalization { input: String, canonical: String },
}

/// Per-process session-scoped metric store.
struct Collector {
    guard_hits: HashMap<(String, String), u64>,
    command_runs: HashMap<String, u64>,
    datum_type_unknowns: HashMap<String, u64>,
    postel_normalizations: HashMap<(String, String), u64>,
    session_start_secs: u64,
}

impl Collector {
    fn new() -> Self {
        Self {
            guard_hits: HashMap::new(),
            command_runs: HashMap::new(),
            datum_type_unknowns: HashMap::new(),
            postel_normalizations: HashMap::new(),
            session_start_secs: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }
    }

    fn record(&mut self, event: MetricEvent) {
        match event {
            MetricEvent::GuardHit { guard, action } => {
                *self.guard_hits.entry((guard, action)).or_insert(0) += 1;
            }
            MetricEvent::CommandRun { cmd, .. } => {
                let key = cmd.split_whitespace().take(3).collect::<Vec<_>>().join(" ");
                *self.command_runs.entry(key).or_insert(0) += 1;
            }
            MetricEvent::DatumTypeUnknown { type_str } => {
                *self.datum_type_unknowns.entry(type_str).or_insert(0) += 1;
            }
            MetricEvent::PostelNormalization { input, canonical } => {
                *self
                    .postel_normalizations
                    .entry((input, canonical))
                    .or_insert(0) += 1;
            }
            MetricEvent::DatumFilterRun { .. } => {}
        }
    }

    fn scrape(&self) -> String {
        let mut out = String::new();

        out.push_str("# HELP b00t_guard_hits_total Total guard pattern matches\n");
        out.push_str("# TYPE b00t_guard_hits_total counter\n");
        let mut hits: Vec<_> = self.guard_hits.iter().collect();
        hits.sort_by(|a, b| a.0.cmp(b.0));
        for ((guard, action), count) in &hits {
            out.push_str(&format!(
                "b00t_guard_hits_total{{guard=\"{guard}\",action=\"{action}\"}} {count}\n"
            ));
        }

        out.push_str("# HELP b00t_command_runs_total Total CLI command runs\n");
        out.push_str("# TYPE b00t_command_runs_total counter\n");
        let mut cmds: Vec<_> = self.command_runs.iter().collect();
        cmds.sort_by(|a, b| b.1.cmp(a.1));
        for (cmd, count) in &cmds {
            let cmd_escaped = cmd.replace('"', "\\\"");
            out.push_str(&format!(
                "b00t_command_runs_total{{cmd=\"{cmd_escaped}\"}} {count}\n"
            ));
        }

        out.push_str("# HELP b00t_unknown_datum_types_total Unknown b00t.type strings\n");
        out.push_str("# TYPE b00t_unknown_datum_types_total counter\n");
        let mut types: Vec<_> = self.datum_type_unknowns.iter().collect();
        types.sort_by_key(|(k, _)| k.clone());
        for (type_str, count) in &types {
            out.push_str(&format!(
                "b00t_unknown_datum_types_total{{type=\"{type_str}\"}} {count}\n"
            ));
        }

        out.push_str("# HELP b00t_postel_normalizations_total Postel/DWIW normalizations\n");
        out.push_str("# TYPE b00t_postel_normalizations_total counter\n");
        let mut norms: Vec<_> = self.postel_normalizations.iter().collect();
        norms.sort_by_key(|(k, _)| k.clone());
        for ((input, canonical), count) in &norms {
            out.push_str(&format!(
                "b00t_postel_normalizations_total{{input=\"{input}\",canonical=\"{canonical}\"}} {count}\n"
            ));
        }

        out
    }

    fn summary(&self) -> String {
        let total_guards: u64 = self.guard_hits.values().sum();
        let total_cmds: u64 = self.command_runs.values().sum();
        let total_unknown: u64 = self.datum_type_unknowns.values().sum();
        let total_postel: u64 = self.postel_normalizations.values().sum();

        let top_cmds: Vec<_> = {
            let mut v: Vec<_> = self.command_runs.iter().collect();
            v.sort_by(|a, b| b.1.cmp(a.1));
            v.into_iter().take(3).collect()
        };
        let top_str = top_cmds
            .iter()
            .map(|(c, n)| format!("{c}({n})"))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "cmds={total_cmds} guard_hits={total_guards} unknown_types={total_unknown} postel={total_postel}{}",
            if top_str.is_empty() {
                String::new()
            } else {
                format!(" | top: {top_str}")
            }
        )
    }
}

static COLLECTOR: OnceLock<Mutex<Collector>> = OnceLock::new();

fn collector() -> &'static Mutex<Collector> {
    COLLECTOR.get_or_init(|| Mutex::new(Collector::new()))
}

/// Record a metric event. Best-effort — ignores lock poison.
pub fn record(event: MetricEvent) {
    if let Ok(mut c) = collector().lock() {
        c.record(event);
    }
}

/// Emit Prometheus text format. Returns empty string if collector is poisoned.
pub fn scrape() -> String {
    collector().lock().map(|c| c.scrape()).unwrap_or_default()
}

/// One-line session metrics summary for `b00t session status`.
pub fn summary() -> String {
    collector().lock().map(|c| c.summary()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guard_hit_increments_counter() {
        // Use a fresh collector rather than the global one (tests share process)
        let mut c = Collector::new();
        c.record(MetricEvent::GuardHit {
            guard: "find_guard".into(),
            action: "warn".into(),
        });
        c.record(MetricEvent::GuardHit {
            guard: "find_guard".into(),
            action: "warn".into(),
        });
        c.record(MetricEvent::GuardHit {
            guard: "docker_guard".into(),
            action: "warn".into(),
        });
        let text = c.scrape();
        assert!(
            text.contains("find_guard\",action=\"warn\"} 2"),
            "guard hit count"
        );
        assert!(
            text.contains("docker_guard\",action=\"warn\"} 1"),
            "docker guard"
        );
    }

    #[test]
    fn test_unknown_datum_type_counter() {
        let mut c = Collector::new();
        c.record(MetricEvent::DatumTypeUnknown {
            type_str: "prd".into(),
        });
        c.record(MetricEvent::DatumTypeUnknown {
            type_str: "prd".into(),
        });
        c.record(MetricEvent::DatumTypeUnknown {
            type_str: "okr".into(),
        });
        let text = c.scrape();
        assert!(text.contains("type=\"prd\"} 2"), "prd count");
        assert!(text.contains("type=\"okr\"} 1"), "okr count");
    }

    #[test]
    fn test_postel_normalization_counter() {
        let mut c = Collector::new();
        c.record(MetricEvent::PostelNormalization {
            input: "P2".into(),
            canonical: "2".into(),
        });
        let text = c.scrape();
        assert!(text.contains("input=\"P2\",canonical=\"2\"} 1"));
    }

    #[test]
    fn test_summary_format() {
        let mut c = Collector::new();
        c.record(MetricEvent::CommandRun {
            cmd: "cargo check".into(),
            tier: None,
            duration_ms: None,
        });
        c.record(MetricEvent::CommandRun {
            cmd: "cargo check".into(),
            tier: None,
            duration_ms: None,
        });
        let s = c.summary();
        assert!(s.contains("cmds=2"), "summary has cmd count");
    }
}
