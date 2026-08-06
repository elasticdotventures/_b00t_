use anyhow::{Result, bail};

use crate::index::DatumIndexEntry;

/// `type_tags` filter: require ALL tags or ANY tag.
#[derive(Debug, Clone)]
pub enum EdlTagFilter {
    All(Vec<String>),
    Any(Vec<String>),
}

impl EdlTagFilter {
    pub fn matches(&self, tags: &[String]) -> bool {
        match self {
            Self::All(required) => required.iter().all(|r| tags.iter().any(|t| t == r)),
            Self::Any(wanted) => wanted.iter().any(|w| tags.iter().any(|t| t == w)),
        }
    }
}

/// Executive Discovery Language query — emitted by ch0nky agents in OODA Orient phase.
///
/// Matches entries in a [`DatumIndex`] in < 100ms (in-memory scan).
///
/// Optional `z3_constraint` is a raw SMT-LIB2 s-expression string; validated
/// via `validate_z3()` (requires `z3-subprocess` feature + `z3` in PATH).
#[derive(Debug, Clone)]
pub struct EdlQuery {
    pub type_tags: Option<EdlTagFilter>,
    pub datum_type: Option<String>,
    pub tier: Option<String>,
    pub complexity_max: Option<u8>,
    /// Raw SMT-LIB2 constraint string (validated via z3 subprocess).
    pub z3_constraint: Option<String>,
}

impl EdlQuery {
    /// Test whether an index entry satisfies all query constraints.
    pub fn matches(&self, entry: &DatumIndexEntry) -> bool {
        if let Some(filter) = &self.type_tags {
            if !filter.matches(&entry.type_tags) {
                return false;
            }
        }
        if let Some(dt) = &self.datum_type {
            if entry.datum_type.as_deref() != Some(dt.as_str()) {
                return false;
            }
        }
        if let Some(tier) = &self.tier {
            if entry.tier.as_deref() != Some(tier.as_str()) {
                return false;
            }
        }
        if let Some(max) = self.complexity_max {
            if entry.complexity.map_or(true, |c| c > max) {
                return false;
            }
        }
        true
    }

    /// Validate the `z3_constraint` via `z3` subprocess.
    ///
    /// Returns `Ok(())` if satisfiable; `Err` if unsat or z3 unavailable.
    #[cfg(feature = "z3-subprocess")]
    pub fn validate_z3(&self) -> Result<()> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let Some(constraint) = &self.z3_constraint else {
            return Ok(());
        };

        let smt = format!(
            "(declare-const complexity Int)\n\
             (declare-const tier String)\n\
             (declare-const status String)\n\
             (assert (and (>= complexity 1) (<= complexity 10)))\n\
             (assert {constraint})\n\
             (check-sat)\n"
        );

        let mut child = Command::new("z3")
            .arg("-in")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("z3 not found in PATH: {e}"))?;

        child.stdin.as_mut().unwrap().write_all(smt.as_bytes())?;
        let output = child.wait_with_output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        if stdout.trim() == "unsat" {
            bail!("EDL z3 constraint is unsatisfiable: {constraint}");
        }
        if stdout.trim().starts_with("error") {
            bail!("z3 parse error: {}", stdout.trim());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::DatumIndexEntry;

    fn entry(tags: &[&str], tier: &str, complexity: u8) -> DatumIndexEntry {
        DatumIndexEntry {
            key: "test".into(),
            path: "test.tomllmd".into(),
            datum_type: Some("prd".into()),
            tier: Some(tier.into()),
            complexity: Some(complexity),
            type_tags: tags.iter().map(|s| s.to_string()).collect(),
            summary: None,
        }
    }

    #[test]
    fn tag_filter_all() {
        let f = EdlTagFilter::All(vec!["prd".into(), "ooda".into()]);
        assert!(f.matches(&["prd".into(), "ooda".into(), "extra".into()]));
        assert!(!f.matches(&["prd".into()])); // missing ooda
    }

    #[test]
    fn tag_filter_any() {
        let f = EdlTagFilter::Any(vec!["ooda".into(), "agent".into()]);
        assert!(f.matches(&["ooda".into()]));
        assert!(f.matches(&["agent".into(), "cli".into()]));
        assert!(!f.matches(&["prd".into()]));
    }

    #[test]
    fn query_matches_tier_and_complexity() {
        let e = entry(&["prd", "ooda"], "frontier", 6);
        let q = EdlQuery {
            type_tags: Some(EdlTagFilter::All(vec!["ooda".into()])),
            datum_type: None,
            tier: Some("frontier".into()),
            complexity_max: Some(7),
            z3_constraint: None,
        };
        assert!(q.matches(&e));

        let q2 = EdlQuery {
            complexity_max: Some(5),
            ..q.clone()
        };
        assert!(!q2.matches(&e)); // complexity 6 > max 5
    }

    #[test]
    fn query_no_constraints_matches_all() {
        let e = entry(&["prd"], "sm0l", 1);
        let q = EdlQuery {
            type_tags: None,
            datum_type: None,
            tier: None,
            complexity_max: None,
            z3_constraint: None,
        };
        assert!(q.matches(&e));
    }
}

/// Validate the `z3_constraint` SMT-LIB2 string for basic syntax without running z3.
/// Checks for balanced parentheses and non-empty expression. Returns `None` if no constraint.
pub fn check_z3_syntax(constraint: &str) -> Result<()> {
    let trimmed = constraint.trim();
    if trimmed.is_empty() {
        bail!("z3 constraint is empty");
    }
    let mut depth: i32 = 0;
    for ch in trimmed.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    bail!("z3 constraint has unbalanced parentheses: {constraint}");
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        bail!("z3 constraint has unclosed parentheses (depth={depth}): {constraint}");
    }
    Ok(())
}

impl EdlQuery {
    /// Validate z3_constraint syntax (no subprocess; pure Rust).
    /// Returns `Ok(())` if constraint is None or syntax is balanced.
    pub fn validate_z3_syntax(&self) -> Result<()> {
        match &self.z3_constraint {
            None => Ok(()),
            Some(c) => check_z3_syntax(c),
        }
    }
}
