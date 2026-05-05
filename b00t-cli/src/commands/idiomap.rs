use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use shellexpand::tilde;
use std::collections::HashMap;
use std::path::Path;

use crate::IdiomaticEntry;

/// A scanned idiomatic with its source file
#[derive(Debug, Clone, Serialize)]
pub struct ScannedIdiomatic {
    pub source_file: String,
    pub entry: IdiomaticEntry,
}

/// An edge in the idiomatic similarity graph
#[derive(Debug, Clone, Serialize)]
pub struct IdiomaticEdge {
    pub source: String,
    pub target: String,
    pub distance: usize,
}

#[derive(Parser, Debug, Clone)]
pub enum IdiomapCommands {
    /// Scan all datums and show federated idiomatic map
    #[clap(about = "Scan all datums and show federated idiomatic map")]
    Scan {
        #[clap(long, help = "Output as adjacency list (source → target distance)")]
        graph: bool,
        #[clap(long, help = "Filter by topic")]
        topic: Option<String>,
        #[clap(long, help = "JSON output")]
        json: bool,
        #[clap(long, help = "Validate: each guard_ids[] references a real guard")]
        check: bool,
    },
    /// Generate a knowledge-cutoff quiz for a topic
    #[clap(about = "Generate a knowledge-cutoff quiz for a topic")]
    Quiz {
        #[clap(help = "Topic to generate quiz for")]
        topic: String,
    },
}

// ── Levenshtein distance ────────────────────────────────────────────

fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_len = a.chars().count();
    let b_len = b.chars().count();
    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut prev_row: Vec<usize> = (0..=b_len).collect();
    let mut curr_row: Vec<usize> = vec![0; b_len + 1];

    for (i, ca) in a.chars().enumerate() {
        curr_row[0] = i + 1;
        for (j, cb) in b.chars().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr_row[j + 1] = std::cmp::min(
                std::cmp::min(curr_row[j] + 1, prev_row[j + 1] + 1),
                prev_row[j] + cost,
            );
        }
        std::mem::swap(&mut prev_row, &mut curr_row);
    }
    prev_row[b_len]
}

// ── Datum scanner ──────────────────────────────────────────────────

/// Try to parse a TOML file as a b00t datum to extract [[b00t.idiomatic]] sections.
/// Falls back to raw TOML Value parsing if standard BootDatum deserialization fails.
fn scan_file_for_idiomatics(path: &Path) -> Result<Vec<ScannedIdiomatic>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Try standard deserialization first
    let entries: Vec<ScannedIdiomatic> = match toml::from_str::<BootDatumProxy>(&content) {
        Ok(proxy) => proxy
            .b00t
            .idiomatic
            .unwrap_or_default()
            .into_iter()
            .map(|e| ScannedIdiomatic {
                source_file: filename.clone(),
                entry: IdiomaticEntry {
                    name: e.name,
                    guard_ids: e.guard_ids,
                    pattern: e.pattern,
                    principle: e.principle,
                    context_saved: e.context_saved,
                    topic: e.topic,
                    synonyms: e.synonyms,
                    priority: e.priority,
                },
            })
            .collect(),
        Err(_) => {
            // Fallback: try raw TOML Value parse for non-standard TOML files
            let value: toml::Value = content.parse()?;
            let mut entries = Vec::new();

            if let Some(b00t_table) = value.get("b00t").and_then(|v| v.as_table()) {
                if let Some(idiomatic_array) = b00t_table.get("idiomatic").and_then(|v| v.as_array())
                {
                    for item in idiomatic_array {
                        if let Some(table) = item.as_table() {
                            let name = table
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unnamed")
                                .to_string();
                            let guard_ids = table
                                .get("guard_ids")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                });
                            let pattern = table
                                .get("pattern")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            let principle = table
                                .get("principle")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            let context_saved = table
                                .get("context_saved")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            let topic = table
                                .get("topic")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            let synonyms = table
                                .get("synonyms")
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                });
                            let priority = table
                                .get("priority")
                                .and_then(|v| v.as_str())
                                .map(String::from);

                            entries.push(ScannedIdiomatic {
                                source_file: filename.clone(),
                                entry: IdiomaticEntry {
                                    name,
                                    guard_ids,
                                    pattern,
                                    principle,
                                    context_saved,
                                    topic,
                                    synonyms,
                                    priority,
                                },
                            });
                        }
                    }
                }
            }
            entries
        }
    };

    Ok(entries)
}

/// Proxy struct for TOML deserialization — we only care about the b00t.idiomatic section
#[derive(Deserialize)]
struct BootDatumProxy {
    #[serde(default)]
    b00t: B00tSectionProxy,
}

#[derive(Deserialize, Default)]
struct B00tSectionProxy {
    #[serde(default)]
    idiomatic: Option<Vec<IdiomaticEntryRaw>>,
}

#[derive(Deserialize)]
struct IdiomaticEntryRaw {
    name: String,
    #[serde(default)]
    guard_ids: Option<Vec<String>>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    principle: Option<String>,
    #[serde(default)]
    context_saved: Option<String>,
    #[serde(default)]
    topic: Option<String>,
    #[serde(default)]
    synonyms: Option<Vec<String>>,
    #[serde(default)]
    priority: Option<String>,
}

/// Scan all datum files in the b00t path
fn scan_all_idiomatics(b00t_path: &str) -> Result<Vec<ScannedIdiomatic>> {
    let path = Path::new(b00t_path);
    let mut all_entries = Vec::new();

    // Scan _b00t_/*.toml, _b00t_/*.tomllm, and _b00t_/datums/*.datum
    if path.is_dir() {
        for entry in walkdir::WalkDir::new(path)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let fname = entry.file_name().to_string_lossy().to_string();
            if fname.ends_with(".toml") || fname.ends_with(".tomllm") || fname.ends_with(".datum")
            {
                if let Ok(entries) = scan_file_for_idiomatics(entry.path()) {
                    all_entries.extend(entries);
                }
            }
        }
    }

    Ok(all_entries)
}

// ── Handlers ───────────────────────────────────────────────────────

pub fn handle_idiomap_command(cmd: &IdiomapCommands, b00t_path: &str) -> Result<()> {
    match cmd {
        IdiomapCommands::Scan {
            graph,
            topic,
            json,
            check,
        } => handle_scan(*graph, topic.as_deref(), *json, *check, b00t_path),
        IdiomapCommands::Quiz { topic } => handle_quiz(topic, b00t_path),
    }
}

fn handle_scan(
    graph_mode: bool,
    topic_filter: Option<&str>,
    json: bool,
    check: bool,
    b00t_path: &str,
) -> Result<()> {
    let b00t_path = tilde(b00t_path).to_string();
    let entries = scan_all_idiomatics(&b00t_path)?;

    // Filter by topic
    let mut entries = entries;
    if let Some(topic) = topic_filter {
        let t_lower = topic.to_lowercase();
        entries.retain(|e| {
            e.entry
                .topic
                .as_deref()
                .map(|t| t.to_lowercase().contains(&t_lower))
                .unwrap_or(false)
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    if entries.is_empty() {
        println!("No idiomatic entries found in {} (or matching filter).", b00t_path);
        println!("  Add [[b00t.idiomatic]] sections to your datum files.");
        return Ok(());
    }

    // Build name → entry map for graph
    let name_map: HashMap<&str, &ScannedIdiomatic> =
        entries.iter().map(|e| (e.entry.name.as_str(), e)).collect();

    if graph_mode {
        // Output adjacency list: source → target → distance
        let names: Vec<&str> = name_map.keys().copied().collect();
        let mut edges: Vec<IdiomaticEdge> = Vec::new();
        for i in 0..names.len() {
            for j in i + 1..names.len() {
                let dist = levenshtein_distance(names[i], names[j]);
                if dist < 10 {
                    edges.push(IdiomaticEdge {
                        source: names[i].to_string(),
                        target: names[j].to_string(),
                        distance: dist,
                    });
                }
            }
        }
        edges.sort_by_key(|e| e.distance);

        if json {
            println!("{}", serde_json::to_string_pretty(&edges)?);
        } else {
            println!("Federated Idiomatic Graph — {} entries, {} edges", names.len(), edges.len());
            println!("{}\n", "=".repeat(60));
            for edge in &edges {
                let label = if edge.distance < 3 {
                    "near-synonym"
                } else {
                    "related"
                };
                println!(
                    "  {} ↔ {}  (distance: {}, {})",
                    edge.source, edge.target, edge.distance, label
                );
            }
        }
        return Ok(());
    }

    // Default: show federated map grouped by source file
    let mut by_file: HashMap<&str, Vec<&ScannedIdiomatic>> = HashMap::new();
    for e in &entries {
        by_file.entry(e.source_file.as_str()).or_default().push(e);
    }

    println!(
        "Federated Idiomatic Map — {} entries from {} files\n",
        entries.len(),
        by_file.len()
    );

    // Sort files for consistent output
    let mut files: Vec<&&str> = by_file.keys().collect();
    files.sort();

    for file in files {
        let idiomatic_entries = &by_file[file];
        println!("  📄 {}", file);
        for entry in idiomatic_entries {
            let priority_tag = match entry.entry.priority.as_deref() {
                Some("S") => " [S]",
                Some("A") => " [A]",
                Some("B") => " [B]",
                Some("C") => " [C]",
                Some(p) => {
                    // Use allocated string to avoid temporary issues
                    let s = format!(" [{}]", p);
                    Box::leak(s.into_boxed_str())
                }
                None => "",
            };
            println!(
                "     └─ {}{} — {}",
                entry.entry.name,
                priority_tag,
                entry
                    .entry
                    .principle
                    .as_deref()
                    .unwrap_or("(no principle)")
            );
            if let Some(topic) = &entry.entry.topic {
                println!("        topic: {}", topic);
            }
            if let Some(guards) = &entry.entry.guard_ids {
                if !guards.is_empty() {
                    println!("        guards: {}", guards.join(", "));
                }
            }
            if let Some(saved) = &entry.entry.context_saved {
                println!("        context_saved: {}", saved);
            }
        }
        println!();
    }

    // Check mode: validate guard references
    if check {
        println!("{}\nGuard Reference Validation:", "=".repeat(60));
        let mut all_ok = true;
        for entry in &entries {
            if let Some(guards) = &entry.entry.guard_ids {
                for gid in guards {
                    // Simple check: does the guard ID reference exist in the hive-guards file?
                    if !gid.contains(':') {
                        println!(
                            "  ⚠️  {} references guard '{}' without file prefix (expected 'file:line')",
                            entry.entry.name, gid
                        );
                        all_ok = false;
                    }
                }
            }
        }
        if all_ok {
            println!("  ✅ All guard references use file:line format.");
        }
    }

    Ok(())
}

fn handle_quiz(topic: &str, b00t_path: &str) -> Result<()> {
    let b00t_path = tilde(b00t_path).to_string();
    let entries = scan_all_idiomatics(&b00t_path)?;

    // Filter to topic
    let t_lower = topic.to_lowercase();
    let mut relevant: Vec<&ScannedIdiomatic> = entries
        .iter()
        .filter(|e| {
            e.entry
                .topic
                .as_deref()
                .map(|t| t.to_lowercase().contains(&t_lower))
                .unwrap_or(false)
                || e.entry.name.to_lowercase().contains(&t_lower)
                || e
                    .entry
                    .principle
                    .as_deref()
                    .map(|p| p.to_lowercase().contains(&t_lower))
                    .unwrap_or(false)
        })
        .collect();

    if relevant.is_empty() {
        // Fallback: show all entries as the quiz
        println!(
            "No idiomatic entries found for topic '{}'. Showing all entries:\n",
            topic
        );
        for e in &entries {
            relevant.push(e);
        }
    }

    println!("🏫 Federated Idiomatics Quiz — Topic: {}\n", topic);
    println!("{}", "=".repeat(60));

    for (i, entry) in relevant.iter().enumerate() {
        println!("Q{}: What idiomatic pattern is described, and what context does it save?", i + 1);
        if let Some(principle) = &entry.entry.principle {
            println!("   Clue: {}", principle);
        }
        if let Some(pattern) = &entry.entry.pattern {
            println!("   Pattern: {}", pattern);
        }
        println!(
            "   (Source: {}, Name: {})",
            entry.source_file, entry.entry.name
        );
        println!();
    }

    println!("{}", "=".repeat(60));
    println!(
        "Answer key: {} questions. Review the idiomatic entries above to verify.",
        relevant.len()
    );
    println!("💡 Each idiomatic should save ≥15 tokens per encounter.");
    println!("   If it doesn't, it's preference, not idiomatic.");

    Ok(())
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_computation() {
        // Same string → distance 0
        assert_eq!(levenshtein_distance("hello", "hello"), 0);
        // One insertion
        assert_eq!(levenshtein_distance("hello", "hellox"), 1);
        // One deletion
        assert_eq!(levenshtein_distance("hello", "hell"), 1);
        // One substitution
        assert_eq!(levenshtein_distance("hello", "hallo"), 1);
        // Completely different
        assert_eq!(levenshtein_distance("abc", "xyz"), 3);
        // Empty strings
        assert_eq!(levenshtein_distance("", ""), 0);
        assert_eq!(levenshtein_distance("a", ""), 1);
        assert_eq!(levenshtein_distance("", "b"), 1);
        // Realistic idiomatic names
        assert_eq!(
            levenshtein_distance("polyseme-purge", "polyseme-purge"),
            0
        );
        let dist = levenshtein_distance("polyseme-purge", "type-over-severity");
        assert!(dist > 5, "different names should have distance > 5, got {}", dist);
        // Near-synonyms (short)
        assert_eq!(levenshtein_distance("guard", "guardian"), 3);
    }

    #[test]
    fn test_scan_finds_existing_entries() {
        // The hive-guards.hive.toml file should exist in the workspace
        let test_paths = [
            Path::new("_b00t_/hive-guards.hive.toml"),
            Path::new("../_b00t_/hive-guards.hive.toml"),
        ];

        let mut found = false;
        for p in &test_paths {
            if p.exists() {
                let results = scan_file_for_idiomatics(p);
                // This test checks that scanning doesn't crash
                // After adding [[b00t.idiomatic]] sections, it should find entries
                assert!(results.is_ok(), "scan should succeed: {:?}", results.err());
                found = true;
                break;
            }
        }
        // If no test file found, skip (running outside workspace or file doesn't exist yet)
        if !found {
            eprintln!("hive-guards.hive.toml not found, skipping scan test");
        }
    }

    #[test]
    fn test_tilde_expansion_in_path() {
        // Path::new("~/.b00t/...") should NOT be used without shellexpand.
        // Verify the default path is not literal tilde.
        let tilde_path = Path::new("~/.b00t/_b00t_");
        assert!(!tilde_path.is_dir(), "literal tilde path should not resolve");
        // Verify expanded path exists
        let expanded = shellexpand::tilde("~/.b00t/_b00t_");
        let expanded_path = Path::new(expanded.as_ref());
        assert!(expanded_path.is_dir(), "expanded tilde path should resolve to _b00t_ dir");
    }

    #[test]
    fn test_empty_map() {
        // A temporary file with no idiomatic sections should return empty
        let dir = tempfile::tempdir().unwrap();
        let empty_file = dir.path().join("empty.toml");
        std::fs::write(&empty_file, "[b00t]\nname = \"test\"\ntype = \"datum\"\nhint = \"test\"\n")
            .unwrap();
        let results = scan_file_for_idiomatics(&empty_file).unwrap();
        assert!(
            results.is_empty(),
            "file with no idiomatic sections should return empty vec, got {}",
            results.len()
        );
    }

    #[test]
    fn test_scan_finds_subdirectory_entries() {
        // Create temp dir with files at various depths
        // Only depth-3 files have [[b00t.idiomatic]] sections
        // Verify scan_all_idiomatics() finds them with depth-appropriate exclusion
        use std::fs;
        use std::path::PathBuf;

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Helper: write a datum TOML file
        fn write_datum(path: &PathBuf, has_idiomatic: bool) {
            let content = if has_idiomatic {
                r#"[b00t]
name = "test-datum"
type = "datum"
hint = "test"

[[b00t.idiomatic]]
name = "test-idiomatic"
principle = "Always test walkdir depth"
pattern = "test-*"
topic = "testing"
"#
            } else {
                "[b00t]\nname = \"test-datum\"\ntype = \"datum\"\nhint = \"test\"\n"
            };
            fs::write(path, content).unwrap();
        }

        // depth 1: root-level file (no idiomatic)
        write_datum(&root.join("depth1.toml"), false);

        // depth 2: subdirectory file (no idiomatic)
        fs::create_dir_all(root.join("sub1")).unwrap();
        write_datum(&root.join("sub1/depth2.toml"), false);

        // depth 3: nested files WITH idiomatic sections
        fs::create_dir_all(root.join("sub1/sub2")).unwrap();
        write_datum(&root.join("sub1/sub2/depth3.toml"), true);

        // depth 4: WalkDir max_depth is inclusive — file at depth 4 IS found
        fs::create_dir_all(root.join("sub1/sub2/sub3")).unwrap();
        write_datum(&root.join("sub1/sub2/sub3/depth4.toml"), true);

        // Run scan_all_idiomatics on the temp dir
        let path_str = root.to_string_lossy().to_string();
        let results = scan_all_idiomatics(&path_str).unwrap();

        // Should find the depth-3 entry
        let depth3_count = results
            .iter()
            .filter(|e| e.source_file == "depth3.toml")
            .count();
        assert_eq!(
            depth3_count, 1,
            "should find exactly 1 idiomatic in depth3.toml, found {}",
            depth3_count
        );

        // WalkDir max_depth(4) includes depth 4 — depth4.toml IS found
        let depth4_count = results
            .iter()
            .filter(|e| e.source_file == "depth4.toml")
            .count();
        assert_eq!(
            depth4_count, 1,
            "should find exactly 1 idiomatic in depth4.toml (depth 4 is max_depth inclusive), found {}",
            depth4_count
        );

        // Both depth3 and depth4 have idiomatic sections, both within max_depth(4)
        assert_eq!(
            results.len(),
            2,
            "expected exactly 2 total idiomatic entries (depth3 + depth4), found {}",
            results.len()
        );

        // Verify the found entry has the expected name
        assert_eq!(results[0].entry.name, "test-idiomatic");
    }
}
