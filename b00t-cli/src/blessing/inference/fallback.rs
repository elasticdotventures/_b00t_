// b00t-cli/src/blessing/inference/fallback.rs
// Ripgrep BM25 fallback: Keyword-based retrieval without vector embeddings
// Task 5: Always-available text search backend using ripgrep keyword matching
// Phase 8 markers: 🦨 for ripgrep integration tasks

use super::{Embedding, LLMInference, ModelInfo};
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// BM25 keyword search result from ripgrep backend
/// Contains blessing metadata and matched lines for keyword query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BM25SearchResult {
    /// Blessing identifier in knowledge index
    pub blessing_id: String,
    /// Relevance score: keyword match count / query words (0.0 to 1.0)
    pub relevance_score: f32,
    /// Lines from blessing that matched query keywords
    pub matched_lines: Vec<String>,
}

/// Ripgrep-based BM25 fallback implementation
/// Uses keyword search when vector embeddings unavailable
/// Always available (no GPU/CPU requirements)
pub struct RipgrepBM25 {
    /// Core model metadata: model_id, embedding_dim, backend_name, available flag
    model_info: ModelInfo,
    /// Path to knowledge index directory containing .toml blessing files
    index_dir: PathBuf,
}

impl RipgrepBM25 {
    /// Create new BM25 fallback instance
    /// index_dir: location of knowledge_index (containing blessing .toml files)
    pub fn new() -> Self {
        Self::with_index_dir(PathBuf::from("/tmp/knowledge_index"))
    }

    /// Create BM25 instance with custom index directory
    pub fn with_index_dir(index_dir: PathBuf) -> Self {
        Self {
            model_info: ModelInfo {
                model_id: "ripgrep-bm25".to_string(),
                embedding_dim: 0, // BM25 doesn't use embeddings
                backend_name: "ripgrep".to_string(),
                available: true,
            },
            index_dir,
        }
    }

    /// Keyword-based search using ripgrep (stub for Phase 8)
    /// Searches knowledge_index for query keywords
    /// Returns BM25SearchResult with relevance scoring
    /// 🦨 TODO: Implement actual ripgrep command execution
    /// 🦨 TODO: Parse ripgrep output into BlessingSearchResult
    /// 🦨 TODO: BM25 scoring algorithm (keyword match count / query words)
    pub fn search(&self, _query: &str, _top_k: usize) -> Vec<BM25SearchResult> {
        // Stub: Returns empty results
        // Phase 8 will execute: ripgrep -i --files-with-matches <query> <index_dir>
        vec![]
    }

    /// Retrieve blessing from knowledge index by ID
    /// Reads .toml file from index_dir and returns content
    /// 🦨 TODO: Parse ripgrep output into BlessingSearchResult
    /// 🦨 TODO: Error handling for missing blessings
    pub fn retrieve_blessing(&self, _blessing_id: &str) -> Result<String> {
        // Stub: Returns empty string
        // Phase 8 will read: <index_dir>/<blessing_id>.toml
        Ok(String::new())
    }
}

impl Default for RipgrepBM25 {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMInference for RipgrepBM25 {
    /// Embed text into vector space
    /// BM25/Ripgrep doesn't use embeddings; returns zero-vector placeholder
    async fn embed(&self, _text: &str) -> Result<Embedding> {
        Ok(Embedding {
            data: vec![], // Empty vector: BM25 doesn't produce embeddings
        })
    }

    /// Compose multiple blessing layers into unified representation
    /// Ripgrep-based composition: search text directly without vector operations
    async fn compose_layers(&mut self, _blessing_ids: &[&str]) -> Result<()> {
        // No-op: Ripgrep is stateless
        Ok(())
    }

    /// Check if backend is available and ready to use
    /// Ripgrep is always available (it's a CLI tool with no GPU/CPU requirements)
    fn is_available(&self) -> bool {
        true
    }

    /// Get model metadata (ID, dimension, backend name)
    fn model_info(&self) -> ModelInfo {
        self.model_info.clone()
    }

    /// Clear cached layers and reset internal state
    /// No-op: Ripgrep is stateless (no caching)
    fn clear_layers(&mut self) -> Result<()> {
        Ok(())
    }
}
