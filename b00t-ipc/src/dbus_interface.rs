//! DBus server interface for b00t hive control
//!
//! Exposes hive CMDB operations over the system (or session) bus.
//! Bus name: `com.promptexecution.b00t1`
//! Object path: `/com/promptexecution/b00t1`
//!
//! Methods delegate to existing `hive.rs` functions — this is a thin
//! privilege-boundary shim, not a reimplementation.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Result type returned by stack activate/deactivate over DBus
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StackResult {
    pub success: bool,
    pub log: Vec<String>,
}

/// The DBus service object — holds datum directory path for hive operations
pub struct B00tService {
    pub datum_dir: PathBuf,
}

impl B00tService {
    pub fn new(datum_dir: PathBuf) -> Self {
        Self { datum_dir }
    }
}

fn validate_unit_name_strict(unit: &str) -> zbus::fdo::Result<()> {
    // Preserve existing validation logic.
    validate_unit_name(unit)?;

    // Additional hardening: reject values that look like options or contain whitespace.
    if unit.starts_with('-') {
        return Err(zbus::fdo::Error::InvalidArgs(
            "invalid unit name: must not start with '-'".to_string(),
        ));
    }

    if unit.chars().any(char::is_whitespace) {
        return Err(zbus::fdo::Error::InvalidArgs(
            "invalid unit name: must not contain whitespace".to_string(),
        ));
    }

    Ok(())
}

// 🤓 zbus 4.x #[interface] requires &self methods returning zbus::Result or fdo::Result
#[zbus::interface(name = "com.promptexecution.b00t1")]
impl B00tService {
    /// Liveness check
    async fn ping(&self) -> String {
        "pong".to_string()
    }

    /// JSON SystemSnapshot — delegates to hive::SystemSnapshot::capture()
    async fn get_system_status(&self) -> zbus::fdo::Result<String> {
        // Import inline to avoid hard dep on b00t-cli types at compile time
        // The actual capture happens in the binary that links both crates
        let snapshot = dbus_hive_bridge::capture_system_status()
            .map_err(|e| zbus::fdo::Error::Failed(format!("capture failed: {e}")))?;
        Ok(snapshot)
    }

    /// Query systemctl status for a unit
    async fn service_status(&self, unit: &str) -> zbus::fdo::Result<String> {
        validate_unit_name_strict(unit)?;
        let output = std::process::Command::new("systemctl")
            .args(["status", unit])
            .output()
            .map_err(|e| zbus::fdo::Error::Failed(format!("systemctl: {e}")))?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Start a systemd unit (system-level — the whole point of the DBus boundary)
    async fn service_start(&self, unit: &str) -> zbus::fdo::Result<bool> {
        validate_unit_name_strict(unit)?;
        let status = std::process::Command::new("systemctl")
            .args(["start", unit])
            .status()
            .map_err(|e| zbus::fdo::Error::Failed(format!("systemctl start: {e}")))?;
        Ok(status.success())
    }

    /// Stop a systemd unit
    async fn service_stop(&self, unit: &str) -> zbus::fdo::Result<bool> {
        validate_unit_name_strict(unit)?;
        let status = std::process::Command::new("systemctl")
            .args(["stop", unit])
            .status()
            .map_err(|e| zbus::fdo::Error::Failed(format!("systemctl stop: {e}")))?;
        Ok(status.success())
    }

    /// Activate a hive profile — JSON StackResult
    async fn stack_activate(&self, profile: &str, force: bool) -> zbus::fdo::Result<String> {
        validate_profile_name(profile)?;
        let result = dbus_hive_bridge::activate_profile_bridge(profile, &self.datum_dir, force)
            .map_err(|e| zbus::fdo::Error::Failed(format!("activate: {e}")))?;
        serde_json::to_string(&result).map_err(|e| zbus::fdo::Error::Failed(format!("json: {e}")))
    }

    /// Deactivate a hive profile — JSON StackResult
    async fn stack_deactivate(&self, profile: &str) -> zbus::fdo::Result<String> {
        validate_profile_name(profile)?;
        let result = dbus_hive_bridge::deactivate_profile_bridge(profile, &self.datum_dir)
            .map_err(|e| zbus::fdo::Error::Failed(format!("deactivate: {e}")))?;
        serde_json::to_string(&result).map_err(|e| zbus::fdo::Error::Failed(format!("json: {e}")))
    }
}

/// Reject unit names with path traversal or shell metacharacters
fn validate_unit_name(unit: &str) -> zbus::fdo::Result<()> {
    if unit.contains('/') || unit.contains("..") || unit.contains(';') || unit.contains('`') {
        return Err(zbus::fdo::Error::InvalidArgs(
            "invalid unit name".to_string(),
        ));
    }
    Ok(())
}

/// Reject profile names with shell metacharacters
fn validate_profile_name(profile: &str) -> zbus::fdo::Result<()> {
    if profile.contains('/')
        || profile.contains("..")
        || profile.contains(';')
        || profile.contains('`')
        || profile.contains(' ')
    {
        return Err(zbus::fdo::Error::InvalidArgs(
            "invalid profile name".to_string(),
        ));
    }
    Ok(())
}

/// Bridge trait — implemented in the binary crate that links b00t-cli + b00t-ipc.
/// Keeps b00t-ipc free of circular deps on b00t-cli's hive module.
pub mod dbus_hive_bridge {
    use super::StackResult;
    use anyhow::Result;
    use std::path::Path;
    use std::sync::OnceLock;

    type CaptureFn = Box<dyn Fn() -> Result<String> + Send + Sync>;
    type ActivateFn = Box<dyn Fn(&str, &Path, bool) -> Result<StackResult> + Send + Sync>;
    type DeactivateFn = Box<dyn Fn(&str, &Path) -> Result<StackResult> + Send + Sync>;

    static CAPTURE: OnceLock<CaptureFn> = OnceLock::new();
    static ACTIVATE: OnceLock<ActivateFn> = OnceLock::new();
    static DEACTIVATE: OnceLock<DeactivateFn> = OnceLock::new();

    /// Register bridge functions — called once at server startup from b00t-cli
    pub fn register(
        capture: impl Fn() -> Result<String> + Send + Sync + 'static,
        activate: impl Fn(&str, &Path, bool) -> Result<StackResult> + Send + Sync + 'static,
        deactivate: impl Fn(&str, &Path) -> Result<StackResult> + Send + Sync + 'static,
    ) {
        let _ = CAPTURE.set(Box::new(capture));
        let _ = ACTIVATE.set(Box::new(activate));
        let _ = DEACTIVATE.set(Box::new(deactivate));
    }

    pub fn capture_system_status() -> Result<String> {
        let f = CAPTURE
            .get()
            .ok_or_else(|| anyhow::anyhow!("bridge not registered"))?;
        f()
    }

    pub fn activate_profile_bridge(
        profile: &str,
        datum_dir: &Path,
        force: bool,
    ) -> Result<StackResult> {
        let f = ACTIVATE
            .get()
            .ok_or_else(|| anyhow::anyhow!("bridge not registered"))?;
        f(profile, datum_dir, force)
    }

    pub fn deactivate_profile_bridge(profile: &str, datum_dir: &Path) -> Result<StackResult> {
        let f = DEACTIVATE
            .get()
            .ok_or_else(|| anyhow::anyhow!("bridge not registered"))?;
        f(profile, datum_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_result_round_trip() {
        let result = StackResult {
            success: true,
            log: vec!["started foo.service".into(), "profile activated".into()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: StackResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert_eq!(parsed.log.len(), 2);
    }

    #[test]
    fn validate_unit_rejects_traversal() {
        assert!(validate_unit_name("../etc/passwd").is_err());
        assert!(validate_unit_name("foo;rm -rf /").is_err());
        assert!(validate_unit_name("b00t-hive-qwen3.service").is_ok());
    }

    #[test]
    fn validate_profile_rejects_metachar() {
        assert!(validate_profile_name("inference-qwen3").is_ok());
        assert!(validate_profile_name("download-mode").is_ok());
        assert!(validate_profile_name("foo bar").is_err());
        assert!(validate_profile_name("`whoami`").is_err());
    }
}
