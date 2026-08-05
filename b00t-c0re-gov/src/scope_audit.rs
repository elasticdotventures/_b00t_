//! Append-only JSONL audit logger for scope-chain reads (#900).
//!
//! #900's ask, verbatim: `boundaries_crossed[from,to,direction]` instead of
//! a boolean. A boolean answers "did this cross a boundary at all"; the
//! array answers exactly which scopes were checked-and-missed on the way
//! to wherever the value actually resolved, in order — the difference
//! matters for anyone auditing why a read saw what it saw (e.g. "was this
//! repo-scope override intentional, or did repo-scope silently fall
//! through to global because nobody wrote it there").

use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::ScopeId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Which way the data moved across a scope boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditDirection {
    Read,
    Write,
}

/// One scope boundary a resolution walk crossed: checked `from`, didn't
/// find the key, moved on to `to`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryCrossing {
    pub from: ScopeId,
    pub to: ScopeId,
    pub direction: AuditDirection,
}

/// One audited scope-chain access: the key involved, every boundary the
/// resolution walk crossed (possibly empty — the most-specific scope had
/// it, zero boundaries crossed, still worth recording), and where it
/// actually resolved (`None` if not found anywhere in the chain).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub key: String,
    pub boundaries_crossed: Vec<BoundaryCrossing>,
    pub resolved_at: Option<ScopeId>,
}

/// Appends `AuditEvent`s as one JSON object per line to a file, one logger
/// per scope root (per #893's "Append-only JSONL audit logger per scope
/// root").
pub struct AuditLogger {
    path: PathBuf,
}

impl AuditLogger {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append one event. Never truncates or rewrites existing lines —
    /// opens in append mode every call, so concurrent writers each get
    /// their own atomic `write` of a single line (no read-modify-write
    /// race on the file as a whole).
    pub fn append(&self, event: &AuditEvent) -> ScopeResult<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(event)?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Read back every event logged so far, in append order. Mainly for
    /// tests and audit tooling, not the hot path.
    pub fn read_all(&self) -> ScopeResult<Vec<AuditEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = std::fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: AuditEvent = serde_json::from_str(&line)
                .map_err(|e| ScopeError::WriteRejected(format!("corrupt audit line: {e}")))?;
            events.push(event);
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_event() -> AuditEvent {
        AuditEvent {
            timestamp: Utc::now(),
            key: "greeting".to_string(),
            boundaries_crossed: vec![BoundaryCrossing {
                from: ScopeId::Repo("myrepo".into()),
                to: ScopeId::Node("myhost".into()),
                direction: AuditDirection::Read,
            }],
            resolved_at: Some(ScopeId::Node("myhost".into())),
        }
    }

    #[test]
    fn append_then_read_all_round_trips() {
        let dir = tempdir().unwrap();
        let logger = AuditLogger::open(dir.path().join("audit.jsonl"));

        logger.append(&sample_event()).unwrap();
        let events = logger.read_all().unwrap();

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].key, "greeting");
        assert_eq!(events[0].boundaries_crossed.len(), 1);
        assert_eq!(
            events[0].resolved_at,
            Some(ScopeId::Node("myhost".into()))
        );
    }

    #[test]
    fn append_is_additive_not_overwriting() {
        let dir = tempdir().unwrap();
        let logger = AuditLogger::open(dir.path().join("audit.jsonl"));

        for i in 0..5 {
            let mut event = sample_event();
            event.key = format!("key-{i}");
            logger.append(&event).unwrap();
        }

        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].key, "key-0");
        assert_eq!(events[4].key, "key-4");
    }

    #[test]
    fn empty_or_missing_file_reads_as_no_events() {
        let dir = tempdir().unwrap();
        let logger = AuditLogger::open(dir.path().join("never-written.jsonl"));
        assert_eq!(logger.read_all().unwrap(), Vec::new());
    }

    #[test]
    fn zero_boundary_crossings_still_recorded() {
        // The most-specific scope had it: no boundaries crossed, but the
        // event itself is still worth logging -- #900's whole point is
        // that a boolean would collapse this into "false" (indistinguishable
        // from "never checked"), losing exactly this case.
        let dir = tempdir().unwrap();
        let logger = AuditLogger::open(dir.path().join("audit.jsonl"));
        let event = AuditEvent {
            timestamp: Utc::now(),
            key: "immediate-hit".to_string(),
            boundaries_crossed: Vec::new(),
            resolved_at: Some(ScopeId::Repo("myrepo".into())),
        };
        logger.append(&event).unwrap();

        let events = logger.read_all().unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].boundaries_crossed.is_empty());
        assert_eq!(events[0].resolved_at, Some(ScopeId::Repo("myrepo".into())));
    }

    #[test]
    fn creates_parent_directory_if_missing() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a/b/c/audit.jsonl");
        let logger = AuditLogger::open(&nested);
        logger.append(&sample_event()).unwrap();
        assert!(nested.exists());
    }
}
