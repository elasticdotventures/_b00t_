//! TomllmDelta: RFC 6902-style patch protocol for .tomllm datums.
//!
//! Delta files express typed operations on TOML key paths.
//! Applied on top of a base datum to produce a merged result.
//!
//! PRD-ARCH-002: Context Delta Protocol
//! Target: `b00t whoami --role=X` loads base (cached) + 100-token delta
//! instead of 4K full reload (~75% token reduction).
//!
//! # Delta file format (.tomllm.delta)
//!
//! ```toml
//! [delta]
//! base = "_b00t_/++abstract.agent.tomllm"
//!
//! [[delta.ops]]
//! op = "replace"
//! path = "b00t.agent.role"
//! value = "domain-architect"
//!
//! [[delta.ops]]
//! op = "append"
//! path = "b00t.agent.skills"
//! value = "okr-routing"
//!
//! [[delta.ops]]
//! op = "set"
//! path = "b00t.agent.tier_override"
//! value = "frontier"
//! ```

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

/// A single patch operation on a TOML key path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaOp {
    /// Operation: "replace" | "append" | "remove" | "set"
    pub op: String,
    /// Dot-delimited key path, e.g. "b00t.agent.role"
    pub path: String,
    /// New value (not needed for "remove")
    pub value: Option<toml::Value>,
}

/// Delta document parsed from a .tomllm.delta file.
#[derive(Debug, Clone, Deserialize)]
pub struct TomllmDeltaDoc {
    pub delta: DeltaSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeltaSection {
    pub base: String,
    pub ops: Vec<DeltaOp>,
}

/// Apply a sequence of delta ops to a base TOML document string.
///
/// Returns the merged TOML as a string. Comments in the base are NOT preserved
/// (toml::Value round-trip strips them) — this is intentional: the merged
/// result is for token injection, not human editing.
pub fn apply_delta(base_toml: &str, delta: &TomllmDeltaDoc) -> Result<String> {
    let mut doc: toml::Value = toml::from_str(base_toml).context("failed to parse base TOML")?;

    for op in &delta.delta.ops {
        apply_op(&mut doc, op).with_context(|| format!("op={} path={}", op.op, op.path))?;
    }

    toml::to_string_pretty(&doc).context("failed to serialize merged TOML")
}

fn apply_op(doc: &mut toml::Value, op: &DeltaOp) -> Result<()> {
    let parts: Vec<&str> = op.path.split('.').collect();
    match op.op.as_str() {
        "replace" => {
            let val = op.value.as_ref().context("replace op requires value")?;
            set_at_path(doc, &parts, val.clone(), false)?;
        }
        "set" => {
            let val = op.value.as_ref().context("set op requires value")?;
            set_at_path(doc, &parts, val.clone(), true)?;
        }
        "append" => {
            let val = op.value.as_ref().context("append op requires value")?;
            append_at_path(doc, &parts, val.clone())?;
        }
        "remove" => {
            remove_at_path(doc, &parts)?;
        }
        other => bail!("unknown delta op '{other}'; use replace|set|append|remove"),
    }
    Ok(())
}

/// Navigate to parent of `parts` last element, set or replace value.
/// `insert_only`: if true, skip if key already exists (set semantics).
fn set_at_path(
    doc: &mut toml::Value,
    parts: &[&str],
    value: toml::Value,
    insert_only: bool,
) -> Result<()> {
    let (key, ancestors) = parts.split_last().context("path must not be empty")?;
    let parent = navigate_mut(doc, ancestors, true)?;
    match parent {
        toml::Value::Table(t) => {
            if insert_only && t.contains_key(*key) {
                return Ok(());
            }
            t.insert(key.to_string(), value);
            Ok(())
        }
        other => bail!("expected table at parent path, got {:?}", other.type_str()),
    }
}

/// Append value to array at path. Creates array if key missing.
fn append_at_path(doc: &mut toml::Value, parts: &[&str], value: toml::Value) -> Result<()> {
    let (key, ancestors) = parts.split_last().context("path must not be empty")?;
    let parent = navigate_mut(doc, ancestors, true)?;
    match parent {
        toml::Value::Table(t) => {
            let entry = t
                .entry(key.to_string())
                .or_insert(toml::Value::Array(vec![]));
            match entry {
                toml::Value::Array(arr) => {
                    arr.push(value);
                    Ok(())
                }
                other => bail!("expected array at '{key}', got {:?}", other.type_str()),
            }
        }
        other => bail!("expected table at parent path, got {:?}", other.type_str()),
    }
}

/// Remove key at path. Ok if key is already absent.
fn remove_at_path(doc: &mut toml::Value, parts: &[&str]) -> Result<()> {
    let (key, ancestors) = parts.split_last().context("path must not be empty")?;
    let parent = navigate_mut(doc, ancestors, false)?;
    if let toml::Value::Table(t) = parent {
        t.remove(*key);
    }
    Ok(())
}

/// Navigate to the node at `path` within `doc`.
/// `create_tables`: create intermediate tables if missing.
fn navigate_mut<'a>(
    doc: &'a mut toml::Value,
    path: &[&str],
    create_tables: bool,
) -> Result<&'a mut toml::Value> {
    let mut current = doc;
    for segment in path {
        match current {
            toml::Value::Table(t) => {
                if create_tables {
                    current = t
                        .entry(segment.to_string())
                        .or_insert(toml::Value::Table(toml::map::Map::new()));
                } else {
                    current = t
                        .get_mut(*segment)
                        .context(format!("key '{segment}' not found"))?;
                }
            }
            other => bail!("expected table at '{segment}', got {:?}", other.type_str()),
        }
    }
    Ok(current)
}

/// Parse a .tomllm.delta file and apply it to a base TOML string.
pub fn patch_from_str(base_toml: &str, delta_toml: &str) -> Result<String> {
    let delta: TomllmDeltaDoc = toml::from_str(delta_toml).context("failed to parse delta file")?;
    apply_delta(base_toml, &delta)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: &str = r#"
[b00t]
name = "abstract-agent"
type = "agent"
hint = "base agent"

[b00t.agent]
role = "generalist"
skills = ["bash", "git"]
tier = "sm0l"
"#;

    #[test]
    fn test_replace_scalar() {
        let delta = r#"
[delta]
base = "base.tomllm"

[[delta.ops]]
op = "replace"
path = "b00t.agent.role"
value = "domain-architect"
"#;
        let result = patch_from_str(BASE, delta).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(
            v["b00t"]["agent"]["role"].as_str(),
            Some("domain-architect")
        );
    }

    #[test]
    fn test_append_to_array() {
        let delta = r#"
[delta]
base = "base.tomllm"

[[delta.ops]]
op = "append"
path = "b00t.agent.skills"
value = "okr-routing"
"#;
        let result = patch_from_str(BASE, delta).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        let skills = v["b00t"]["agent"]["skills"].as_array().unwrap();
        assert!(skills.iter().any(|s| s.as_str() == Some("okr-routing")));
        assert_eq!(skills.len(), 3);
    }

    #[test]
    fn test_set_inserts_missing_key() {
        let delta = r#"
[delta]
base = "base.tomllm"

[[delta.ops]]
op = "set"
path = "b00t.agent.tier_override"
value = "frontier"
"#;
        let result = patch_from_str(BASE, delta).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(
            v["b00t"]["agent"]["tier_override"].as_str(),
            Some("frontier")
        );
    }

    #[test]
    fn test_set_skips_existing_key() {
        let delta = r#"
[delta]
base = "base.tomllm"

[[delta.ops]]
op = "set"
path = "b00t.agent.role"
value = "should-not-overwrite"
"#;
        let result = patch_from_str(BASE, delta).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        // "role" already exists → set is no-op
        assert_eq!(v["b00t"]["agent"]["role"].as_str(), Some("generalist"));
    }

    #[test]
    fn test_remove_key() {
        let delta = r#"
[delta]
base = "base.tomllm"

[[delta.ops]]
op = "remove"
path = "b00t.agent.tier"
"#;
        let result = patch_from_str(BASE, delta).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        assert!(v["b00t"]["agent"].get("tier").is_none());
    }

    #[test]
    fn test_unknown_op_errors() {
        let delta = r#"
[delta]
base = "base.tomllm"

[[delta.ops]]
op = "xyzzy"
path = "b00t.name"
value = "x"
"#;
        assert!(patch_from_str(BASE, delta).is_err());
    }

    #[test]
    fn test_multi_op_delta() {
        let delta = r#"
[delta]
base = "base.tomllm"

[[delta.ops]]
op = "replace"
path = "b00t.agent.role"
value = "domain-architect"

[[delta.ops]]
op = "append"
path = "b00t.agent.skills"
value = "okr-routing"

[[delta.ops]]
op = "set"
path = "b00t.agent.tier_override"
value = "frontier"
"#;
        let result = patch_from_str(BASE, delta).unwrap();
        let v: toml::Value = toml::from_str(&result).unwrap();
        assert_eq!(
            v["b00t"]["agent"]["role"].as_str(),
            Some("domain-architect")
        );
        assert_eq!(
            v["b00t"]["agent"]["tier_override"].as_str(),
            Some("frontier")
        );
        let skills = v["b00t"]["agent"]["skills"].as_array().unwrap();
        assert_eq!(skills.len(), 3);
    }
}
