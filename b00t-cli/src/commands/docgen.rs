// b00t-cli/src/commands/docgen.rs
//
// l3dg3rr docgen proxy — MCP client that queries codebase-memory-mcp
// and emits .tomllm / rustdoc / json output.
//
// Run via: b00t-cli docgen --format=tomllm --project=<name> --limit=30
// Or via:  b00t grok ask "show l3dg3rr docs for <project>"
//
// This is the Rust-native replacement for the Python prototype at
// vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/_b00t_/scripts/l3dg3rr-doc-proxy.py

use anyhow::{Context, Result};
use clap::Parser;
use serde_json::Value;

#[derive(Parser, Debug)]
pub struct DocgenArgs {
    /// Project name in the l3dg3rr knowledge graph
    #[arg(long, default_value = "home-brianh-.b00t-vendor-codebase-memory-mcp-b00t-ir0n-ledg3rr")]
    pub project: String,

    /// Output format: tomllm, rustdoc, or json
    #[arg(long, default_value = "tomllm")]
    pub format: DocgenFormat,

    /// Max results
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocgenFormat {
    Tomllm,
    Rustdoc,
    Json,
}

impl std::str::FromStr for DocgenFormat {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tomllm" | "toml" => Ok(DocgenFormat::Tomllm),
            "rustdoc" | "doc" => Ok(DocgenFormat::Rustdoc),
            "json" => Ok(DocgenFormat::Json),
            _ => Err(format!("Unknown format: {s}. Use tomllm, rustdoc, or json")),
        }
    }
}

/// Run the docgen command: query l3dg3rr graph, format output
pub fn run_docgen(args: &DocgenArgs) -> Result<String> {
    let cbm_path = find_cbm_binary()?;

    // Build and run the search_graph MCP call
    let params = serde_json::json!({
        "project": args.project,
        "query": "function method struct enum trait",
        "limit": args.limit
    });

    let output = std::process::Command::new(&cbm_path)
        .args(["cli", "search_graph", &params.to_string()])
        .output()
        .context("failed to execute codebase-memory-mcp")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("cbm search_graph failed: {stderr}");
    }

    // Parse JSON output (strip log lines that precede it)
    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: Value = stdout
        .lines()
        .rev()
        .find(|l| l.trim().starts_with('{') || l.trim().starts_with('['))
        .and_then(|l| serde_json::from_str(l.trim()).ok())
        .context("no valid JSON found in cbm output")?;

    let functions = results
        .get("results")
        .and_then(|r| r.as_array())
        .map(|r| r.clone())
        .unwrap_or_default();

    let output = match args.format {
        DocgenFormat::Tomllm => format_tomllm(&functions),
        DocgenFormat::Rustdoc => format_rustdoc(&functions),
        DocgenFormat::Json => serde_json::to_string_pretty(&functions)?,
    };

    Ok(output)
}

fn find_cbm_binary() -> Result<String> {
    // Allow overriding via environment variable for CI / out-of-tree setups
    if let Ok(path) = std::env::var("B00T_CBM_BINARY") {
        if !path.is_empty() {
            return Ok(path);
        }
    }

    // Check standard build location first
    let candidates = [
        // Relative to CARGO_MANIFEST_DIR (set at compile time) so invocation
        // location does not affect resolution
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp"
        ),
        // Expanded home path
        "~/vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp",
        // In PATH
        "codebase-memory-mcp",
    ];

    for candidate in &candidates {
        let expanded = shellexpand::tilde(candidate).to_string();
        if std::path::Path::new(&expanded).exists() {
            return Ok(expanded);
        }
        // Check if it's a command in PATH
        if !candidate.contains('/') {
            if let Ok(path) = std::process::Command::new("which")
                .arg(candidate)
                .output()
            {
                if path.status.success() {
                    return Ok(candidate.to_string());
                }
            }
        }
    }

    anyhow::bail!(
        "codebase-memory-mcp binary not found. Set B00T_CBM_BINARY env var, \
         add it to PATH, or build it: cd vendor/... && make -f Makefile.cbm cbm"
    )
}

fn format_tomllm(functions: &[Value]) -> String {
    let mut out = String::new();
    out.push_str("# l3dg3rr docgen export — .tomllm format\n");
    out.push_str("# schema: docgen-v1 | Auto-generated from knowledge graph\n\n");

    for f in functions {
        let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let qn = f.get("qualified_name").and_then(|v| v.as_str()).unwrap_or(name);
        let file = f.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let sig = f.get("signature").and_then(|v| v.as_str()).unwrap_or("");
        let rt = f.get("return_type").and_then(|v| v.as_str()).unwrap_or("");
        let complexity = f.get("complexity").and_then(|v| v.as_u64()).unwrap_or(0);
        let doc = f.get("docstring").and_then(|v| v.as_str()).unwrap_or("");

        out.push_str(&format!("[[{qn}]]\n"));
        out.push_str(&format!("name = \"{name}\"\n"));
        out.push_str(&format!("file_path = \"{file}\"\n"));
        out.push_str(&format!("signature = \"{}\"\n", sig.escape_default()));
        out.push_str(&format!("return_type = \"{rt}\"\n"));
        out.push_str(&format!("complexity = {complexity}\n"));
        if !doc.is_empty() {
            let oneline = doc.replace('\n', " ");
            out.push_str(&format!("# @tribal: {oneline}\n"));
        }
        out.push('\n');
    }

    out.push_str("# b00t:map v1\n");
    out.push_str("# summary: l3dg3rr function documentation export\n");
    out.push_str("# tags: l3dg3rr, docgen, auto-export\n");
    out.push_str("# tier: sm0l\n");
    out.push_str("# cmds: b00t-cli docgen --format=tomllm\n");
    out.push_str("# complexity: 3\n");
    out
}

fn format_rustdoc(functions: &[Value]) -> String {
    let mut out = String::new();
    out.push_str("// l3dg3rr docgen export — rustdoc style\n\n");

    for f in functions {
        let name = f.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
        let sig = f.get("signature").and_then(|v| v.as_str()).unwrap_or("");
        let file = f.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
        let line = f.get("start_line").and_then(|v| v.as_u64()).unwrap_or(0);
        let doc = f.get("docstring").and_then(|v| v.as_str()).unwrap_or("");

        let sig_display = if sig.is_empty() { format!("{name}()") } else { sig.to_string() };
        out.push_str(&format!("/// `{sig_display}`\n"));
        for doc_line in doc.lines() {
            out.push_str(&format!("/// {doc_line}\n"));
        }
        out.push_str("///\n");
        out.push_str("/// # Examples\n");
        out.push_str("/// ```no_run\n");
        out.push_str(&format!("/// // TODO: add usage example for {name}\n"));
        out.push_str("/// ```\n");
        out.push_str("/// # Safety\n");
        out.push_str(&format!("/// Review source at {file}:{line}\n"));
        out.push_str(&format!("fn {name}(...); // stub\n\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_format_tomllm_basic() {
        let functions = vec![
            json!({"name": "start_server", "qualified_name": "http::start_server", "file_path": "src/http.c",
                   "signature": "int start_server(int port)", "return_type": "int", "complexity": 5,
                   "docstring": "Start the HTTP server on given port"}),
        ];
        let output = format_tomllm(&functions);
        assert!(output.contains("start_server"));
        assert!(output.contains("http::start_server"));
        assert!(output.contains("# @tribal:"));
        assert!(output.contains("# b00t:map v1"));
    }

    #[test]
    fn test_format_rustdoc_basic() {
        let functions = vec![
            json!({"name": "handle_request", "file_path": "src/http.c", "start_line": 42,
                   "signature": "void handle_request(Request* req)", "docstring": "Handle incoming HTTP request"}),
        ];
        let output = format_rustdoc(&functions);
        assert!(output.contains("handle_request"));
        assert!(output.contains("# Examples"));
        assert!(output.contains("# Safety"));
        assert!(output.contains("src/http.c:42"));
    }

    #[test]
    fn test_format_tomllm_empty() {
        let output = format_tomllm(&[]);
        assert!(!output.is_empty());
        assert!(output.contains("# b00t:map v1"));
    }

    #[test]
    fn test_format_rustdoc_no_docstring() {
        let functions = vec![
            json!({"name": "no_docs_fn", "file_path": "mod.rs", "start_line": 10}),
        ];
        let output = format_rustdoc(&functions);
        assert!(output.contains("no_docs_fn"));
        // Should not crash on missing fields
    }

    #[test]
    fn test_docgen_format_parsing() {
        assert_eq!("tomllm".parse::<DocgenFormat>().unwrap(), DocgenFormat::Tomllm);
        assert_eq!("toml".parse::<DocgenFormat>().unwrap(), DocgenFormat::Tomllm);
        assert_eq!("rustdoc".parse::<DocgenFormat>().unwrap(), DocgenFormat::Rustdoc);
        assert_eq!("json".parse::<DocgenFormat>().unwrap(), DocgenFormat::Json);
        assert!("xyz".parse::<DocgenFormat>().is_err());
    }
}
