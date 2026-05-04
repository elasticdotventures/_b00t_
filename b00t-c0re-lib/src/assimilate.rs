//! # Assimilate pipeline — indelible document-level attribution for grok knowledge
//!
//! Each chunk is wrapped in a `<container>` span with doc_id, chunk_id, position, signature.
//! A transaction log (append-only JSONL) enables replay after corruption.
//! Content-addressed dedup via blake3 hashing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ── DocumentId ────────────────────────────────────────────────────────────
// Content-addressed: blake3 hash of normalized content.
// Same document → same ID — enables dedup.

/// Compute a content-addressed DocumentId (blake3 hex) from bytes.
pub fn compute_doc_id(content: &[u8]) -> String {
    blake3::hash(content).to_hex().to_string()
}

// ── DocumentRecord ────────────────────────────────────────────────────────

/// A record representing an ingested document with its metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRecord {
    pub doc_id: String,
    pub source_url: Option<String>,
    pub source_path: Option<String>,
    pub title: Option<String>,
    pub content_length: u64,
    pub ingested_at: String,
    pub content_hash: String,
}

impl DocumentRecord {
    /// Create a new document record from content bytes.
    pub fn new(
        content: &[u8],
        source_url: Option<String>,
        source_path: Option<String>,
        title: Option<String>,
    ) -> Self {
        let hash = compute_doc_id(content);
        Self {
            doc_id: hash.clone(),
            source_url,
            source_path,
            title,
            content_length: content.len() as u64,
            ingested_at: chrono::Utc::now().to_rfc3339(),
            content_hash: hash,
        }
    }
}

// ── ChunkRecord with Container Span ───────────────────────────────────────

/// Generate a container span tag for a chunk.
///
/// Format:
/// `<container doc="{doc_id}" chunk="{chunk_id}" pos="{pos}/{total}" sig="{content_hash}">`
pub fn make_container_tag(
    doc_id: &str,
    chunk_id: &str,
    pos: u32,
    total: u32,
    content_hash: &str,
) -> String {
    format!(
        r#"<container doc="{}" chunk="{}" pos="{}/{}" sig="{}">"#,
        doc_id, chunk_id, pos, total, content_hash
    )
}

/// A record representing a single chunk of a document with its container span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRecord {
    pub chunk_id: String,
    pub doc_id: String,
    pub position: u32,
    pub total_chunks: u32,
    pub container: String,
    pub content: String,
    pub content_hash: String,
}

impl ChunkRecord {
    /// Create a new chunk record.
    pub fn new(doc_id: &str, content: &str, position: u32, total_chunks: u32) -> Self {
        let content_hash = blake3::hash(content.as_bytes()).to_hex().to_string();
        let chunk_id = format!("{}::chunk::{}", doc_id, position);
        let container =
            make_container_tag(doc_id, &chunk_id, position, total_chunks, &content_hash);
        Self {
            chunk_id,
            doc_id: doc_id.to_string(),
            position,
            total_chunks,
            container,
            content: content.to_string(),
            content_hash,
        }
    }

    /// Return the full container-wrapped text.
    pub fn container_text(&self) -> String {
        format!("{}\n{}\n</container>", self.container, self.content)
    }
}

// ── Transaction Log ───────────────────────────────────────────────────────

/// A single entry in the append-only transaction log.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionEntry {
    pub tx_id: String,
    pub action: String, // "ingest_doc" | "chunk" | "learn" | "verify"
    pub doc_id: String,
    pub chunk_id: Option<String>,
    pub timestamp: String,
    pub payload_hash: String,
}

/// Default transaction directory: `~/.b00t/transactions/`
pub fn default_tx_log_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".b00t")
        .join("transactions")
}

/// Get the daily log path: `~/.b00t/transactions/YYYY-MM-DD.jsonl`
pub fn daily_tx_path(tx_dir: &Path) -> PathBuf {
    let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
    tx_dir.join(format!("{}.jsonl", date))
}

/// Append a transaction entry to the daily log. Creates dir + file if missing.
pub fn append_transaction(tx_dir: &Path, entry: &TransactionEntry) -> Result<()> {
    std::fs::create_dir_all(tx_dir)?;
    let path = daily_tx_path(tx_dir);
    let json = serde_json::to_string(entry)?;
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    writeln!(file, "{}", json)?;
    Ok(())
}

/// Read all transactions from all log files in a directory, sorted by date.
pub fn read_all_transactions(tx_dir: &Path) -> Result<Vec<TransactionEntry>> {
    let mut entries = Vec::new();
    if !tx_dir.exists() {
        return Ok(entries);
    }
    let mut files: Vec<_> = std::fs::read_dir(tx_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
        .collect();
    files.sort_by_key(|e| e.file_name());

    for file in &files {
        let content = std::fs::read_to_string(file.path())?;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<TransactionEntry>(line) {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    // Skip corrupt lines — log warning
                    tracing::warn!(
                        "Corrupt tx log line in {}: {}",
                        file.path().display(),
                        e
                    );
                }
            }
        }
    }
    Ok(entries)
}

// ── Assimilate Pipeline ───────────────────────────────────────────────────

/// Configuration for the assimilate pipeline.
#[derive(Debug, Clone)]
pub struct AssimilateConfig {
    pub tx_dir: PathBuf,
    pub chunk_size: usize,    // bytes per chunk (default 4096)
    pub chunk_overlap: usize, // overlap between chunks (default 256)
}

impl Default for AssimilateConfig {
    fn default() -> Self {
        Self {
            tx_dir: default_tx_log_dir(),
            chunk_size: 4096,
            chunk_overlap: 256,
        }
    }
}

/// Chunk text content into overlapping segments, returning `ChunkRecord`s.
///
/// Uses a simple sliding-window approach. Future: ledgrrr semantic chunker.
pub fn chunk_text(content: &str, doc_id: &str, config: &AssimilateConfig) -> Vec<ChunkRecord> {
    if content.is_empty() {
        return vec![];
    }

    let chunk_size = config.chunk_size;
    let overlap = config.chunk_overlap.min(chunk_size / 2);
    let step = chunk_size - overlap;
    let total_chars = content.len();
    let total_chunks = if total_chars <= chunk_size {
        1
    } else {
        ((total_chars - chunk_size + step - 1) / step) + 1
    } as u32;

    let mut chunks = Vec::new();
    let mut pos = 0usize;
    let mut idx = 0u32;

    while pos < total_chars {
        let end = (pos + chunk_size).min(total_chars);
        let chunk_content = &content[pos..end];
        chunks.push(ChunkRecord::new(doc_id, chunk_content, idx, total_chunks));
        pos += step;
        idx += 1;
        if idx >= total_chunks {
            break;
        }
    }

    chunks
}

/// Register the grok assimilate datum in the irontology/raglite knowledgebase.
///
/// Can be called independently of the full pipeline to register a datum shape.
pub fn register_assimilate_datum(datum_json: &str) -> Result<()> {
    // Placeholder: future integration with irontology bridge
    // For now, just validate JSON and echo
    let _: serde_json::Value =
        serde_json::from_str(datum_json).context("Invalid datum JSON")?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_doc_id() {
        let id = compute_doc_id(b"hello world");
        assert_eq!(id.len(), 64);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_doc_id_is_content_addressed() {
        let id1 = compute_doc_id(b"same content");
        let id2 = compute_doc_id(b"same content");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_doc_id_differs_for_different_content() {
        let id1 = compute_doc_id(b"content a");
        let id2 = compute_doc_id(b"content b");
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_make_container_tag() {
        let tag = make_container_tag("abc123", "abc123::chunk::0", 0, 5, "def456");
        assert!(tag.contains(r#"doc="abc123""#));
        assert!(tag.contains(r#"chunk="abc123::chunk::0""#));
        assert!(tag.contains(r#"pos="0/5""#));
        assert!(tag.contains(r#"sig="def456""#));
        assert!(tag.starts_with("<container "));
    }

    #[test]
    fn test_chunk_record_creates_container() {
        let rec = ChunkRecord::new("doc1", "some text content", 0, 3);
        assert!(rec.container_text().contains("<container doc="));
        assert!(rec.container_text().contains("</container>"));
        assert!(rec.container_text().contains("some text content"));
    }

    #[test]
    fn test_chunk_text_no_overlap() {
        let config = AssimilateConfig {
            chunk_size: 10,
            chunk_overlap: 0,
            ..Default::default()
        };
        let chunks =
            chunk_text("Hello World This Is A Test Document For Chunking", "doc1", &config);
        assert!(chunks.len() >= 2);
        assert_eq!(chunks[0].doc_id, "doc1");
        assert_eq!(chunks[0].position, 0);
    }

    #[test]
    fn test_chunk_text_small_content() {
        let config = AssimilateConfig::default();
        let chunks = chunk_text("tiny", "doc1", &config);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].total_chunks, 1);
    }

    #[test]
    fn test_chunk_text_empty() {
        let config = AssimilateConfig::default();
        let chunks = chunk_text("", "doc1", &config);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_daily_tx_path() {
        let path = daily_tx_path(Path::new("/tmp/tx"));
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert!(filename.ends_with(".jsonl"));
        assert_eq!(filename.len(), 16); // YYYY-MM-DD.jsonl = 16 chars
    }

    #[test]
    fn test_append_and_read_transactions() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let entry = TransactionEntry {
            tx_id: "2026-05-04::0001".to_string(),
            action: "ingest_doc".to_string(),
            doc_id: "abc".to_string(),
            chunk_id: None,
            timestamp: "2026-05-04T00:00:00Z".to_string(),
            payload_hash: "def".to_string(),
        };
        append_transaction(dir.path(), &entry)?;
        let entries = read_all_transactions(dir.path())?;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].tx_id, "2026-05-04::0001");
        Ok(())
    }
}
