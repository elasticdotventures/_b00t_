//! Explicit command observations with a deadline and a three-valued exit contract.
//!
//! No implicit shell, inherited input, or captured output. Probe authors must return
//! 1 only for a measured failed condition, and 2 when the observation is unavailable.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use wait_timeout::ChildExt;

use super::FieldOutcome;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeSpec {
    pub argv: Vec<String>,
    /// Inclusive range 1..=60000; missing means a three-second observation.
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

fn default_timeout() -> u64 {
    3000
}

impl ProbeSpec {
    pub(super) fn evaluate(&self, datum_path: &str) -> Option<FieldOutcome> {
        let unknown = |reason: &str| Some(FieldOutcome::UnknownReason(format!("probe: {reason}")));
        let Some(program) = self.argv.first().filter(|program| !program.is_empty()) else {
            return unknown("empty executable");
        };
        if !(1..=60000).contains(&self.timeout_ms) {
            return unknown("deadline must be between 1 and 60000 milliseconds");
        }
        let path = Path::new(datum_path);
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or_else(|| Path::new("."))
        };
        let mut child = match Command::new(program)
            .args(&self.argv[1..])
            .current_dir(directory)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => child,
            Err(_) => return unknown("executable or working directory unavailable"),
        };
        match child.wait_timeout(Duration::from_millis(self.timeout_ms)) {
            Ok(Some(status)) => match status.code() {
                Some(0) => None,
                Some(1) => Some(FieldOutcome::Violated(
                    "probe observed a failed condition".into(),
                )),
                _ => unknown("observation unavailable or terminated abnormally"),
            },
            result => {
                // Reap the direct child on timeout or wait failure. Probes that create children
                // must bound those children themselves; the supplied Git probe does so.
                let _ = child.kill();
                let _ = child.wait();
                unknown(if result.is_ok() {
                    "observation timed out"
                } else {
                    "could not wait for observation"
                })
            }
        }
    }
}
