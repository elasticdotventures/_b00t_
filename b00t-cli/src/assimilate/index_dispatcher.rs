//! Index dispatcher — hybrid trait + MCP bridge for vector/graph indexing.
//!
//! Abstracts the indexing target so assimilation can feed multiple backends:
//! - Native: grafeo (graph), raglite (RAG)
//! - MCP bridge: codebase-memory-mcp, or any MCP server with index_project tool

use crate::assimilate::crawl_engine::CrawledDoc;
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;

/// Report from an indexing operation.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IngestReport {
    pub target: String,
    pub docs_indexed: usize,
    pub concepts_indexed: usize,
    pub errors: Vec<String>,
}

impl fmt::Display for IngestReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} doc(s), {} concept(s)",
            self.target, self.docs_indexed, self.concepts_indexed
        )?;
        if !self.errors.is_empty() {
            write!(f, ", {} error(s)", self.errors.len())?;
        }
        Ok(())
    }
}

/// Trait for index targets — native or MCP-bridged.
pub trait IndexTarget: Send + Sync {
    fn name(&self) -> &str;
    fn ingest(&self, docs: &[CrawledDoc], topic: &str) -> Result<IngestReport>;
}

/// Dispatcher that fans out to multiple index targets.
pub struct IndexDispatcher {
    targets: Vec<Box<dyn IndexTarget>>,
}

impl IndexDispatcher {
    /// Discover available targets by name.
    /// Supported: "grafeo", "raglite", "codebase-memory", "codebase"
    pub fn discover(names: &[String]) -> Result<Self> {
        let mut targets: Vec<Box<dyn IndexTarget>> = Vec::new();

        for name in names {
            let normalized = name.to_lowercase();
            match normalized.as_str() {
                "grafeo" | "graph" => {
                    targets.push(Box::new(GrafeoTarget::new()));
                }
                "raglite" | "rag" => {
                    targets.push(Box::new(RagliteTarget::new()));
                }
                "codebase-memory" | "codebase" | "cb" | "codebase_memory" => {
                    targets.push(Box::new(McpBridgeTarget::new("codebase-memory")?));
                }
                other => {
                    // Try as MCP server name
                    targets.push(Box::new(McpBridgeTarget::new(other)?));
                }
            }
        }

        if targets.is_empty() {
            return Err(anyhow!("no index targets discovered"));
        }

        Ok(Self { targets })
    }

    /// Ingest documents into all targets.
    pub fn ingest(&self, docs: &[CrawledDoc], topic: &str) -> Result<String> {
        let reports: Vec<String> = self
            .targets
            .iter()
            .filter_map(|t| {
                match t.ingest(docs, topic) {
                    Ok(report) => Some(report.to_string()),
                    Err(e) => Some(format!("{}: ERROR: {e}", t.name())),
                }
            })
            .collect();

        Ok(reports.join("; "))
    }
}

// ── Native: Grafeo ──────────────────────────────────────────────────────────

pub struct GrafeoTarget;

impl GrafeoTarget {
    pub fn new() -> Self {
        Self
    }
}

impl IndexTarget for GrafeoTarget {
    fn name(&self) -> &str {
        "grafeo"
    }

    fn ingest(&self, docs: &[CrawledDoc], topic: &str) -> Result<IngestReport> {
        let mut report = IngestReport {
            target: self.name().to_string(),
            ..Default::default()
        };

        // Use b00t data fabric CLI to upsert concept nodes
        for doc in docs {
            report.docs_indexed += 1;
            for concept in &doc.extraction.concepts {
                let subject = format!("concept:{}", concept.name);
                let predicate = "b00t:hasTopic";
                let object = topic.to_string();

                // Shell out to b00t data fabric upsert
                let result = Command::new("b00t")
                    .args([
                        "data",
                        "fabric",
                        "upsert",
                        "--subject",
                        &subject,
                        "--predicate",
                        predicate,
                        "--object",
                        &object,
                        "--namespace",
                        "assimilate",
                    ])
                    .output();

                match result {
                    Ok(out) if out.status.success() => {
                        report.concepts_indexed += 1;
                    }
                    Ok(out) => {
                        report.errors.push(format!(
                            "grafeo upsert failed for '{subject}': {}",
                            String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("?")
                        ));
                    }
                    Err(e) => {
                        report.errors.push(format!("grafeo CLI error: {e}"));
                    }
                }
            }
        }

        Ok(report)
    }
}

// ── Native: Raglite ─────────────────────────────────────────────────────────

pub struct RagliteTarget;

impl RagliteTarget {
    pub fn new() -> Self {
        Self
    }
}

impl IndexTarget for RagliteTarget {
    fn name(&self) -> &str {
        "raglite"
    }

    fn ingest(&self, docs: &[CrawledDoc], topic: &str) -> Result<IngestReport> {
        let mut report = IngestReport {
            target: self.name().to_string(),
            ..Default::default()
        };

        for doc in docs {
            let content = &doc.content.text;
            let result = Command::new("b00t")
                .args([
                    "grok",
                    "learn",
                    "--topic",
                    topic,
                    "--rag",
                    "raglite",
                ])
                .stdin(std::process::Stdio::piped())
                .spawn();

            match result {
                Ok(mut child) => {
                    use std::io::Write;
                    if let Some(stdin) = &mut child.stdin {
                        let _ = stdin.write_all(content.as_bytes());
                    }
                    match child.wait() {
                        Ok(status) if status.success() => {
                            report.docs_indexed += 1;
                        }
                        Ok(status) => {
                            report.errors.push(format!(
                                "raglite exited {:?} for {topic}",
                                status.code()
                            ));
                        }
                        Err(e) => {
                            report.errors.push(format!("raglite wait error: {e}"));
                        }
                    }
                }
                Err(e) => {
                    report.errors.push(format!("raglite spawn error: {e}"));
                }
            }
        }

        Ok(report)
    }
}

// ── MCP Bridge ──────────────────────────────────────────────────────────────

/// MCP bridge target — communicates with external MCP servers like codebase-memory.
pub struct McpBridgeTarget {
    server_name: String,
    binary_path: Option<String>,
}

impl McpBridgeTarget {
    pub fn new(server_name: &str) -> Result<Self> {
        // Try to find the MCP server binary via b00t datum lookup
        let binary_path = Self::find_mcp_binary(server_name);

        if binary_path.is_none() {
            eprintln!("⚠️  MCP server '{server_name}' binary not found — will attempt lazy spawn");
        }

        Ok(Self {
            server_name: server_name.to_string(),
            binary_path,
        })
    }

    /// Find MCP server binary from datum config.
    fn find_mcp_binary(name: &str) -> Option<String> {
        // Check common locations
        let b00t_path = std::env::var("_B00T_Path")
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                format!("{home}/.b00t/_b00t_")
            });

        let datum_path = format!("{b00t_path}/{name}.mcp.toml");
        if let Ok(content) = std::fs::read_to_string(&datum_path) {
            // Parse the command from the TOML (simple extraction)
            for line in content.lines() {
                if line.trim_start().starts_with("command") {
                    if let Some(eq_pos) = line.find('=') {
                        let val = line[eq_pos + 1..].trim().trim_matches('"');
                        return Some(val.to_string());
                    }
                }
            }
        }
        None
    }
}

impl IndexTarget for McpBridgeTarget {
    fn name(&self) -> &str {
        &self.server_name
    }

    fn ingest(&self, docs: &[CrawledDoc], topic: &str) -> Result<IngestReport> {
        let mut report = IngestReport {
            target: self.name().to_string(),
            ..Default::default()
        };

        // For codebase-memory specifically, we write docs to temp files and call index_project
        if self.server_name.contains("codebase-memory") || self.server_name == "codebase" {
            // Write combined content to a temp directory
            let tmp_dir = std::env::temp_dir().join(format!("b00t-assimilate-{topic}"));
            std::fs::create_dir_all(&tmp_dir)
                .map_err(|e| anyhow!("create temp dir: {e}"))?;

            for (i, doc) in docs.iter().enumerate() {
                let file_path = tmp_dir.join(format!("{i:03}.md"));
                let content = format!(
                    "# {} (depth {})\n\nSource: {}\n\n{}\n\n## Concepts\n\n{}",
                    topic,
                    doc.depth,
                    doc.url,
                    doc.content.text,
                    doc.extraction
                        .concepts
                        .iter()
                        .map(|c| format!("- **{}**: {}", c.name, c.description))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
                std::fs::write(&file_path, &content)
                    .map_err(|e| anyhow!("write temp file: {e}"))?;
                report.docs_indexed += 1;
            }

            // Call codebase-memory-mcp index via b00t CLI or direct subprocess
            eprintln!("  → indexing into {} via temp dir {}", self.server_name, tmp_dir.display());

            // Try b00t ast-extract → codebase-memory pipeline
            let result = Command::new("b00t")
                .args(["ast-extract", "--path", &tmp_dir.display().to_string()])
                .output();

            match result {
                Ok(out) if out.status.success() => {
                    report.concepts_indexed = docs.iter().map(|d| d.extraction.concepts.len()).sum();
                }
                Ok(out) => {
                    report.errors.push(format!(
                        "ast-extract failed: {}",
                        String::from_utf8_lossy(&out.stderr).lines().next().unwrap_or("?")
                    ));
                }
                Err(e) => {
                    report.errors.push(format!("ast-extract spawn error: {e}"));
                }
            }
        } else {
            // Generic MCP: write content and let the user manually index
            report.errors.push(format!(
                "generic MCP target '{}' — write docs to temp and index manually",
                self.server_name
            ));
            report.docs_indexed = docs.len();
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ingest_report_display() {
        let report = IngestReport {
            target: "test".to_string(),
            docs_indexed: 5,
            concepts_indexed: 12,
            errors: vec![],
        };
        assert!(report.to_string().contains("5 doc(s)"));
        assert!(report.to_string().contains("12 concept(s)"));
    }

    #[test]
    fn test_ingest_report_with_errors() {
        let report = IngestReport {
            target: "test".to_string(),
            docs_indexed: 0,
            concepts_indexed: 0,
            errors: vec!["boom".to_string()],
        };
        assert!(report.to_string().contains("1 error(s)"));
    }

    #[test]
    fn test_dispatcher_rejects_empty() {
        let result = IndexDispatcher::discover(&[]);
        assert!(result.is_err());
    }
}
