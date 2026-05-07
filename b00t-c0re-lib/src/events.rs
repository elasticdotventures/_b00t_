//! Unified event writer for b00t telemetry.
//!
//! All telemetry sources (session_track, GuardViolationCounter, mcp_list_view)
//! write to a single `~/.b00t/events.jsonl` file via this module.
//!
//! Each line is a JSON object with `ts`, `event`, `detail`, and `pid` fields.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};

/// A single telemetry event written to events.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct B00tEvent {
    /// RFC3339 timestamp
    pub ts: String,
    /// Event type identifier (e.g. "mcp_install", "guard", "mcp_list_view")
    pub event: String,
    /// Human-readable detail or payload
    pub detail: String,
    /// Process ID that emitted the event
    pub pid: u32,
}

impl B00tEvent {
    /// Create a new event with the current timestamp and process ID.
    pub fn new(event: &str, detail: &str) -> Self {
        Self {
            ts: chrono::Utc::now().to_rfc3339(),
            event: event.to_string(),
            detail: detail.to_string(),
            pid: std::process::id(),
        }
    }

    /// Serialize to a JSON string for writing to the JSONL file.
    pub fn to_json_line(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Resolve the path to the events.jsonl file.
pub fn events_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    Path::new(&home).join(".b00t").join("events.jsonl")
}

/// Ensure the `~/.b00t/` directory exists.
fn ensure_events_dir() -> std::io::Result<()> {
    let path = events_path();
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "events.jsonl has no parent"))?;
    std::fs::create_dir_all(dir)
}

/// Write a telemetry event to `~/.b00t/events.jsonl`.
///
/// Creates the directory and file if they don't exist.
/// Errors are silently swallowed — telemetry is best-effort.
pub fn write_event(event: &str, detail: &str) {
    let _ = ensure_events_dir();
    let path = events_path();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let entry = B00tEvent::new(event, detail);
        let _ = writeln!(file, "{}", entry.to_json_line());
    }
}

/// Write a pre-constructed [`B00tEvent`] to `~/.b00t/events.jsonl`.
///
/// Useful when the caller needs to control the timestamp or pid.
pub fn write_event_obj(event: &B00tEvent) {
    let _ = ensure_events_dir();
    let path = events_path();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = writeln!(file, "{}", event.to_json_line());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, MutexGuard};

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct TempHome {
        _guard: MutexGuard<'static, ()>,
        old_home: Option<String>,
        _temp_dir: tempfile::TempDir,
    }

    impl TempHome {
        fn new() -> Self {
            let guard = HOME_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let temp_dir = tempfile::tempdir().unwrap();
            fs::create_dir_all(temp_dir.path().join(".b00t")).unwrap();
            let old_home = std::env::var("HOME").ok();
            unsafe {
                std::env::set_var("HOME", temp_dir.path().to_str().unwrap());
            }

            Self {
                _guard: guard,
                old_home,
                _temp_dir: temp_dir,
            }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            if let Some(old) = &self.old_home {
                unsafe {
                    std::env::set_var("HOME", old);
                }
            } else {
                unsafe {
                    std::env::remove_var("HOME");
                }
            }
        }
    }

    #[test]
    fn test_event_creation() {
        let event = B00tEvent::new("test_event", "test detail");
        assert_eq!(event.event, "test_event");
        assert_eq!(event.detail, "test detail");
        assert_eq!(event.pid, std::process::id());
        // ts should be a valid RFC3339 string
        assert!(!event.ts.is_empty());
        assert!(event.ts.contains('T'));
    }

    #[test]
    fn test_event_serialization() {
        let event = B00tEvent::new("mcp_install", "github:installed");
        let json = event.to_json_line();
        assert!(json.contains("\"event\":\"mcp_install\""));
        assert!(json.contains("\"detail\":\"github:installed\""));
        assert!(json.contains("\"pid\":"));
        assert!(json.contains("\"ts\":"));
    }

    #[test]
    fn test_event_roundtrip() {
        let event = B00tEvent::new("guard", "pip install");
        let json = event.to_json_line();
        let parsed: B00tEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event, "guard");
        assert_eq!(parsed.detail, "pip install");
        assert_eq!(parsed.pid, event.pid);
        assert_eq!(parsed.ts, event.ts);
    }

    #[test]
    fn test_write_event_creates_file() {
        let _temp_home = TempHome::new();
        let path = events_path();
        assert!(!path.exists());

        write_event("test", "hello");

        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        let line = content.lines().next().unwrap();
        let parsed: B00tEvent = serde_json::from_str(line).unwrap();
        assert_eq!(parsed.event, "test");
        assert_eq!(parsed.detail, "hello");
    }

    #[test]
    fn test_write_event_appends() {
        let _temp_home = TempHome::new();
        write_event("first", "event1");
        write_event("second", "event2");

        let content = fs::read_to_string(events_path()).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);

        let e1: B00tEvent = serde_json::from_str(lines[0]).unwrap();
        let e2: B00tEvent = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(e1.event, "first");
        assert_eq!(e2.event, "second");
    }

    #[test]
    fn test_write_event_obj() {
        let _temp_home = TempHome::new();
        let event = B00tEvent::new("custom", "payload");
        write_event_obj(&event);

        let content = fs::read_to_string(events_path()).unwrap();
        let parsed: B00tEvent = serde_json::from_str(content.lines().next().unwrap()).unwrap();
        assert_eq!(parsed.event, "custom");
        assert_eq!(parsed.detail, "payload");
    }

    #[test]
    fn test_events_path_resolution() {
        let _temp_home = TempHome::new();
        let path = events_path();
        assert!(path.to_string_lossy().ends_with(".b00t/events.jsonl"));
    }
}
