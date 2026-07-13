use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::UnifiedConfig;

/// A single artifact reference within a polyseme datum.
/// Each ref resolves one meaning of the polysemous name to a concrete datum.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct PolysemeRef {
    /// Short disambiguating name (e.g. "bubblewrap-sandbox", "bubblewrap-android")
    pub name: String,
    /// Canonical upstream identifier (e.g. "github:containers/bubblewrap")
    pub canonical: String,
    /// Concrete datum name this ref resolves to (e.g. "bubblewrap-sandbox.cli")
    pub datum: String,
    /// Human-readable description of this specific meaning
    pub description: String,
}

/// Polyseme config — blackhole/box of artifact references.
/// A polyseme datum contains NO install/run logic itself; it is a
/// knowledge-graph branch point that maps a name to its possible meanings.
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, Default)]
#[serde(default)]
pub struct PolysemeConfig {
    /// Named artifact references — each one is a distinct resolution.
    pub refs: Option<Vec<PolysemeRef>>,
    /// Source URLs that were assimilated to build this polyseme.
    pub sources: Option<Vec<String>>,
}

/// Load a polyseme datum and return its refs.
pub fn load_polyseme_refs(name: &str, path: &str) -> Result<Vec<PolysemeRef>> {
    let expanded = crate::get_expanded_path(path)?;
    let suffixes = [".polyseme.toml", ".polyseme.tomllmd", ".polyseme.tomllm"];
    let mut found = None;
    for suffix in &suffixes {
        let p = expanded.join(format!("{name}{suffix}"));
        if p.exists() {
            found = Some(p);
            break;
        }
    }
    let file_path = found
        .ok_or_else(|| anyhow::anyhow!("polyseme datum '{name}' not found"))?;
    let content = std::fs::read_to_string(&file_path)
        .context(format!("read {}", file_path.display()))?;
    let config: UnifiedConfig =
        toml::from_str(&content).context(format!("parse {}", file_path.display()))?;
    Ok(config
        .b00t
        .polyseme
        .and_then(|p| p.refs)
        .unwrap_or_default())
}
