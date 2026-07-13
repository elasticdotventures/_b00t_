// 🤓 b00t pipeline logs — stage execution log store, query, and follow-mode stream.
//    Provides PipelineLogEntry / LogLevel types, a LogStore trait with an
//    in-memory VecLogStore backed by Arc<Mutex<Vec<...>>>, and a CLI handler
//    that prints a colored table. Designed as a self-contained module that
//    the PipelineCommands enum dispatches to.
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ── LogLevel ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
    Debug,
}

impl LogLevel {
    /// Return a colored ANSI label for this level.
    /// Uses the crate-wide `ansi` helpers that auto-disable when stdout is
    /// not a terminal.
    pub fn as_colored_str(&self) -> String {
        match self {
            LogLevel::Error => crate::ansi::red("ERROR"),
            LogLevel::Warn => crate::ansi::yellow("WARN"),
            LogLevel::Info => crate::ansi::cyan("INFO"),
            LogLevel::Debug => crate::ansi::dim("DEBUG"),
        }
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
            LogLevel::Debug => write!(f, "DEBUG"),
        }
    }
}

impl FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" | "err" => Ok(LogLevel::Error),
            "debug" | "dbg" => Ok(LogLevel::Debug),
            _ => Err(format!(
                "invalid log level: '{s}' (expected info, warn, error, or debug)"
            )),
        }
    }
}

// ── PipelineLogEntry ──────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PipelineLogEntry {
    pub run_id: String,
    pub stage_name: String,
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
    pub metadata: Option<HashMap<String, String>>,
}

impl PipelineLogEntry {
    /// Create a new entry with the given fields; timestamp is set to `Utc::now()`.
    pub fn new(
        run_id: impl Into<String>,
        stage_name: impl Into<String>,
        level: LogLevel,
        message: impl Into<String>,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            stage_name: stage_name.into(),
            timestamp: Utc::now(),
            level,
            message: message.into(),
            metadata: None,
        }
    }

    /// Attach optional metadata key/value pairs.
    pub fn with_metadata(mut self, metadata: HashMap<String, String>) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

// ── PipelineLogQuery ──────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct PipelineLogQuery {
    pub pipeline_name: Option<String>,
    pub run_id: Option<String>,
    pub stage_name: Option<String>,
    pub level: Option<LogLevel>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
}

// ── LogStore trait ────────────────────────────────────────────────────

/// Abstraction over log storage backends.
///
/// Implementors must be `Send + Sync` so they can be shared across threads
/// (e.g. for `--follow` mode or concurrent pipeline execution).
pub trait LogStore: Send + Sync {
    /// Persist a single log entry.
    fn store(&self, entry: PipelineLogEntry);
    /// Return entries matching the given query filters, sorted by timestamp
    /// ascending and truncated to `query.limit` (if set).
    fn query(&self, query: &PipelineLogQuery) -> Vec<PipelineLogEntry>;
}

// ── VecLogStore ───────────────────────────────────────────────────────

/// In-memory log store backed by `Arc<Mutex<Vec<PipelineLogEntry>>>`.
///
/// All methods are O(n) in the number of stored entries — suitable for
/// development, testing, and moderate workloads.
pub struct VecLogStore {
    entries: Arc<Mutex<Vec<PipelineLogEntry>>>,
}

impl VecLogStore {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl LogStore for VecLogStore {
    fn store(&self, entry: PipelineLogEntry) {
        let mut entries = self.entries.lock().expect("VecLogStore lock poisoned");
        entries.push(entry);
    }

    fn query(&self, query: &PipelineLogQuery) -> Vec<PipelineLogEntry> {
        let entries = self.entries.lock().expect("VecLogStore lock poisoned");
        let mut results: Vec<PipelineLogEntry> = entries
            .iter()
            .filter(|e| {
                if let Some(ref pipeline_name) = query.pipeline_name {
                    // Match pipeline name as a prefix of the run_id.
                    if !e.run_id.starts_with(pipeline_name.as_str()) {
                        return false;
                    }
                }
                if let Some(ref run_id) = query.run_id {
                    if e.run_id != *run_id {
                        return false;
                    }
                }
                if let Some(ref stage_name) = query.stage_name {
                    if e.stage_name != *stage_name {
                        return false;
                    }
                }
                if let Some(ref level) = query.level {
                    if e.level != *level {
                        return false;
                    }
                }
                if let Some(ref since) = query.since {
                    if e.timestamp < *since {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();
        // Sort ascending so the caller sees chronological order.
        results.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        if let Some(limit) = query.limit {
            results.truncate(limit);
        }
        results
    }
}

// ── Global store singleton ────────────────────────────────────────────

/// Lazily-initialised global log store, shared across pipeline execution
/// and the CLI `logs` command.
pub static PIPELINE_LOG_STORE: Lazy<VecLogStore> = Lazy::new(VecLogStore::new);

// ── CLI Args ──────────────────────────────────────────────────────────

#[derive(Debug, clap::Args, Clone)]
pub struct PipelineLogsArgs {
    #[clap(long, help = "Filter by pipeline name (matches run_id prefix)")]
    pub pipeline: Option<String>,

    #[clap(long, help = "Filter by stage name")]
    pub stage: Option<String>,

    #[clap(long, help = "Filter by log level (info, warn, error, debug)")]
    pub level: Option<LogLevel>,

    #[clap(long, help = "Show entries since RFC 3339 timestamp (e.g. 2026-07-12T00:00:00Z)")]
    pub since: Option<String>,

    #[clap(long, help = "Maximum number of entries to return")]
    pub limit: Option<usize>,

    #[clap(
        long,
        help = "Follow log output in real-time (tail -f style); polls every second"
    )]
    pub follow: bool,
}

// ── Handler ───────────────────────────────────────────────────────────

pub fn handle_pipeline_logs(store: &dyn LogStore, args: &PipelineLogsArgs) -> anyhow::Result<()> {
    let since = if let Some(ref since_str) = args.since {
        Some(
            DateTime::parse_from_rfc3339(since_str)
                .map_err(|e| anyhow::anyhow!("invalid --since timestamp: {e}"))?
                .with_timezone(&Utc),
        )
    } else {
        None
    };

    let query = PipelineLogQuery {
        pipeline_name: args.pipeline.clone(),
        run_id: None,
        stage_name: args.stage.clone(),
        level: args.level.clone(),
        since,
        limit: args.limit,
    };

    if args.follow {
        follow_logs(store, query)
    } else {
        let results = store.query(&query);
        if results.is_empty() {
            println!("No log entries found.");
        } else {
            print_log_table(&results);
        }
        Ok(())
    }
}

// ── Follow mode ───────────────────────────────────────────────────────

fn follow_logs(store: &dyn LogStore, mut query: PipelineLogQuery) -> anyhow::Result<()> {
    // Track how many entries we've already printed so we only show new ones.
    let mut seen: usize = 0;

    loop {
        let results = store.query(&query);
        if results.len() > seen {
            let new_batch: Vec<_> = results.into_iter().skip(seen).collect();
            if let Some(last) = new_batch.last() {
                // Advance the `since` cursor so subsequent queries don't re-scan
                // the full history unnecessarily (VecLogStore is O(n) regardless,
                // but this is correct semantics for any backend).
                query.since = Some(last.timestamp);
            }
            print_log_table(&new_batch);
            seen += new_batch.len();
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

// ── Table formatter ───────────────────────────────────────────────────

fn print_log_table(entries: &[PipelineLogEntry]) {
    if entries.is_empty() {
        return;
    }

    // Dynamic column widths.
    let ts_width: usize = 24; // "2026-07-12T00:00:00.000Z" = 24 chars
    let lvl_width: usize = 5; // "ERROR"
    let stage_width: usize = entries
        .iter()
        .map(|e| e.stage_name.len())
        .max()
        .unwrap_or(5)
        .max("STAGE".len());

    // Header row.
    println!(
        "{:<ts_width$}  {:<lvl_width$}  {:<stage_width$}  MESSAGE",
        "TIMESTAMP", "LEVEL", "STAGE",
        ts_width = ts_width,
        lvl_width = lvl_width,
        stage_width = stage_width,
    );
    println!(
        "{:-<ts_width$}  {:-<lvl_width$}  {:-<stage_width$}  {:-<30}",
        "", "", "", "",
        ts_width = ts_width,
        lvl_width = lvl_width,
        stage_width = stage_width,
    );

    // Data rows.
    for entry in entries {
        let ts = entry.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ");
        let level_str = entry.level.as_colored_str();
        println!(
            "{ts}  {level_str}  {:<stage_width$}  {}",
            entry.stage_name,
            entry.message,
            stage_width = stage_width,
        );
    }
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn make_entry(
        run_id: &str,
        stage: &str,
        level: LogLevel,
        msg: &str,
        ts: DateTime<Utc>,
    ) -> PipelineLogEntry {
        PipelineLogEntry {
            run_id: run_id.to_string(),
            stage_name: stage.to_string(),
            timestamp: ts,
            level,
            message: msg.to_string(),
            metadata: None,
        }
    }

    // ── VecLogStore ──

    #[test]
    fn test_store_and_query_all() {
        let store = VecLogStore::new();
        store.store(make_entry("pipe-1", "build", LogLevel::Info, "started", Utc::now()));
        store.store(make_entry("pipe-1", "test", LogLevel::Warn, "flake", Utc::now()));
        let results = store.query(&PipelineLogQuery::default());
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_by_run_id() {
        let store = VecLogStore::new();
        store.store(make_entry("run-a", "s1", LogLevel::Info, "a1", Utc::now()));
        store.store(make_entry("run-b", "s1", LogLevel::Info, "b1", Utc::now()));
        let q = PipelineLogQuery {
            run_id: Some("run-a".into()),
            ..Default::default()
        };
        let results = store.query(&q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].run_id, "run-a");
    }

    #[test]
    fn test_query_by_pipeline_name() {
        let store = VecLogStore::new();
        store.store(make_entry("my-pipeline::uuid1", "build", LogLevel::Info, "ok", Utc::now()));
        store.store(make_entry("other-pipe::uuid2", "build", LogLevel::Info, "ok", Utc::now()));
        let q = PipelineLogQuery {
            pipeline_name: Some("my-pipeline".into()),
            ..Default::default()
        };
        let results = store.query(&q);
        assert_eq!(results.len(), 1);
        assert!(results[0].run_id.starts_with("my-pipeline"));
    }

    #[test]
    fn test_query_by_stage() {
        let store = VecLogStore::new();
        store.store(make_entry("p1", "build", LogLevel::Info, "building", Utc::now()));
        store.store(make_entry("p1", "test", LogLevel::Info, "testing", Utc::now()));
        let q = PipelineLogQuery {
            stage_name: Some("build".into()),
            ..Default::default()
        };
        let results = store.query(&q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].stage_name, "build");
    }

    #[test]
    fn test_query_by_level() {
        let store = VecLogStore::new();
        store.store(make_entry("p1", "s1", LogLevel::Info, "info msg", Utc::now()));
        store.store(make_entry("p1", "s2", LogLevel::Error, "err msg", Utc::now()));
        let q = PipelineLogQuery {
            level: Some(LogLevel::Error),
            ..Default::default()
        };
        let results = store.query(&q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].level, LogLevel::Error);
        assert_eq!(results[0].message, "err msg");
    }

    #[test]
    fn test_query_by_since() {
        let store = VecLogStore::new();
        let early = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let later = Utc.with_ymd_and_hms(2025, 6, 1, 0, 0, 0).unwrap();
        store.store(make_entry("p1", "s1", LogLevel::Info, "early", early));
        store.store(make_entry("p1", "s2", LogLevel::Info, "later", later));
        let q = PipelineLogQuery {
            since: Some(Utc.with_ymd_and_hms(2025, 3, 1, 0, 0, 0).unwrap()),
            ..Default::default()
        };
        let results = store.query(&q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "later");
    }

    #[test]
    fn test_query_limit() {
        let store = VecLogStore::new();
        store.store(make_entry("p1", "s1", LogLevel::Info, "first", Utc::now()));
        store.store(make_entry("p1", "s2", LogLevel::Info, "second", Utc::now()));
        let q = PipelineLogQuery {
            limit: Some(1),
            ..Default::default()
        };
        let results = store.query(&q);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_multiple_filters() {
        let store = VecLogStore::new();
        store.store(make_entry("pipe-a::r1", "build", LogLevel::Info, "ok", Utc::now()));
        store.store(make_entry("pipe-a::r1", "build", LogLevel::Error, "fail", Utc::now()));
        store.store(make_entry("pipe-b::r2", "build", LogLevel::Info, "ok", Utc::now()));
        // Filter by pipeline-name AND level.
        let q = PipelineLogQuery {
            pipeline_name: Some("pipe-a".into()),
            level: Some(LogLevel::Error),
            ..Default::default()
        };
        let results = store.query(&q);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].level, LogLevel::Error);
        assert!(results[0].run_id.starts_with("pipe-a"));
    }

    #[test]
    fn test_sort_by_timestamp() {
        let store = VecLogStore::new();
        let t1 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap();
        // Insert out of order.
        store.store(make_entry("p1", "s2", LogLevel::Info, "second", t2));
        store.store(make_entry("p1", "s1", LogLevel::Info, "first", t1));
        let results = store.query(&PipelineLogQuery::default());
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].message, "first");
        assert_eq!(results[1].message, "second");
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let store = VecLogStore::new();
        let results = store.query(&PipelineLogQuery::default());
        assert!(results.is_empty());
    }

    // ── LogLevel ──

    #[test]
    fn test_log_level_from_str() {
        assert_eq!("info".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("INFO".parse::<LogLevel>().unwrap(), LogLevel::Info);
        assert_eq!("warn".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("warning".parse::<LogLevel>().unwrap(), LogLevel::Warn);
        assert_eq!("error".parse::<LogLevel>().unwrap(), LogLevel::Error);
        assert_eq!("err".parse::<LogLevel>().unwrap(), LogLevel::Error);
        assert_eq!("debug".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert_eq!("dbg".parse::<LogLevel>().unwrap(), LogLevel::Debug);
        assert!("invalid".parse::<LogLevel>().is_err());
    }

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Info.to_string(), "INFO");
        assert_eq!(LogLevel::Warn.to_string(), "WARN");
        assert_eq!(LogLevel::Error.to_string(), "ERROR");
        assert_eq!(LogLevel::Debug.to_string(), "DEBUG");
    }

    // ── PipelineLogEntry builder ──

    #[test]
    fn test_log_entry_builder() {
        let entry = PipelineLogEntry::new("run-1", "stage-1", LogLevel::Info, "hello world")
            .with_metadata({
                let mut m = HashMap::new();
                m.insert("key".into(), "val".into());
                m
            });
        assert_eq!(entry.run_id, "run-1");
        assert_eq!(entry.stage_name, "stage-1");
        assert_eq!(entry.level, LogLevel::Info);
        assert_eq!(entry.message, "hello world");
        let meta = entry.metadata.unwrap();
        assert_eq!(meta.get("key").unwrap(), "val");
    }

    #[test]
    fn test_log_entry_timestamp_is_set() {
        let entry = PipelineLogEntry::new("r", "s", LogLevel::Debug, "ts");
        // Just verify it's a reasonable recent timestamp (within 5 seconds).
        let age = Utc::now().signed_duration_since(entry.timestamp);
        assert!(age.num_seconds() < 5, "timestamp should be recent");
    }

    // ── Thread safety ──

    #[test]
    fn test_vec_log_store_is_send_sync() {
        // Compile-time assertion: VecLogStore must implement Send + Sync.
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<VecLogStore>();
        assert_sync::<VecLogStore>();
    }

    // ── Multiple stores are independent ──

    #[test]
    fn test_independent_stores() {
        let store_a = VecLogStore::new();
        let store_b = VecLogStore::new();
        store_a.store(make_entry("a", "s1", LogLevel::Info, "from a", Utc::now()));
        store_b.store(make_entry("b", "s1", LogLevel::Info, "from b", Utc::now()));
        assert_eq!(store_a.query(&PipelineLogQuery::default()).len(), 1);
        assert_eq!(store_a.query(&PipelineLogQuery::default())[0].run_id, "a");
        assert_eq!(store_b.query(&PipelineLogQuery::default()).len(), 1);
        assert_eq!(store_b.query(&PipelineLogQuery::default())[0].run_id, "b");
    }
}
