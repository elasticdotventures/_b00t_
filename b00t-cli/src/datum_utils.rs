//! Datum utility functions for loading and searching datums
//!
//! Provides recursive datum discovery, pattern search, constraint filtering,
//! and graph export capabilities for the b00t datum system.

use crate::{BootDatum, DatumType, UnifiedConfig};
use anyhow::Result;
use b00t_c0re_lib::lfmf::DatumLookup;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Maximum recursion depth for datum discovery
const DEFAULT_MAX_DEPTH: usize = 10;

/// Datum search result with file path metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumSearchResult {
    /// Datum key (filename without .toml)
    pub key: String,
    /// Resolved file path
    pub path: String,
    /// Datum type
    pub datum_type: Option<DatumType>,
    /// Datum name
    pub name: String,
    /// Hint/description
    pub hint: String,
    /// LFMF category if set
    pub lfmf_category: Option<String>,
    /// Match reason for search results
    pub match_reason: Option<String>,
}

/// Filter criteria for datum queries
#[derive(Debug, Clone, Default)]
pub struct DatumFilter {
    /// Only available datums (prerequisites satisfied)
    pub available_only: bool,
    /// Only datums with all prerequisites met
    pub prereqs_satisfied: bool,
    /// Required OS (e.g., "linux", "macos", "windows")
    pub require_os: Option<String>,
    /// Required commands/tools
    pub require_cmds: Vec<String>,
    /// Must have any of these env vars set
    pub needs_any_env: bool,
    /// Must have all of these env vars set
    pub needs_all_env: bool,
    /// Filter by datum type
    pub datum_type: Option<DatumType>,
    /// Custom constraint requirements (e.g., "OS:linux", "CMD:docker")
    pub require_constraints: Vec<String>,
}

/// Graph node for datum ontology export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumGraphNode {
    pub key: String,
    pub name: String,
    pub datum_type: Option<DatumType>,
    pub hint: String,
}

/// Graph edge for datum ontology export
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumGraphEdge {
    pub from: String,
    pub to: String,
    pub edge_type: String, // "depends_on", "entangled_*", etc.
}

/// Datum ontology graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumGraph {
    pub nodes: Vec<DatumGraphNode>,
    pub edges: Vec<DatumGraphEdge>,
}

/// Implementation of DatumLookup trait for b00t datums
/// Enables LFMF system to resolve datum names to categories
pub struct B00tDatumLookup {
    b00t_path: String,
}

impl B00tDatumLookup {
    pub fn new(b00t_path: String) -> Self {
        Self { b00t_path }
    }
}

impl DatumLookup for B00tDatumLookup {
    fn find_datum(&self, pattern: &str) -> Option<(String, Option<String>)> {
        if let Ok(Some(datum)) = find_datum_by_pattern(&self.b00t_path, pattern) {
            Some((datum.name, datum.lfmf_category))
        } else {
            None
        }
    }
}

/// Get all datums from _b00t_ directory (non-recursive, for backwards compatibility)
pub fn get_all_datums(b00t_path: &str) -> Result<HashMap<String, BootDatum>> {
    get_all_datums_recursive(b00t_path, 0)
}

/// Get all datums recursively with configurable depth
///
/// # Arguments
/// * `b00t_path` - Path to _b00t_ directory
/// * `max_depth` - Maximum recursion depth (0 = top-level only, None = unlimited)
///
/// # Returns
/// HashMap of datum key -> (BootDatum, file path)
pub fn get_all_datums_with_paths(
    b00t_path: &str,
    max_depth: Option<usize>,
) -> Result<HashMap<String, (BootDatum, String)>> {
    let expanded_path = shellexpand::tilde(b00t_path);
    let path = Path::new(expanded_path.as_ref());
    let mut datums = HashMap::new();

    if !path.exists() {
        return Ok(datums);
    }

    let depth = max_depth.unwrap_or(DEFAULT_MAX_DEPTH);
    scan_datums_recursive(path, &mut datums, 0, depth)?;

    Ok(datums)
}

/// Recursive scanner for datum files
fn scan_datums_recursive(
    dir: &Path,
    datums: &mut HashMap<String, (BootDatum, String)>,
    current_depth: usize,
    max_depth: usize,
) -> Result<()> {
    if current_depth > max_depth {
        return Ok(());
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return Ok(()), // Skip directories we can't read
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let entry_path = entry.path();

        if entry_path.is_dir() {
            // Recurse into subdirectories
            scan_datums_recursive(&entry_path, datums, current_depth + 1, max_depth)?;
        } else if entry_path.extension().and_then(|s| s.to_str()) == Some("toml") {
            if let Some(filename) = entry_path.file_name().and_then(|s| s.to_str()) {
                // Skip non-datum files
                if filename == "bootstrap.toml"
                    || filename == "git-cliff.toml"
                    || filename == "_b00t_.toml"
                {
                    continue;
                }

                // Try to parse as unified config
                if let Ok(content) = fs::read_to_string(&entry_path) {
                    if let Ok(config) = toml::from_str::<UnifiedConfig>(&content) {
                        let datum_key = filename.trim_end_matches(".toml").to_string();
                        let path_str = entry_path.to_string_lossy().to_string();
                        datums.insert(datum_key, (config.b00t, path_str));
                    }
                }
            }
        }
    }

    Ok(())
}

/// Get all datums recursively (returns just BootDatum for backwards compatibility)
fn get_all_datums_recursive(b00t_path: &str, max_depth: usize) -> Result<HashMap<String, BootDatum>> {
    let with_paths = get_all_datums_with_paths(b00t_path, if max_depth == 0 { None } else { Some(max_depth) })?;
    Ok(with_paths.into_iter().map(|(k, (d, _))| (k, d)).collect())
}

/// Get datum by name pattern (searches for matching datums)
pub fn find_datum_by_pattern(b00t_path: &str, pattern: &str) -> Result<Option<BootDatum>> {
    let datums = get_all_datums(b00t_path)?;

    // Exact match first (key lookup)
    if let Some(datum) = datums.get(pattern) {
        return Ok(Some(datum.clone()));
    }

    // Try name or lfmf_category match in single pass
    for (_, datum) in datums.iter() {
        if datum.name == pattern {
            return Ok(Some(datum.clone()));
        }
        if let Some(category) = &datum.lfmf_category {
            if category == pattern {
                return Ok(Some(datum.clone()));
            }
        }
    }

    Ok(None)
}

/// Get all datums that have a specific LFMF category
pub fn get_datums_by_lfmf_category(b00t_path: &str, category: &str) -> Result<Vec<BootDatum>> {
    let datums = get_all_datums(b00t_path)?;
    let matching: Vec<BootDatum> = datums
        .into_values()
        .filter(|d| {
            if let Some(cat) = &d.lfmf_category {
                cat == category
            } else {
                false
            }
        })
        .collect();

    Ok(matching)
}

/// Get learn content for a datum (either from topic reference or inline)
pub fn get_datum_learn_content(b00t_path: &str, datum: &BootDatum) -> Result<Option<String>> {
    if let Some(learn) = &datum.learn {
        if let Some(topic) = &learn.topic {
            // Load from learn topic
            let content = b00t_c0re_lib::learn::get_learn_lesson(b00t_path, topic)?;
            return Ok(Some(content));
        } else if let Some(inline) = &learn.inline {
            // Return inline content
            return Ok(Some(inline.clone()));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_datum_file(dir: &std::path::Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn test_get_all_datums() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();

        // Create test datum files
        create_test_datum_file(
            temp_dir.path(),
            "just.cli.toml",
            r#"
[b00t]
name = "just"
type = "cli"
hint = "Command runner"
lfmf_category = "just"
"#,
        );

        create_test_datum_file(
            temp_dir.path(),
            "rust.cli.toml",
            r#"
[b00t]
name = "rustc"
type = "cli"
hint = "Rust compiler"
"#,
        );

        let datums = get_all_datums(b00t_path).unwrap();
        assert_eq!(datums.len(), 2);
        assert!(datums.contains_key("just.cli"));
        assert!(datums.contains_key("rust.cli"));
    }

    #[test]
    fn test_find_datum_by_pattern() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(
            temp_dir.path(),
            "just.cli.toml",
            r#"
[b00t]
name = "just"
type = "cli"
hint = "Command runner"
lfmf_category = "just"
"#,
        );

        // Test exact match
        let datum = find_datum_by_pattern(b00t_path, "just.cli").unwrap();
        assert!(datum.is_some());
        assert_eq!(datum.as_ref().unwrap().name, "just");

        // Test name match
        let datum = find_datum_by_pattern(b00t_path, "just").unwrap();
        assert!(datum.is_some());
        assert_eq!(datum.unwrap().name, "just");

        // Test not found
        let datum = find_datum_by_pattern(b00t_path, "nonexistent").unwrap();
        assert!(datum.is_none());
    }

    #[test]
    fn test_get_datums_by_lfmf_category() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(
            temp_dir.path(),
            "just.cli.toml",
            r#"
[b00t]
name = "just"
type = "cli"
hint = "Command runner"
lfmf_category = "just"
"#,
        );

        create_test_datum_file(
            temp_dir.path(),
            "justfile.mcp.toml",
            r#"
[b00t]
name = "justfile-mcp"
type = "mcp"
hint = "Just MCP server"
lfmf_category = "just"
"#,
        );

        let datums = get_datums_by_lfmf_category(b00t_path, "just").unwrap();
        assert_eq!(datums.len(), 2);
        assert!(datums.iter().any(|d| d.name == "just"));
        assert!(datums.iter().any(|d| d.name == "justfile-mcp"));
    }

    #[test]
    fn test_get_datum_learn_content_inline() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(
            temp_dir.path(),
            "test.cli.toml",
            r#"
[b00t]
name = "test"
type = "cli"
hint = "Test tool"

[b00t.learn]
inline = "This is inline learn content"
"#,
        );

        let datum = find_datum_by_pattern(b00t_path, "test").unwrap().unwrap();
        let content = get_datum_learn_content(b00t_path, &datum).unwrap();

        assert!(content.is_some());
        assert_eq!(content.unwrap(), "This is inline learn content");
    }

    #[test]
    fn test_datum_with_usage_examples() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(
            temp_dir.path(),
            "just.cli.toml",
            r#"
[b00t]
name = "just"
type = "cli"
hint = "Command runner"

[[b00t.usage]]
description = "List recipes"
command = "just -l"

[[b00t.usage]]
description = "Run recipe"
command = "just build"
output = "Building..."
"#,
        );

        let datum = find_datum_by_pattern(b00t_path, "just").unwrap().unwrap();
        assert!(datum.usage.is_some());
        let usage = datum.usage.unwrap();
        assert_eq!(usage.len(), 2);
        assert_eq!(usage[0].description, "List recipes");
        assert_eq!(usage[0].command, "just -l");
        assert_eq!(usage[1].output, Some("Building...".to_string()));
    }
}
