// b00t-cli/src/blessing/inference/mod.rs
// LLM Inference abstraction layer with multiple backends (Candle, llama.cpp-rs, Ripgrep)
// 🤓 INCOSE V-model: Requirement → Design → Implementation → Validation
//    This module provides the interface (Design phase) for all inference backends

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Embedding: Vector of f32 with cosine similarity computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    /// Raw embedding vector (e.g., 384 dimensions for all-MiniLM-L6-v2)
    pub data: Vec<f32>,
}

impl Embedding {
    /// Compute cosine similarity with another embedding
    /// Returns value in [-1, 1], where 1.0 = identical, 0.0 = orthogonal
    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        if self.data.is_empty() || other.data.is_empty() {
            return 0.0;
        }

        let dot_product: f32 = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a * b)
            .sum();

        let self_magnitude = self.magnitude();
        let other_magnitude = other.magnitude();

        if self_magnitude == 0.0 || other_magnitude == 0.0 {
            return 0.0;
        }

        dot_product / (self_magnitude * other_magnitude)
    }

    /// Compute L2 norm (Euclidean magnitude) of embedding vector
    pub fn magnitude(&self) -> f32 {
        self.data.iter().map(|x| x * x).sum::<f32>().sqrt()
    }
}

/// Model metadata and availability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Hugging Face model ID (e.g., "all-MiniLM-L6-v2")
    pub model_id: String,
    /// Embedding dimension (e.g., 384 for all-MiniLM, 0 for BM25)
    pub embedding_dim: u32,
    /// Backend name ("candle", "llamacpp", "ripgrep")
    pub backend_name: String,
    /// Whether backend successfully initialized
    pub available: bool,
}

/// Configuration for inference backend selection and initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Base model ID for all embeddings
    pub base_model_id: String,
    /// Directory for knowledge index / vector database
    pub knowledge_index_dir: String,
    /// Prefer Candle if available (GPU-accelerated)
    pub prefer_candle: bool,
    /// Enable llama.cpp-rs fallback for CPU inference
    pub enable_llamacpp: bool,
}

/// LLMInference trait: Core abstraction for embedding and layering operations
/// All inference backends (Candle, llama.cpp-rs, Ripgrep) implement this trait
#[async_trait]
pub trait LLMInference: Send + Sync {
    /// Embed text into vector space
    /// Input: raw text (any length, backend handles truncation)
    /// Output: Embedding struct with normalized f32 vector
    async fn embed(&self, text: &str) -> Result<Embedding>;

    /// Compose multiple blessing layers into unified representation
    /// Input: slice of blessing IDs to retrieve from knowledge base
    /// Output: Unified vector representation or error
    /// 🤓 "Layers" = blessings with overlapping roles/capabilities
    ///    Composition = intersection/union of permission sets
    async fn compose_layers(&mut self, blessing_ids: &[&str]) -> Result<()>;

    /// Check if backend is available and ready to use
    fn is_available(&self) -> bool;

    /// Get model metadata (ID, dimension, backend name)
    fn model_info(&self) -> ModelInfo;

    /// Clear cached layers and reset internal state
    /// Default implementation: no-op (overridden by backends with caching)
    fn clear_layers(&mut self) -> Result<()> {
        Ok(())
    }
}

/// Backend selector enum: Wraps concrete implementations behind trait object
/// Each variant holds a boxed trait object pointing to backend implementation
#[derive(Clone)]
pub enum InferenceBackendSelector {
    /// Candle backend: Meta's Rust ML framework (GPU-accelerated with CUDA)
    /// Feature: "candle" in Cargo.toml
    Candle,

    /// llama.cpp-rs backend: CPU-optimized inference via llama.cpp wrapper
    /// Feature: "llamacpp-fallback" in Cargo.toml
    #[cfg(feature = "llamacpp-fallback")]
    LlamaCpp,

    /// Ripgrep BM25 fallback: Keyword-based retrieval (no embeddings)
    /// Always available, lowest quality but guaranteed to work
    Ripgrep,
}

impl std::fmt::Debug for InferenceBackendSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InferenceBackendSelector::Candle => write!(f, "Candle"),
            #[cfg(feature = "llamacpp-fallback")]
            InferenceBackendSelector::LlamaCpp => write!(f, "LlamaCpp"),
            InferenceBackendSelector::Ripgrep => write!(f, "Ripgrep"),
        }
    }
}

/// Select inference backend based on configuration and availability
/// Implements fallback chain: Candle → llama.cpp → Ripgrep
pub fn select_inference_backend(config: &InferenceConfig) -> InferenceBackendSelector {
    // Phase 1: Try preferred backend (Candle)
    if config.prefer_candle {
        // Task 3: Candle backend implementation will check actual GPU availability
        // For now, return selector variant
        return InferenceBackendSelector::Candle;
    }

    // Phase 2: Try llama.cpp if enabled
    #[cfg(feature = "llamacpp-fallback")]
    {
        if config.enable_llamacpp {
            // Task 4: llama.cpp backend implementation will verify library availability
            return InferenceBackendSelector::LlamaCpp;
        }
    }

    // Phase 3: Final fallback to ripgrep (always available)
    // Task 5: Ripgrep fallback will use BM25 keyword search
    InferenceBackendSelector::Ripgrep
}

// Module declarations
// Task 3: Candle backend implementation
#[cfg(feature = "candle")]
pub mod candle;

// Task 4: llama.cpp-rs backend implementation (feature-gated)
#[cfg(feature = "llamacpp-fallback")]
pub mod llamacpp;

// Task 5: Ripgrep BM25 fallback implementation
pub mod fallback;

// Test suite
#[cfg(test)]
mod tests;
