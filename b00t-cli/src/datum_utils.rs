//! Datum utility functions for loading and searching datums
//!
//! Provides recursive datum discovery, pattern search, constraint filtering,
//! and graph export capabilities for the b00t datum system.

pub use crate::VisualizationSpec;
use crate::{BootDatum, DatumType, UnifiedConfig};
use anyhow::Result;
use b00t_c0re_lib::lfmf::DatumLookup;
use bstr::ByteSlice;
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
    /// Filter by datum type(s); empty = all types
    pub datum_types: Vec<DatumType>,
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

/// Merge `b00t.*` Git attributes into a parsed datum without mutating the datum file.
///
/// Example `_b00t_/.gitattributes` entry:
/// `plantuml.mcp.toml b00t.status=sunset b00t.enabled=false b00t.status_msg=java-too-heavy`
pub fn apply_git_attributes_to_config(config: &mut UnifiedConfig, datum_path: &Path) {
    let attrs = match git_b00t_attributes(datum_path) {
        Ok(attrs) => attrs,
        Err(_) => return,
    };
    if attrs.is_empty() {
        return;
    }

    if let Some(value) = attrs.get("status") {
        config.b00t.status = Some(value.clone());
    }
    if let Some(value) = attrs.get("enabled") {
        config.b00t.enabled = parse_bool_attr(value);
    }
    if let Some(value) = attrs.get("status_msg") {
        config.b00t.status_msg = Some(value.clone());
    }
    if let Some(value) = attrs.get("replacement") {
        config.b00t.replacement = Some(value.clone());
    }
    config.b00t.git_attributes.extend(attrs);
}

fn parse_bool_attr(value: &str) -> Option<bool> {
    match value {
        "true" | "1" | "yes" | "on" | "set" => Some(true),
        "false" | "0" | "no" | "off" | "unset" => Some(false),
        _ => None,
    }
}

fn git_b00t_attributes(datum_path: &Path) -> Result<HashMap<String, String>> {
    let Some(repo_root) = find_git_worktree_root(datum_path) else {
        return Ok(HashMap::new());
    };
    let relative_path = datum_path
        .strip_prefix(&repo_root)
        .unwrap_or(datum_path)
        .to_string_lossy()
        .replace('\\', "/");
    let mut search = gix_attributes::Search::default();
    let mut collection = gix_attributes::search::MetadataCollection::default();
    let mut has_attributes = false;
    for attr_path in gitattributes_chain(&repo_root, datum_path) {
        let attr_bytes = fs::read(&attr_path)?;
        search.add_patterns_buffer(
            &attr_bytes,
            attr_path,
            Some(&repo_root),
            &mut collection,
            true,
        );
        has_attributes = true;
    }
    if !has_attributes {
        return Ok(HashMap::new());
    }

    let mut outcome = gix_attributes::search::Outcome::default();
    outcome.initialize(&collection);
    if !search.pattern_matching_relative_path(
        relative_path.as_bytes().as_bstr(),
        gix_attributes::glob::pattern::Case::Sensitive,
        Some(false),
        &mut outcome,
    ) {
        return Ok(HashMap::new());
    }

    let mut attrs = HashMap::new();
    for attr_match in outcome.iter() {
        let name = attr_match.assignment.name.as_str();
        if let Some(key) = name.strip_prefix("b00t.") {
            let value = match attr_match.assignment.state {
                gix_attributes::StateRef::Set => "set".to_string(),
                gix_attributes::StateRef::Unset => "unset".to_string(),
                gix_attributes::StateRef::Unspecified => "unspecified".to_string(),
                gix_attributes::StateRef::Value(value) => value.as_bstr().to_str_lossy().into_owned(),
            };
            attrs.insert(key.to_string(), value);
        }
    }
    Ok(attrs)
}

fn gitattributes_chain(repo_root: &Path, datum_path: &Path) -> Vec<std::path::PathBuf> {
    let start = if datum_path.is_dir() {
        datum_path
    } else {
        datum_path.parent().unwrap_or(repo_root)
    };
    let mut dirs: Vec<_> = start
        .ancestors()
        .take_while(|ancestor| *ancestor != repo_root)
        .collect();
    dirs.push(repo_root);
    dirs.reverse();

    dirs.into_iter()
        .map(|dir| dir.join(".gitattributes"))
        .filter(|path| path.exists())
        .collect()
}

fn find_git_worktree_root(path: &Path) -> Option<std::path::PathBuf> {
    let start = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or_else(|| Path::new("."))
    };
    for ancestor in start.ancestors() {
        if ancestor.join(".git").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    None
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
        } else if matches!(
            entry_path.extension().and_then(|s| s.to_str()),
            Some("toml") | Some("tomllm") | Some("tomllmd") // 🤓 .tomllmd currently downgrades to the generic .tomllm parser path
        ) {
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
                    if let Ok(mut config) = toml::from_str::<UnifiedConfig>(&content) {
                        apply_git_attributes_to_config(&mut config, &entry_path);
                        // Strip outer extension (.tomllmd / .tomllm / .toml) for datum key.
                        // 🤓 precedence: .tomllmd > .tomllm > .toml.
                        let ext = if filename.ends_with(".tomllmd") {
                            ".tomllmd"
                        } else if filename.ends_with(".tomllm") {
                            ".tomllm"
                        } else {
                            ".toml"
                        };
                        let datum_key = filename.trim_end_matches(ext).to_string();
                        let path_str = entry_path.to_string_lossy().to_string();
                        let new_rank = match ext {
                            ".tomllmd" => 3,
                            ".tomllm" => 2,
                            _ => 1,
                        };
                        let current_rank = datums
                            .get(&datum_key)
                            .map(|(_, existing_path)| {
                                if existing_path.ends_with(".tomllmd") {
                                    3
                                } else if existing_path.ends_with(".tomllm") {
                                    2
                                } else {
                                    1
                                }
                            })
                            .unwrap_or(0);
                        if new_rank >= current_rank {
                            datums.insert(datum_key, (config.b00t, path_str));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Get all datums recursively (returns just BootDatum for backwards compatibility)
fn get_all_datums_recursive(
    b00t_path: &str,
    max_depth: usize,
) -> Result<HashMap<String, BootDatum>> {
    let with_paths = get_all_datums_with_paths(
        b00t_path,
        if max_depth == 0 {
            None
        } else {
            Some(max_depth)
        },
    )?;
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

/// Search datums by regex/literal pattern across key, name, hint, lfmf_category, and path.
///
/// - `pattern`: compiled as regex; falls back to literal substring if invalid regex
/// - `type_filter`: comma-separated DatumType names (case-insensitive); None = all types
/// - `depth`: recursion depth; None = DEFAULT_MAX_DEPTH
pub fn search_datums(
    b00t_path: &str,
    pattern: &str,
    type_filter: Option<&str>,
    depth: Option<usize>,
) -> Result<Vec<DatumSearchResult>> {
    let datums = get_all_datums_with_paths(b00t_path, depth)?;

    // Compile regex; fallback to literal contains
    let re = Regex::new(pattern).ok();

    let type_filters: Option<Vec<String>> = type_filter.map(|t| {
        t.split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let matches_pattern = |s: &str| -> bool {
        if let Some(r) = &re {
            r.is_match(s)
        } else {
            s.contains(pattern)
        }
    };

    let mut results: Vec<DatumSearchResult> = datums
        .into_iter()
        .filter_map(|(key, (datum, path))| {
            // Type filter
            if let Some(filters) = &type_filters {
                let type_str = datum
                    .datum_type
                    .as_ref()
                    .map(|t| format!("{:?}", t).to_lowercase())
                    .unwrap_or_default();
                if !filters.iter().any(|f| type_str.contains(f.as_str())) {
                    return None;
                }
            }

            // Pattern match across fields — first match wins for match_reason
            let match_reason = if matches_pattern(&key) {
                Some("key".to_string())
            } else if matches_pattern(&datum.name) {
                Some("name".to_string())
            } else if matches_pattern(&datum.hint) {
                Some("hint".to_string())
            } else if datum
                .lfmf_category
                .as_deref()
                .map(matches_pattern)
                .unwrap_or(false)
            {
                Some("lfmf_category".to_string())
            } else if matches_pattern(&path) {
                Some("path".to_string())
            } else {
                return None;
            };

            Some(DatumSearchResult {
                key,
                path,
                datum_type: datum.datum_type,
                name: datum.name,
                hint: datum.hint,
                lfmf_category: datum.lfmf_category,
                match_reason,
            })
        })
        .collect();

    results.sort_by(|a, b| a.key.cmp(&b.key));
    Ok(results)
}

/// Apply a `DatumFilter` to a loaded datum map.
///
/// Returns `(key, datum, explain_reason)` tuples.
/// When `explain=true`, filtered-out datums are returned with a non-None reason string.
/// When `explain=false`, only passing datums are returned (reason = None).
pub fn filter_datums(
    datums: HashMap<String, BootDatum>,
    filter: &DatumFilter,
    explain: bool,
) -> Vec<(String, BootDatum, Option<String>)> {
    let current_os = std::env::consts::OS; // "linux", "macos", "windows"

    let mut results = Vec::new();

    for (key, datum) in datums {
        let mut reasons: Vec<String> = Vec::new();

        // datum_type filter — if types list is non-empty, datum must match one of them
        if !filter.datum_types.is_empty() {
            let matches = match &datum.datum_type {
                Some(dt) => filter
                    .datum_types
                    .iter()
                    .any(|ft| std::mem::discriminant(dt) == std::mem::discriminant(ft)),
                None => false,
            };
            if !matches {
                reasons.push(format!(
                    "type mismatch: expected one of {:?}",
                    filter.datum_types
                ));
            }
        }

        // require_os
        if let Some(ref required_os) = filter.require_os {
            if !current_os.eq_ignore_ascii_case(required_os) {
                reasons.push(format!(
                    "OS mismatch: need {}, have {}",
                    required_os, current_os
                ));
            }
        }

        // require_cmds — check each via which/PATH lookup
        for cmd in &filter.require_cmds {
            if which::which(cmd).is_err() {
                reasons.push(format!("CMD not found: {}", cmd));
            }
        }

        // require_constraints — parse "OS:linux" or "CMD:docker" tokens
        for constraint in &filter.require_constraints {
            if let Some(val) = constraint.strip_prefix("OS:") {
                if !current_os.eq_ignore_ascii_case(val) {
                    reasons.push(format!(
                        "constraint OS:{} not satisfied (have {})",
                        val, current_os
                    ));
                }
            } else if let Some(cmd) = constraint.strip_prefix("CMD:") {
                if which::which(cmd).is_err() {
                    reasons.push(format!("constraint CMD:{} not found", cmd));
                }
            }
        }

        // needs_any_env — datum must declare env vars AND at least one is set
        if filter.needs_any_env {
            let any_set = datum
                .env
                .as_ref()
                .map(|env| env.keys().any(|k| std::env::var(k).is_ok()))
                .unwrap_or(false);
            if !any_set {
                reasons.push("needs_any_env: no declared env vars are set".to_string());
            }
        }

        // needs_all_env — datum must declare env vars AND all are set
        if filter.needs_all_env {
            let all_set = datum
                .env
                .as_ref()
                .map(|env| env.keys().all(|k| std::env::var(k).is_ok()))
                .unwrap_or(false);
            if !all_set {
                reasons.push("needs_all_env: not all declared env vars are set".to_string());
            }
        }

        if reasons.is_empty() {
            results.push((key, datum, None));
        } else if explain {
            results.push((key, datum, Some(reasons.join("; "))));
        }
    }

    results.sort_by(|a, b| a.0.cmp(&b.0));
    results
}

/// Build a datum ontology graph from all loaded datums.
///
/// Nodes: one per datum (key, name, type, hint).
/// Edges: depends_on + entangled_* fields → typed directed edges from datum → target.
pub fn build_datum_graph(b00t_path: &str, depth: Option<usize>) -> Result<DatumGraph> {
    let datums = get_all_datums_with_paths(b00t_path, depth)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    for (key, (datum, _path)) in &datums {
        nodes.push(DatumGraphNode {
            key: key.clone(),
            name: datum.name.clone(),
            datum_type: datum.datum_type.clone(),
            hint: datum.hint.clone(),
        });

        // Helper: emit edges for a list of targets under a given edge_type
        macro_rules! push_edges {
            ($field:expr, $label:expr) => {
                if let Some(targets) = $field {
                    for target in targets {
                        edges.push(DatumGraphEdge {
                            from: key.clone(),
                            to: target.clone(),
                            edge_type: $label.to_string(),
                        });
                    }
                }
            };
        }

        push_edges!(&datum.depends_on, "depends_on");
        push_edges!(&datum.entangled_agents, "entangled_agents");
        push_edges!(&datum.entangled_cli, "entangled_cli");
        push_edges!(&datum.entangled_mcp, "entangled_mcp");
        push_edges!(&datum.entangled_ai_models, "entangled_ai_models");
        push_edges!(&datum.entangled_apis, "entangled_apis");
        push_edges!(&datum.entangled_docker, "entangled_docker");
        push_edges!(&datum.entangled_k8s, "entangled_k8s");
    }

    nodes.sort_by(|a, b| a.key.cmp(&b.key));

    Ok(DatumGraph { nodes, edges })
}

/// Serialize a `DatumGraph` to Graphviz DOT format.
///
/// Node labels include key + type. Edge labels show the relationship type.
pub fn graph_to_dot(graph: &DatumGraph) -> String {
    let mut out = String::from("digraph b00t {\n  rankdir=LR;\n  node [shape=box];\n");

    for node in &graph.nodes {
        let type_label = node
            .datum_type
            .as_ref()
            .map(|t| format!("{:?}", t))
            .unwrap_or_else(|| "?".to_string());
        // Escape quotes in labels
        let label = format!("{}\\n{}", node.key, type_label).replace('"', "\\\"");
        let node_key = node.key.replace('"', "\\\"");
        out.push_str(&format!("  \"{}\" [label=\"{}\"];\n", node_key, label));
    }

    for edge in &graph.edges {
        let from = edge.from.replace('"', "\\\"");
        let to = edge.to.replace('"', "\\\"");
        out.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\"];\n",
            from, to, edge.edge_type
        ));
    }

    out.push('}');
    out
}

/// BFS neighbor query on a datum graph.
///
/// Returns all (from, to, edge_type) reachable within `depth` hops from `seed`.
/// `direction`: "out" (seed→x), "in" (x→seed), "both"
pub fn graph_neighbors(
    graph: &DatumGraph,
    seed: &str,
    depth: usize,
    direction: &str,
) -> Vec<DatumGraphEdge> {
    use std::collections::{HashSet, VecDeque};

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut result: Vec<DatumGraphEdge> = Vec::new();

    visited.insert(seed.to_string());
    queue.push_back((seed.to_string(), 0));

    while let Some((current, d)) = queue.pop_front() {
        if d >= depth {
            continue;
        }

        for edge in &graph.edges {
            let (next, include) = match direction {
                "out" if edge.from == current => (edge.to.as_str(), true),
                "in" if edge.to == current => (edge.from.as_str(), true),
                "both" if edge.from == current => (edge.to.as_str(), true),
                "both" if edge.to == current => (edge.from.as_str(), true),
                _ => ("", false),
            };

            if include {
                // Always collect the edge if it is reachable within the depth limit
                result.push(edge.clone());

                // Only enqueue the next node once to avoid revisiting it
                if !visited.contains(next) {
                    visited.insert(next.to_string());
                    queue.push_back((next.to_string(), d + 1));
                }
            }
        }
    }

    result
}

/// Index all datums into irontology-mcp via `GrokClient` for semantic search.
///
/// Builds text = "name: …\ntype: …\nhint: …\n{learn_content}" per datum,
/// then calls `client.learn(text, Some(key))` → `repo.index`.
/// Returns count of indexed datums.
pub async fn index_datums_into_irontology(
    b00t_path: &str,
    client: &mut b00t_c0re_lib::grok::GrokClient,
) -> Result<usize> {
    let datums = get_all_datums_with_paths(b00t_path, None)?;
    let mut count = 0;

    for (key, (datum, _path)) in &datums {
        let type_str = datum
            .datum_type
            .as_ref()
            .map(|t| format!("{:?}", t))
            .unwrap_or_else(|| "unknown".to_string());

        let mut text = format!(
            "name: {}\ntype: {}\nhint: {}\n",
            datum.name, type_str, datum.hint
        );

        if let Ok(Some(learn)) = get_datum_learn_content(b00t_path, datum) {
            text.push('\n');
            // 🤓 cap at 4KB to stay within chunk budget
            text.push_str(&learn.chars().take(4096).collect::<String>());
        }

        match client.learn(&text, Some(key.as_str())).await {
            Ok(_) => count += 1,
            Err(e) => eprintln!("⚠️ index_datums: failed to index {}: {}", key, e),
        }
    }

    Ok(count)
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

    // ── #198 search_datums tests ──────────────────────────────────────────────

    #[test]
    fn test_search_datums_exact_key() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "rust.cli.toml",
            "[b00t]\nname = \"rustc\"\ntype = \"cli\"\nhint = \"Rust compiler\"\n",
        );
        let results = search_datums(b00t_path, "rust.cli", None, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "rust.cli");
        assert_eq!(results[0].match_reason.as_deref(), Some("key"));
    }

    #[test]
    fn test_search_datums_by_hint_regex() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "docker.cli.toml",
            "[b00t]\nname = \"docker\"\ntype = \"cli\"\nhint = \"Container runtime\"\n",
        );
        create_test_datum_file(
            temp_dir.path(),
            "rust.cli.toml",
            "[b00t]\nname = \"rustc\"\ntype = \"cli\"\nhint = \"Rust compiler\"\n",
        );
        // Regex: match "compiler" in hint
        let results = search_datums(b00t_path, "compil", None, None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "rust.cli");
        assert_eq!(results[0].match_reason.as_deref(), Some("hint"));
    }

    #[test]
    fn test_search_datums_type_filter() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "gh.cli.toml",
            "[b00t]\nname = \"gh\"\ntype = \"cli\"\nhint = \"GitHub CLI\"\n",
        );
        create_test_datum_file(
            temp_dir.path(),
            "github.mcp.toml",
            "[b00t]\nname = \"github-mcp\"\ntype = \"mcp\"\nhint = \"GitHub MCP\"\n",
        );
        // Both contain "github" but only mcp type
        let results = search_datums(b00t_path, "git", Some("mcp"), None).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].key, "github.mcp");
    }

    #[test]
    fn test_search_datums_no_match() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "rust.cli.toml",
            "[b00t]\nname = \"rustc\"\ntype = \"cli\"\nhint = \"Rust compiler\"\n",
        );
        let results = search_datums(b00t_path, "zzz_no_match_zzz", None, None).unwrap();
        assert!(results.is_empty());
    }

    // ── #199 filter_datums tests ──────────────────────────────────────────────

    #[test]
    fn test_filter_datums_by_type() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "gh.cli.toml",
            "[b00t]\nname = \"gh\"\ntype = \"cli\"\nhint = \"GitHub CLI\"\n",
        );
        create_test_datum_file(
            temp_dir.path(),
            "github.mcp.toml",
            "[b00t]\nname = \"github-mcp\"\ntype = \"mcp\"\nhint = \"GitHub MCP\"\n",
        );
        let datums = get_all_datums(b00t_path).unwrap();
        let filter = DatumFilter {
            datum_types: vec![crate::DatumType::Mcp],
            ..Default::default()
        };
        let results = filter_datums(datums, &filter, false);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "github.mcp");
    }

    #[test]
    fn test_filter_datums_explain_os_mismatch() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "tool.cli.toml",
            "[b00t]\nname = \"tool\"\ntype = \"cli\"\nhint = \"A tool\"\n",
        );
        let datums = get_all_datums(b00t_path).unwrap();
        let filter = DatumFilter {
            require_constraints: vec!["OS:plan9".to_string()], // impossible OS
            ..Default::default()
        };
        let results = filter_datums(datums, &filter, true); // explain=true
        assert_eq!(results.len(), 1);
        assert!(results[0].2.is_some()); // reason present
        assert!(results[0].2.as_ref().unwrap().contains("plan9"));
    }

    #[test]
    fn test_filter_datums_empty_result() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "tool.cli.toml",
            "[b00t]\nname = \"tool\"\ntype = \"cli\"\nhint = \"A tool\"\n",
        );
        let datums = get_all_datums(b00t_path).unwrap();
        let filter = DatumFilter {
            require_constraints: vec!["CMD:zzz_impossible_cmd_zzz".to_string()],
            ..Default::default()
        };
        let results = filter_datums(datums, &filter, false); // explain=false → empty
        assert!(results.is_empty());
    }

    // ── #201 graph tests ──────────────────────────────────────────────────────

    #[test]
    fn test_build_datum_graph_nodes() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "rust.cli.toml",
            "[b00t]\nname = \"rustc\"\ntype = \"cli\"\nhint = \"Rust compiler\"\n",
        );
        let graph = build_datum_graph(b00t_path, None).unwrap();
        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.nodes[0].key, "rust.cli");
    }

    #[test]
    fn test_build_datum_graph_depends_on_edges() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "cargo.cli.toml",
            "[b00t]\nname = \"cargo\"\ntype = \"cli\"\nhint = \"Rust build tool\"\ndepends_on = [\"rust.cli\"]\n",
        );
        let graph = build_datum_graph(b00t_path, None).unwrap();
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].from, "cargo.cli");
        assert_eq!(graph.edges[0].to, "rust.cli");
        assert_eq!(graph.edges[0].edge_type, "depends_on");
    }

    #[test]
    fn test_graph_to_dot_syntax() {
        let graph = DatumGraph {
            nodes: vec![DatumGraphNode {
                key: "rust.cli".to_string(),
                name: "rustc".to_string(),
                datum_type: None,
                hint: "Rust compiler".to_string(),
            }],
            edges: vec![DatumGraphEdge {
                from: "cargo.cli".to_string(),
                to: "rust.cli".to_string(),
                edge_type: "depends_on".to_string(),
            }],
        };
        let dot = graph_to_dot(&graph);
        assert!(dot.contains("digraph b00t {"));
        assert!(dot.contains("->"));
        assert!(dot.contains("rust.cli"));
        assert!(dot.contains("depends_on"));
    }

    #[test]
    fn test_graph_neighbors_out() {
        let graph = DatumGraph {
            nodes: vec![],
            edges: vec![
                DatumGraphEdge {
                    from: "cargo.cli".to_string(),
                    to: "rust.cli".to_string(),
                    edge_type: "depends_on".to_string(),
                },
                DatumGraphEdge {
                    from: "rust.cli".to_string(),
                    to: "llvm.cli".to_string(),
                    edge_type: "depends_on".to_string(),
                },
            ],
        };
        // depth=1 from cargo → only rust.cli
        let neighbors = graph_neighbors(&graph, "cargo.cli", 1, "out");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].to, "rust.cli");

        // depth=2 from cargo → rust.cli + llvm.cli
        let neighbors2 = graph_neighbors(&graph, "cargo.cli", 2, "out");
        assert_eq!(neighbors2.len(), 2);
    }

    #[test]
    fn test_graph_neighbors_in() {
        let graph = DatumGraph {
            nodes: vec![],
            edges: vec![DatumGraphEdge {
                from: "cargo.cli".to_string(),
                to: "rust.cli".to_string(),
                edge_type: "depends_on".to_string(),
            }],
        };
        // inbound to rust.cli → cargo.cli
        let neighbors = graph_neighbors(&graph, "rust.cli", 1, "in");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].from, "cargo.cli");
    }

    // ── #200 semantic search / index text tests ───────────────────────────────

    #[test]
    fn test_index_datums_text_format() {
        // Validate the text we'd build for indexing (without actually calling irontology)
        let datum = crate::BootDatum {
            name: "rustc".to_string(),
            datum_type: Some(crate::DatumType::Cli),
            hint: "Rust compiler".to_string(),
            ..Default::default()
        };
        let type_str = datum
            .datum_type
            .as_ref()
            .map(|t| format!("{:?}", t))
            .unwrap_or_default();
        let text = format!(
            "name: {}\ntype: {}\nhint: {}\n",
            datum.name, type_str, datum.hint
        );
        assert!(text.contains("name: rustc"));
        assert!(text.contains("type: Cli"));
        assert!(text.contains("hint: Rust compiler"));
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

    #[test]
    fn test_git_attributes_overlay_operational_metadata() {
        let temp_dir = TempDir::new().unwrap();
        std::process::Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        create_test_datum_file(
            temp_dir.path(),
            "plantuml.mcp.toml",
            "[b00t]\nname = \"plantuml\"\ntype = \"mcp\"\nhint = \"PlantUML\"\n",
        );
        std::fs::write(
            temp_dir.path().join(".gitattributes"),
            "plantuml.mcp.toml b00t.status=sunset b00t.enabled=false b00t.status_msg=java-too-heavy b00t.replacement=b00t-viz\n",
        )
        .unwrap();

        let datums = get_all_datums_with_paths(temp_dir.path().to_str().unwrap(), Some(0)).unwrap();
        let (datum, _) = datums.get("plantuml.mcp").unwrap();
        assert_eq!(datum.status.as_deref(), Some("sunset"));
        assert_eq!(datum.enabled, Some(false));
        assert_eq!(datum.status_msg.as_deref(), Some("java-too-heavy"));
        assert_eq!(datum.replacement.as_deref(), Some("b00t-viz"));
        assert_eq!(
            datum.git_attributes.get("status_msg").map(String::as_str),
            Some("java-too-heavy")
        );
    }

    #[test]
    fn test_nested_git_attributes_overlay_operational_metadata() {
        let temp_dir = TempDir::new().unwrap();
        std::process::Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .unwrap();
        let b00t_dir = temp_dir.path().join("_b00t_");
        std::fs::create_dir(&b00t_dir).unwrap();
        create_test_datum_file(
            &b00t_dir,
            "plantuml.mcp.toml",
            "[b00t]\nname = \"plantuml\"\ntype = \"mcp\"\nhint = \"PlantUML\"\n",
        );
        std::fs::write(temp_dir.path().join(".gitattributes"), "* -text\n").unwrap();
        std::fs::write(
            b00t_dir.join(".gitattributes"),
            "plantuml.mcp.toml b00t.status=sunset b00t.enabled=false b00t.status_msg=java-too-heavy b00t.search=exclude\n",
        )
        .unwrap();

        let datums = get_all_datums_with_paths(b00t_dir.to_str().unwrap(), Some(0)).unwrap();
        let (datum, _) = datums.get("plantuml.mcp").unwrap();
        assert_eq!(datum.status.as_deref(), Some("sunset"));
        assert_eq!(datum.enabled, Some(false));
        assert_eq!(datum.status_msg.as_deref(), Some("java-too-heavy"));
        assert_eq!(
            datum.git_attributes.get("search").map(String::as_str),
            Some("exclude")
        );
    }

    // ── tomllmd precedence tests ──────────────────────────────────────────────

    #[test]
    fn test_tomllmd_wins_over_tomllm_and_toml_same_key() {
        // .tomllmd > .tomllm > .toml for the same datum key
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();

        // All three variants present for the same key "tool.cli"
        create_test_datum_file(
            temp_dir.path(),
            "tool.cli.toml",
            "[b00t]\nname = \"tool-toml\"\ntype = \"cli\"\nhint = \"toml variant\"\n",
        );
        create_test_datum_file(
            temp_dir.path(),
            "tool.cli.tomllm",
            "[b00t]\nname = \"tool-tomllm\"\ntype = \"cli\"\nhint = \"tomllm variant\"\n",
        );
        create_test_datum_file(
            temp_dir.path(),
            "tool.cli.tomllmd",
            "[b00t]\nname = \"tool-tomllmd\"\ntype = \"cli\"\nhint = \"tomllmd variant\"\n",
        );

        let datums = get_all_datums_with_paths(b00t_path, Some(0)).unwrap();
        // The key strips the outer extension, so all three map to "tool.cli"
        assert!(datums.contains_key("tool.cli"), "key tool.cli must exist");
        let (datum, path) = datums.get("tool.cli").unwrap();
        assert_eq!(datum.name, "tool-tomllmd", ".tomllmd must win");
        assert!(path.ends_with(".tomllmd"), "path must end with .tomllmd");
    }

    #[test]
    fn test_tomllm_wins_over_toml_same_key() {
        let temp_dir = TempDir::new().unwrap();
        let b00t_path = temp_dir.path().to_str().unwrap();

        create_test_datum_file(
            temp_dir.path(),
            "tool.cli.toml",
            "[b00t]\nname = \"tool-toml\"\ntype = \"cli\"\nhint = \"toml variant\"\n",
        );
        create_test_datum_file(
            temp_dir.path(),
            "tool.cli.tomllm",
            "[b00t]\nname = \"tool-tomllm\"\ntype = \"cli\"\nhint = \"tomllm variant\"\n",
        );

        let datums = get_all_datums_with_paths(b00t_path, Some(0)).unwrap();
        assert!(datums.contains_key("tool.cli"));
        let (datum, path) = datums.get("tool.cli").unwrap();
        assert_eq!(datum.name, "tool-tomllm", ".tomllm must win over .toml");
        assert!(path.ends_with(".tomllm"));
    }
}
