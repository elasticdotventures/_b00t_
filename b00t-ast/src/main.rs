// b00t-ast/src/main.rs
//
// CLI binary for b00t-ast — walk Rust source trees, extract code elements,
// build ontology graphs, and output as JSON for codebase-memory-mcp ingestion.
//
// Usage:
//   b00t-ast dir /path/to/project            # Extract + output JSON
//   b00t-ast dir /path/to/project --json     # Same (default)
//   b00t-ast dir /path/to/project --mcp      # MCP-compatible payload
//   b00t-ast dir /path/to/project --counts   # Summary counts only
//   b00t-ast self                             # Index b00t-ast itself
//
// Output goes to stdout; errors to stderr.

use b00t_ast::{ontology::OntologyGraph, run_extraction};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "b00t-ast",
    about = "Rust AST extraction and ontology graph builder"
)]
enum Cli {
    /// Extract code elements from a directory of Rust source files
    Dir {
        /// Project root directory
        path: PathBuf,

        /// Output format: json (default), mcp-payload, or counts
        #[arg(long, default_value = "json")]
        format: String,

        /// Limit results to N elements (0 = unlimited)
        #[arg(long, default_value_t = 0)]
        limit: usize,
    },
    /// Index b00t-ast's own source tree (convenience)
    Self_ {
        /// Output format
        #[arg(long, default_value = "json")]
        format: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli {
        Cli::Dir {
            path,
            format,
            limit,
        } => {
            run_extraction_with_opts(&path, &format, limit);
        }
        Cli::Self_ { format } => {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            run_extraction_with_opts(&manifest_dir, &format, 0);
        }
    }
}

fn run_extraction_with_opts(path: &PathBuf, format: &str, limit: usize) {
    let path_str = path.to_string_lossy().to_string();
    match run_extraction(&path_str) {
        Ok(result) => {
            match format {
                "counts" => {
                    println!("📁 {} files scanned", result.file_count);
                    println!("📊 {} total elements", result.elements.len());
                    let mut kinds: Vec<_> = result.counts.iter().collect();
                    kinds.sort_by(|a, b| b.1.cmp(a.1));
                    for (kind, count) in &kinds {
                        println!("  {kind:<15} {count}");
                    }
                    if !result.errors.is_empty() {
                        eprintln!("\n⚠️  {} parse errors:", result.errors.len());
                        for err in &result.errors {
                            eprintln!("  {err}");
                        }
                    }
                }
                "mcp" | "mcp-payload" => {
                    let graph = OntologyGraph::from_extraction(&result);
                    let payload = graph.to_mcp_payload();
                    let limited = if limit > 0 {
                        // Apply limit to nodes/edges for MCP payload
                        let mut limited = payload;
                        if let Some(nodes) = limited.get_mut("nodes").and_then(|n| n.as_array_mut())
                        {
                            nodes.truncate(limit);
                        }
                        limited
                    } else {
                        payload
                    };
                    println!("{}", serde_json::to_string_pretty(&limited).unwrap());
                }
                _ => {
                    // Default JSON output
                    let json = result.to_json();
                    if limit > 0 {
                        // Limit elements in output
                        let mut v: serde_json::Value =
                            serde_json::from_str(&json).unwrap_or(serde_json::Value::Null);
                        if let Some(elements) = v.get_mut("elements").and_then(|e| e.as_array_mut())
                        {
                            elements.truncate(limit);
                            elements.shrink_to_fit();
                        }
                        println!("{}", serde_json::to_string_pretty(&v).unwrap());
                    } else {
                        println!("{json}");
                    }
                }
            }

            if result.errors.len() > 5 {
                eprintln!("... and {} more errors", result.errors.len() - 5);
            }
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}
