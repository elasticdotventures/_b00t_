// 🤓 Pipeline transition ledger — durable + live recording of every
//    `pipeline_statemachine::StateMachine` transition (GH #743 follow-up).
//
//    Types:
//      TransitionRecord    — one immutable (from_state, event, to_state, timestamp) entry
//      TransitionSink      — trait for recording a transition (file, NATS, fan-out)
//      FileTransitionLog   — append-only JSON-Lines file per run_id in ~/.b00t/transitions/
//      NatsTransitionSink  — live publish to `pipeline.{run_id}.transition`
//      MultiTransitionSink — fans a transition out to N sinks, non-fatal per-sink failure

use crate::pipeline_nats::NatsTransport;
use crate::pipeline_statemachine::{PipelineEvent, PipelineState};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

// ── TransitionRecord ────────────────────────────────────────────────────────────

/// A single, immutable state-machine transition, ready for durable/live recording.
///
/// `seq` is the transition's 1-based position within its run (mirrors
/// `StateMachine::history().len()` at the time of the call), giving consumers a
/// cheap ordering key without needing to trust wall-clock timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub run_id: String,
    pub seq: u64,
    pub from_state: PipelineState,
    pub event: PipelineEvent,
    pub to_state: PipelineState,
    pub timestamp: DateTime<Utc>,
}

// ── TransitionSink trait ────────────────────────────────────────────────────────

/// Abstract sink for recording a transition. Implementations must be
/// `Send + Sync` to allow sharing across the executor's async boundary,
/// mirroring `CheckpointStore`/`LogStore`.
pub trait TransitionSink: Send + Sync {
    fn record(&self, rec: &TransitionRecord) -> Result<()>;
}

// ── FileTransitionLog ───────────────────────────────────────────────────────────

/// Durable, append-only transition log.
///
/// Writes one JSON object per line to `{base_path}/{run_id}.jsonl`. The file is
/// never truncated or rewritten in place — each `record()` call opens in
/// append mode, writes one line, and closes, so entries already flushed to
/// disk stay immutable even if a later write in the same run fails.
pub struct FileTransitionLog {
    base_path: PathBuf,
}

impl FileTransitionLog {
    /// Create a new log rooted at the given directory. The directory is
    /// created if it does not exist.
    pub fn new(base_path: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_path)
            .with_context(|| format!("creating transitions dir: {}", base_path.display()))?;
        Ok(Self { base_path })
    }

    /// Create a log using the default path `~/.b00t/transitions/`.
    pub fn default_path() -> Result<Self> {
        let home = dirs::home_dir().context("cannot determine home directory")?;
        let base = home.join(".b00t").join("transitions");
        Self::new(base)
    }

    /// Build the file path for a given run ID.
    fn path_for(&self, run_id: &str) -> PathBuf {
        // Sanitize run_id to prevent directory traversal (mirrors FileCheckpointStore).
        let safe_name = run_id.replace('/', "_").replace('\\', "_");
        self.base_path.join(format!("{}.jsonl", safe_name))
    }

    /// Replay all transition records for a run, in the order they were written.
    /// Returns an empty vec if no log exists yet for `run_id`.
    pub fn read_all(&self, run_id: &str) -> Result<Vec<TransitionRecord>> {
        let path = self.path_for(run_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("reading transition log {}", path.display()))?;
        content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line)
                    .with_context(|| format!("parsing transition line in {}", path.display()))
            })
            .collect()
    }
}

impl TransitionSink for FileTransitionLog {
    fn record(&self, rec: &TransitionRecord) -> Result<()> {
        let path = self.path_for(&rec.run_id);
        let mut line = serde_json::to_string(rec)
            .with_context(|| format!("serializing transition for '{}'", rec.run_id))?;
        line.push('\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("opening transition log {}", path.display()))?;
        file.write_all(line.as_bytes())
            .with_context(|| format!("appending transition to {}", path.display()))?;
        Ok(())
    }
}

// ── NatsTransitionSink ──────────────────────────────────────────────────────────

/// Live-publishes each transition to NATS under `pipeline.{run_id}.transition`.
///
/// Deliberately does not reuse `NatsStageRouter`/`subject_for` — those are
/// port-shaped (stage I/O data transport), while a transition is pipeline-level
/// control-plane metadata with no `StagePort`. This uses only the underlying
/// `NatsTransport` primitive.
pub struct NatsTransitionSink {
    transport: Arc<dyn NatsTransport>,
}

impl NatsTransitionSink {
    pub fn new(transport: Arc<dyn NatsTransport>) -> Self {
        Self { transport }
    }

    fn subject_for(run_id: &str) -> String {
        format!("pipeline.{run_id}.transition")
    }
}

impl TransitionSink for NatsTransitionSink {
    fn record(&self, rec: &TransitionRecord) -> Result<()> {
        let subject = Self::subject_for(&rec.run_id);
        let payload = serde_json::to_vec(rec)
            .with_context(|| format!("serializing transition for '{}'", rec.run_id))?;
        self.transport
            .publish(&subject, &payload)
            .with_context(|| format!("publishing transition to '{subject}'"))
    }
}

// ── MultiTransitionSink ─────────────────────────────────────────────────────────

/// Fans a transition out to every sink in the list.
///
/// A single sink's failure is logged to stderr but never blocks the others or
/// fails the overall `record()` call — matches the non-fatal log/checkpoint
/// failure convention already used throughout `PipelineExecutor`.
pub struct MultiTransitionSink(pub Vec<Arc<dyn TransitionSink>>);

impl TransitionSink for MultiTransitionSink {
    fn record(&self, rec: &TransitionRecord) -> Result<()> {
        for sink in &self.0 {
            if let Err(e) = sink.record(rec) {
                eprintln!(
                    "[pipeline_transitions] sink failed to record transition for '{}': {e}",
                    rec.run_id
                );
            }
        }
        Ok(())
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline_nats::MockNatsTransport;

    fn make_record(run_id: &str, seq: u64) -> TransitionRecord {
        TransitionRecord {
            run_id: run_id.to_string(),
            seq,
            from_state: PipelineState::Idle,
            event: PipelineEvent::Validate,
            to_state: PipelineState::Validating,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn file_log_records_and_reads_back_in_order() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log = FileTransitionLog::new(dir.path().to_path_buf()).expect("new log");

        for seq in 1..=3 {
            log.record(&make_record("run-a", seq)).expect("record");
        }

        let recs = log.read_all("run-a").expect("read_all");
        assert_eq!(recs.len(), 3);
        assert_eq!(recs.iter().map(|r| r.seq).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(recs[0].run_id, "run-a");
        assert_eq!(recs[0].to_state, PipelineState::Validating);
    }

    #[test]
    fn file_log_read_all_missing_run_returns_empty() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log = FileTransitionLog::new(dir.path().to_path_buf()).expect("new log");
        let recs = log.read_all("no-such-run").expect("read_all");
        assert!(recs.is_empty());
    }

    #[test]
    fn file_log_separates_runs_into_separate_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log = FileTransitionLog::new(dir.path().to_path_buf()).expect("new log");

        log.record(&make_record("run-a", 1)).expect("record a");
        log.record(&make_record("run-b", 1)).expect("record b");

        assert_eq!(log.read_all("run-a").expect("read a").len(), 1);
        assert_eq!(log.read_all("run-b").expect("read b").len(), 1);
    }

    #[test]
    fn file_log_creates_missing_directory() {
        let dir = tempfile::tempdir().expect("temp dir");
        let nested = dir.path().join("nested").join("transitions");
        assert!(!nested.exists());
        let log = FileTransitionLog::new(nested.clone()).expect("new log creates dir");
        assert!(nested.exists());
        log.record(&make_record("run-a", 1)).expect("record");
    }

    #[test]
    fn file_log_sanitizes_run_id() {
        let dir = tempfile::tempdir().expect("temp dir");
        let log = FileTransitionLog::new(dir.path().to_path_buf()).expect("new log");
        log.record(&make_record("path/../traversal", 1))
            .expect("record should not panic or escape base_path");
        let recs = log
            .read_all("path/../traversal")
            .expect("read_all with tricky run_id");
        assert_eq!(recs.len(), 1);
    }

    #[test]
    fn nats_sink_publishes_to_run_scoped_subject() {
        let transport = Arc::new(MockNatsTransport::new());
        let sink = NatsTransitionSink::new(transport.clone());

        let mut sub = transport
            .subscribe("pipeline.run-nats.transition")
            .expect("subscribe");
        sink.record(&make_record("run-nats", 1)).expect("record");

        let msg = sub.next().expect("should receive published transition");
        let rec: TransitionRecord = serde_json::from_slice(&msg).expect("parse published record");
        assert_eq!(rec.run_id, "run-nats");
        assert_eq!(rec.seq, 1);
    }

    #[test]
    fn multi_sink_fans_out_to_all_sinks() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file_log: Arc<dyn TransitionSink> =
            Arc::new(FileTransitionLog::new(dir.path().to_path_buf()).expect("new log"));
        let transport = Arc::new(MockNatsTransport::new());
        let nats_sink: Arc<dyn TransitionSink> =
            Arc::new(NatsTransitionSink::new(transport.clone()));

        let mut sub = transport
            .subscribe("pipeline.run-multi.transition")
            .expect("subscribe");

        let multi = MultiTransitionSink(vec![file_log.clone(), nats_sink]);
        multi.record(&make_record("run-multi", 1)).expect("record");

        // File sink received it.
        let file_log_concrete = FileTransitionLog::new(dir.path().to_path_buf()).expect("reopen");
        assert_eq!(file_log_concrete.read_all("run-multi").unwrap().len(), 1);

        // NATS sink received it.
        assert!(sub.next().is_some());
    }

    #[test]
    fn multi_sink_one_failure_does_not_block_others() {
        struct FailingSink;
        impl TransitionSink for FailingSink {
            fn record(&self, _rec: &TransitionRecord) -> Result<()> {
                anyhow::bail!("intentional failure");
            }
        }

        let dir = tempfile::tempdir().expect("temp dir");
        let file_log: Arc<dyn TransitionSink> =
            Arc::new(FileTransitionLog::new(dir.path().to_path_buf()).expect("new log"));
        let failing: Arc<dyn TransitionSink> = Arc::new(FailingSink);

        let multi = MultiTransitionSink(vec![failing, file_log]);
        let result = multi.record(&make_record("run-fail", 1));

        assert!(result.is_ok(), "MultiTransitionSink.record() itself must not fail");
        let file_log_concrete = FileTransitionLog::new(dir.path().to_path_buf()).expect("reopen");
        assert_eq!(
            file_log_concrete.read_all("run-fail").unwrap().len(),
            1,
            "the working sink should still have recorded the transition"
        );
    }
}
