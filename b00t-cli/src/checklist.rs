//! `<name>.checklist.toml` — stateful boolean-checklist datum.
//!
//! Phase 1 of CONOPS-system-normal.md: reuses `GateSpec` verbatim for each
//! check (zero new evaluation code), implicit-AND composition only (no
//! `compose_rhai` yet), no persistence yet. Backs the `b00t is <name>` CLI.

use crate::gates::GateSpec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use ufo_types::Disposition;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChecklistCheck {
    pub id: String,
    #[serde(flatten)]
    pub gate: GateSpec,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChecklistSection {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    pub hint: Option<String>,
    #[serde(default, rename = "check")]
    pub checks: Vec<ChecklistCheck>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChecklistFile {
    pub b00t: ChecklistSection,
}

impl ChecklistFile {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("reading checklist {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("parsing checklist {}", path.display()))
    }

    pub fn evaluate(&self, base_path: &str) -> ChecklistResult {
        let outcomes: Vec<CheckOutcome> = self
            .b00t
            .checks
            .iter()
            .map(|c| CheckOutcome::from_disposition(&c.id, c.gate.eval_disposition(base_path)))
            .collect();

        let failing: Vec<String> = outcomes
            .iter()
            .filter(|o| o.status == "violated")
            .map(|o| {
                o.reason
                    .clone()
                    .map(|r| format!("{}: {}", o.id, r))
                    .unwrap_or_else(|| o.id.clone())
            })
            .collect();

        let disposition = if !failing.is_empty() {
            ChecklistDisposition::Violated { failing }
        } else {
            let undetermined: Vec<String> = outcomes
                .iter()
                .filter(|o| o.status == "unknown")
                .map(|o| o.id.clone())
                .collect();
            if !undetermined.is_empty() {
                ChecklistDisposition::Unknown { undetermined }
            } else {
                ChecklistDisposition::Satisfied
            }
        };

        ChecklistResult {
            name: self.b00t.name.clone(),
            outcomes,
            disposition,
        }
    }
}

/// Per-check evaluation outcome. Status is a plain string tag rather than
/// re-exposing `ufo_types::Disposition` directly so this stays serializable
/// without depending on Disposition's own (de)serialize impl.
#[derive(Debug, Clone, Serialize)]
pub struct CheckOutcome {
    pub id: String,
    /// "satisfied" | "violated" | "unknown"
    pub status: &'static str,
    pub reason: Option<String>,
}

impl CheckOutcome {
    fn from_disposition(id: &str, disposition: Disposition) -> Self {
        let (status, reason) = match disposition {
            Disposition::Satisfied => ("satisfied", None),
            Disposition::Violated { reason } => ("violated", Some(reason)),
            Disposition::Unknown => ("unknown", None),
        };
        CheckOutcome {
            id: id.to_string(),
            status,
            reason,
        }
    }
}

/// Aggregate disposition — deliberately 3-valued (not a bare bool). See
/// CONOPS-system-normal.md: collapsing a check that couldn't be determined
/// into "false" was exactly issue #927's bug one layer down in `gates.rs`;
/// this must not reintroduce it at the checklist aggregate.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ChecklistDisposition {
    Satisfied,
    Violated { failing: Vec<String> },
    Unknown { undetermined: Vec<String> },
}

impl ChecklistDisposition {
    /// 0 = Satisfied, 1 = Violated, 2 = Unknown — lets `if b00t is x; then`
    /// distinguish "definitely broken" from "couldn't tell".
    pub fn exit_code(&self) -> i32 {
        match self {
            ChecklistDisposition::Satisfied => 0,
            ChecklistDisposition::Violated { .. } => 1,
            ChecklistDisposition::Unknown { .. } => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChecklistResult {
    pub name: String,
    pub outcomes: Vec<CheckOutcome>,
    pub disposition: ChecklistDisposition,
}

/// List `*.checklist.toml` files present directly under `dir` (no recursion).
pub fn list_checklists(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(fname) = path.file_name().and_then(|f| f.to_str()) {
                if let Some(stripped) = fname.strip_suffix(".checklist.toml") {
                    found.push(stripped.to_string());
                }
            }
        }
    }
    found.sort();
    found
}
