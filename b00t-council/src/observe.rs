//! Durable, observable recording of [`Envelope`]s.
//!
//! Today none of the hive's messaging subsystems keep any durable record of
//! what was sent — Redis `PUBLISH`, NATS core, and the in-process
//! `b00t-ipc::MessageBus` are all fire-and-forget. [`JsonlSink`] closes that
//! gap, following the same local-first append-only JSONL idiom already used
//! by `b00t-lib-chat`'s `LocalLedgrrr` (`OpenOptions::create(true).append(true)`,
//! replay-on-open, one JSON object per line).

use crate::message::{Envelope, Recipient};
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Default location for the hive's message log, mirroring the
/// `~/.local/share/b00t/agents/` convention `crew_handler.rs` already uses
/// for its `AgentStore`/`_crew_meta.json`.
pub fn default_log_path() -> PathBuf {
    dirs_next_home()
        .join(".local")
        .join("share")
        .join("b00t")
        .join("messages.jsonl")
}

// Kept tiny/dependency-free rather than pulling in the `dirs` crate just for
// this: falls back to "." only if $HOME is unset, which should not happen in
// any real hive session.
fn dirs_next_home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Optional filter for [`MessageSink::replay`].
#[derive(Debug, Clone, Default)]
pub struct ReplayFilter {
    /// Only envelopes addressed to this `Recipient::Channel(..)` name — the
    /// convention used to scope a vote's messages to `Channel(proposal_id)`.
    pub channel: Option<String>,
    /// Only envelopes from this player id.
    pub from: Option<String>,
}

impl ReplayFilter {
    pub fn channel(name: impl Into<String>) -> Self {
        Self {
            channel: Some(name.into()),
            from: None,
        }
    }

    fn matches(&self, env: &Envelope<serde_json::Value>) -> bool {
        if let Some(channel) = &self.channel {
            let is_match = matches!(&env.to, Recipient::Channel(c) if c == channel);
            if !is_match {
                return false;
            }
        }
        if let Some(from) = &self.from {
            if &env.from != from {
                return false;
            }
        }
        true
    }
}

/// Where [`Envelope`]s get recorded so hive traffic is observable/replayable.
pub trait MessageSink: Send + Sync {
    fn record(&self, envelope: &Envelope<serde_json::Value>) -> Result<()>;
    fn replay(&self, filter: &ReplayFilter) -> Result<Vec<Envelope<serde_json::Value>>>;
}

/// Default sink: records nothing. Lets existing callers (e.g.
/// `b00t-ipc::MessageBus`) adopt the `MessageSink` plumbing without changing
/// behavior until a real sink is supplied.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopSink;

impl MessageSink for NoopSink {
    fn record(&self, _envelope: &Envelope<serde_json::Value>) -> Result<()> {
        Ok(())
    }

    fn replay(&self, _filter: &ReplayFilter) -> Result<Vec<Envelope<serde_json::Value>>> {
        Ok(Vec::new())
    }
}

/// Append-only JSONL sink — one [`Envelope<serde_json::Value>`] per line.
pub struct JsonlSink {
    path: PathBuf,
    lock: Mutex<()>,
}

impl JsonlSink {
    pub fn at(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            lock: Mutex::new(()),
        }
    }

    pub fn default_location() -> Self {
        Self::at(default_log_path())
    }
}

impl MessageSink for JsonlSink {
    fn record(&self, envelope: &Envelope<serde_json::Value>) -> Result<()> {
        let _guard = self.lock.lock().unwrap();
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {parent:?} for message log"))?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| format!("opening message log {:?}", self.path))?;
        let line = serde_json::to_string(envelope)?;
        writeln!(file, "{line}").with_context(|| format!("appending to {:?}", self.path))?;
        Ok(())
    }

    fn replay(&self, filter: &ReplayFilter) -> Result<Vec<Envelope<serde_json::Value>>> {
        let _guard = self.lock.lock().unwrap();
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = std::fs::File::open(&self.path)
            .with_context(|| format!("opening message log {:?}", self.path))?;
        let mut out = Vec::new();
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let env: Envelope<serde_json::Value> = serde_json::from_str(&line)
                .with_context(|| format!("parsing message log line: {line}"))?;
            if filter.matches(&env) {
                out.push(env);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Envelope;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("b00t-council-test-{}-{}.jsonl", name, std::process::id()))
    }

    #[test]
    fn appends_and_replays_filtered_by_channel() {
        let path = tmp_path("replay");
        let _ = std::fs::remove_file(&path);
        let sink = JsonlSink::at(&path);

        let a = Envelope::new("agentA", Recipient::Channel("prop-1".into()), false, serde_json::json!({"n": 1}));
        let b = Envelope::new("agentB", Recipient::Channel("prop-2".into()), false, serde_json::json!({"n": 2}));
        sink.record(&a).unwrap();
        sink.record(&b).unwrap();

        let only_prop1 = sink.replay(&ReplayFilter::channel("prop-1")).unwrap();
        assert_eq!(only_prop1.len(), 1);
        assert_eq!(only_prop1[0].from, "agentA");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn noop_sink_records_nothing_observable() {
        let sink = NoopSink;
        let env = Envelope::new("x", Recipient::Broadcast, false, serde_json::json!(null));
        sink.record(&env).unwrap();
        assert!(sink.replay(&ReplayFilter::default()).unwrap().is_empty());
    }
}
