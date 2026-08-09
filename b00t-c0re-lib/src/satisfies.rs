//! `Satisfies<T>` — the Tax-Lawyer architecture bridge trait.
//!
//! 🤓 MCP-down/UFO-up bridge: a value satisfies (or violates) a typed
//! constraint, and every check produces an `EvidenceReport` — an
//! arc-kit-au-compatible evidence node — suitable for appending to a JSONL
//! audit trail (`.b00t/audit.jsonl`, readable via `b00t-cli audit trail
//! --path .b00t/audit.jsonl`).
//!
//! This is a minimal, safely-landable slice of GH #780. Full wiring against
//! the vendored `arc-kit-au` crate (`vendor/ledgrrr/crates/arc-kit-au`) is
//! deferred — same "phase-6" deferral already recorded for the reviewer and
//! sudo-operator governance PRDs (see `_b00t_/datums/PRD-SUDO-OPERATOR-GOVERNANCE.tomllmd`
//! and `_b00t_/datums/PRD-REVIEWER-GOVERNANCE-ENGINE.tomllmd`): `node_id` here
//! uses SHA-256 (already a workspace dependency) rather than arc-kit-au's
//! Blake3, and evidence is written as flat JSONL rather than through
//! arc-kit-au's petgraph-backed store.
//!
//! FOL: `self.satisfies(constraint)` → the proposition "self satisfies
//! constraint", evidenced by an `EvidenceReport`.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

/// A constraint-satisfaction check that produces auditable evidence.
///
/// `Self` is the value under test, `T` is the constraint type. Implementors
/// typically wrap an existing boolean/enum check (e.g. `ResourceFit::fits_on`,
/// `StagePort::compatible_with`) and lift the result into an `EvidenceReport`.
pub trait Satisfies<T> {
    fn satisfies(&self, constraint: &T) -> Result<EvidenceReport>;
}

/// Content-addressed evidence that a `Satisfies<T>` check was performed.
///
/// arc-kit-au-compatible shape: `node_id` follows arc-kit-au's
/// `{type_prefix}:{hex_hash}` `NodeId` convention (see
/// `vendor/ledgrrr/crates/arc-kit-au/src/node.rs`), computed deterministically
/// from `constraint_type` + `passed` + `detail` so identical checks produce
/// identical evidence nodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReport {
    /// Deterministic `{prefix}:{sha256_hex}` node identity.
    pub node_id: String,
    /// The Rust type name of the constraint that was checked.
    pub constraint_type: String,
    /// Whether `self` satisfied the constraint.
    pub passed: bool,
    /// RFC3339 timestamp of when the check ran.
    pub timestamp: String,
    /// Human-readable explanation.
    pub detail: String,
}

impl EvidenceReport {
    /// Build a new evidence report, computing a deterministic `node_id`.
    pub fn new(constraint_type: impl Into<String>, passed: bool, detail: impl Into<String>) -> Self {
        let constraint_type = constraint_type.into();
        let detail = detail.into();
        let node_id = Self::compute_node_id(&constraint_type, passed, &detail);
        Self {
            node_id,
            constraint_type,
            passed,
            timestamp: chrono::Utc::now().to_rfc3339(),
            detail,
        }
    }

    /// arc-kit-au-style deterministic node id: `sat:{sha256_hex}`.
    fn compute_node_id(constraint_type: &str, passed: bool, detail: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(constraint_type.as_bytes());
        hasher.update([passed as u8]);
        hasher.update(detail.as_bytes());
        format!("sat:{:x}", hasher.finalize())
    }

    /// FOL: ¬passed → violated.
    pub fn is_violated(&self) -> bool {
        !self.passed
    }

    /// Serialize as a single JSONL line (no trailing newline).
    pub fn to_jsonl_line(&self) -> Result<String> {
        Ok(serde_json::to_string(self)?)
    }

    /// Append this report as one JSONL line to `path`, creating the file
    /// (and any parent directories) if needed.
    ///
    /// Compatible with `b00t-cli audit trail --path <path>`, which reads
    /// arbitrary JSONL objects and falls back gracefully when expected keys
    /// (`stage`/`event`/`result`) are absent.
    pub fn append_to_jsonl(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", self.to_jsonl_line()?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evidence_report_node_id_deterministic() {
        let a = EvidenceReport::new("ResourceRequirements", true, "fits");
        let b = EvidenceReport::new("ResourceRequirements", true, "fits");
        assert_eq!(a.node_id, b.node_id, "same inputs => same node_id");
        assert!(a.node_id.starts_with("sat:"));
    }

    #[test]
    fn test_evidence_report_node_id_differs_on_content() {
        let a = EvidenceReport::new("ResourceRequirements", true, "fits");
        let b = EvidenceReport::new("ResourceRequirements", false, "does not fit");
        assert_ne!(a.node_id, b.node_id);
    }

    #[test]
    fn test_evidence_report_is_violated() {
        let passed = EvidenceReport::new("C", true, "ok");
        let failed = EvidenceReport::new("C", false, "nope");
        assert!(!passed.is_violated());
        assert!(failed.is_violated());
    }

    #[test]
    fn test_evidence_report_jsonl_roundtrip() {
        let report = EvidenceReport::new("StagePort", true, "compatible");
        let line = report.to_jsonl_line().unwrap();
        assert!(!line.contains('\n'));
        let parsed: EvidenceReport = serde_json::from_str(&line).unwrap();
        assert_eq!(parsed, report);
    }

    #[test]
    fn test_evidence_report_append_to_jsonl() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("audit.jsonl");

        let r1 = EvidenceReport::new("ResourceRequirements", true, "fits on host");
        let r2 = EvidenceReport::new("StagePort", false, "direction mismatch");
        r1.append_to_jsonl(&path).unwrap();
        r2.append_to_jsonl(&path).unwrap();

        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);

        let parsed1: EvidenceReport = serde_json::from_str(lines[0]).unwrap();
        let parsed2: EvidenceReport = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(parsed1, r1);
        assert_eq!(parsed2, r2);
    }
}
