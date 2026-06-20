//! Gate audit trail — JSONL append-only audit logging for Zellij gate decisions.
//!
//! Provides structured audit entries written as one-JSON-object-per-line
//! (JSONL format) to `~/.b00t/audit/zellij-gate.jsonl`, plus an iterator
//! for reading back historical entries.
//!
//! # Format
//! Each line is a complete JSON object:
//! ```json
//! {"action":"build","quadrant":"Urgent & Important","result":"Allow","agent_id":"sm0l","session_id":"abc123","timestamp":"2026-06-20T12:00:00Z"}
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// A single gate audit entry — one line in the JSONL audit log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateAudit {
    /// The action being gated (e.g., "build", "deploy", "test").
    pub action: String,
    /// The Eisenhower quadrant classification at decision time.
    pub quadrant: String,
    /// The gate result (Allow, Deny, or Hook).
    pub result: String,
    /// The agent ID that triggered the check.
    pub agent_id: String,
    /// The session ID for correlation across multiple checks.
    pub session_id: String,
    /// When the gate decision was made.
    pub timestamp: DateTime<Utc>,
}

impl GateAudit {
    /// Create a new gate audit entry.
    pub fn new(
        action: impl Into<String>,
        quadrant: impl Into<String>,
        result: impl Into<String>,
        agent_id: impl Into<String>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            action: action.into(),
            quadrant: quadrant.into(),
            result: result.into(),
            agent_id: agent_id.into(),
            session_id: session_id.into(),
            timestamp: Utc::now(),
        }
    }

    /// Append this audit entry to the default JSONL audit file.
    ///
    /// Creates parent directories automatically. Uses append mode
    /// so entries are never overwritten — the file is an append-only log.
    ///
    /// # Errors
    /// Returns `std::io::Error` if the file cannot be opened or written.
    pub fn append_to_default(&self) -> Result<(), std::io::Error> {
        let path = Self::default_path();
        self.append_to_path(&path)
    }

    /// Append this entry to a specific path.
    ///
    /// Creates parent directories automatically. Uses append mode.
    pub fn append_to_path(&self, path: &Path) -> Result<(), std::io::Error> {
        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut file = OpenOptions::new().create(true).append(true).open(path)?;

        let line = serde_json::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        writeln!(file, "{line}")?;
        Ok(())
    }

    /// Returns the default audit log path: `~/.b00t/audit/zellij-gate.jsonl`
    #[must_use]
    pub fn default_path() -> PathBuf {
        let home = dirs_fallback();
        home.join(".b00t").join("audit").join("zellij-gate.jsonl")
    }
}

/// An iterator over gate audit entries in a JSONL file.
///
/// Reads one line at a time, parsing each as a [`GateAudit`].
/// Lines that fail to parse are silently skipped (best-effort reading).
#[derive(Debug)]
pub struct AuditLog {
    reader: BufReader<File>,
}

impl AuditLog {
    /// Open an audit log file for reading.
    ///
    /// Returns `None` if the file does not exist (not an error — an
    /// empty audit trail is valid).
    pub fn open(path: &Path) -> Option<Self> {
        let file = File::open(path).ok()?;
        Some(Self {
            reader: BufReader::new(file),
        })
    }

    /// Open the default audit log path.
    #[must_use]
    pub fn open_default() -> Option<Self> {
        Self::open(&GateAudit::default_path())
    }

    /// Collect all valid entries into a `Vec`.
    ///
    /// Malformed lines are silently skipped.
    pub fn collect_all(&mut self) -> Vec<GateAudit> {
        self.collect()
    }

    /// Return the last `n` entries from the log.
    ///
    /// Reads the entire file but only retains the tail. Malformed lines
    /// are silently skipped.
    pub fn tail(&mut self, n: usize) -> Vec<GateAudit> {
        let all: Vec<GateAudit> = self.collect();
        let start = all.len().saturating_sub(n);
        all.into_iter().skip(start).collect()
    }
}

impl Iterator for AuditLog {
    type Item = GateAudit;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => return None, // EOF
                Ok(_) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if let Ok(entry) = serde_json::from_str::<GateAudit>(trimmed) {
                        return Some(entry);
                    }
                    // Malformed line — skip and try next
                }
                Err(_) => return None,
            }
        }
    }
}

/// Resolve home directory without pulling in the `dirs` crate.
///
/// Checks `$HOME` first, then falls back to `/home/{user}` from `$USER`,
/// then `/root` as a last resort.
fn dirs_fallback() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home);
    }
    if let Ok(user) = std::env::var("USER") {
        return PathBuf::from("/home").join(user);
    }
    PathBuf::from("/root")
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_audit_path(dir: &TempDir) -> PathBuf {
        dir.path().join("audit.jsonl")
    }

    #[test]
    fn test_gate_audit_creation() {
        let entry = GateAudit::new(
            "build",
            "Urgent & Important",
            "Allow",
            "agent-42",
            "session-abc",
        );
        assert_eq!(entry.action, "build");
        assert_eq!(entry.quadrant, "Urgent & Important");
        assert_eq!(entry.result, "Allow");
        assert_eq!(entry.agent_id, "agent-42");
        assert_eq!(entry.session_id, "session-abc");
        // Timestamp should be very recent
        let age = Utc::now() - entry.timestamp;
        assert!(
            age.num_seconds() < 5,
            "Timestamp should be within 5 seconds"
        );
    }

    #[test]
    fn test_append_and_read() {
        let dir = TempDir::new().expect("tempdir");
        let path = temp_audit_path(&dir);

        let entry = GateAudit::new("build", "Urgent & Important", "Allow", "a1", "s1");
        entry.append_to_path(&path).expect("append should succeed");
        entry
            .append_to_path(&path)
            .expect("second append should succeed");

        let entries: Vec<GateAudit> = AuditLog::open(&path).unwrap().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].action, "build");
        assert_eq!(entries[1].action, "build");
    }

    #[test]
    fn test_audit_log_tail() {
        let dir = TempDir::new().expect("tempdir");
        let path = temp_audit_path(&dir);

        for i in 0..10 {
            let entry = GateAudit::new(
                format!("action-{i}"),
                "Urgent & Important",
                "Allow",
                "agent",
                "session",
            );
            entry.append_to_path(&path).expect("append");
        }

        let tail = AuditLog::open(&path).unwrap().tail(3);
        assert_eq!(tail.len(), 3);
        assert_eq!(tail[0].action, "action-7");
        assert_eq!(tail[1].action, "action-8");
        assert_eq!(tail[2].action, "action-9");
    }

    #[test]
    fn test_audit_log_open_missing_file() {
        assert!(AuditLog::open(Path::new("/nonexistent/path/audit.jsonl")).is_none());
    }

    #[test]
    fn test_audit_log_skips_malformed_lines() {
        let dir = TempDir::new().expect("tempdir");
        let path = temp_audit_path(&dir);

        // Write a valid entry
        let entry = GateAudit::new("build", "Urgent & Important", "Allow", "a1", "s1");
        entry.append_to_path(&path).expect("append");

        // Append garbage lines manually
        let mut file = OpenOptions::new().append(true).open(&path).expect("open");
        writeln!(file, "not valid json").expect("write");
        writeln!(file, "").expect("write empty");
        writeln!(file, "{{\"broken\": true").expect("write broken json");

        // Append another valid entry
        entry.append_to_path(&path).expect("append");

        let entries: Vec<GateAudit> = AuditLog::open(&path).unwrap().collect();
        assert_eq!(
            entries.len(),
            2,
            "Should skip 3 malformed lines, keep 2 valid"
        );
    }

    #[test]
    fn test_gate_audit_serialization_roundtrip() {
        let entry = GateAudit::new("deploy", "Urgent & Not Important", "Hook", "a2", "s2");
        let json = serde_json::to_string(&entry).expect("serialize");
        let parsed: GateAudit = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.action, "deploy");
        assert_eq!(parsed.quadrant, "Urgent & Not Important");
        assert_eq!(parsed.result, "Hook");
    }
}
