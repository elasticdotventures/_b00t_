// b00t-cli/src/blessing/inference/embed_anything.rs
// b00t-embed backend: Wraps b00t_embed::EmbedBackend for unified embedding
// Uses embed_anything under the hood for HuggingFace/ONNX/Cloud model support

use super::{Embedding, LLMInference, ModelInfo};
use anyhow::{Context, Result};
use async_trait::async_trait;
use b00t_embed::EmbedBackend;

/// EmbedAnything backend: Wraps b00t_embed's unified embedding adapter
/// Delegates to b00t_embed::EmbedAnythingBackend for model loading and inference
pub struct EmbedAnythingBackend {
    /// Inner b00t-embed concrete backend
    inner: b00t_embed::EmbedAnythingBackend,
    /// Cached model metadata
    model_info: ModelInfo,
}

impl EmbedAnythingBackend {
    /// Create new backend with default HuggingFace model
    /// Uses jinaai/jina-embeddings-v2-small-en for lightweight embedding
    /// Falls back gracefully if model loading fails
    pub async fn new() -> Result<Self> {
        let config = b00t_embed::EmbedConfig {
            provider: b00t_embed::EmbedProvider::HuggingFace {
                model_id: "jinaai/jina-embeddings-v2-small-en".into(),
                revision: None,
            },
            ..b00t_embed::EmbedConfig::default()
        };

        let inner = b00t_embed::EmbedAnythingBackend::new(config)
            .await
            .context("Failed to initialize b00t-embed backend")?;

        let dim = inner.embedding_dim();
        let model_id = inner.model_id().to_string();
        let available = inner.is_available();

        let model_info = ModelInfo {
            model_id,
            embedding_dim: dim as u32,
            backend_name: "embed_anything".to_string(),
            available,
        };

        Ok(Self { inner, model_info })
    }
}

#[async_trait]
impl LLMInference for EmbedAnythingBackend {
    /// Embed text via b00t-embed adapter
    /// Maps b00t_embed::Embedding → local Embedding type
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let b00t_embed::Embedding { data } = self
            .inner
            .embed(text)
            .await
            .context("b00t-embed inference failed")?;
        Ok(Embedding { data })
    }

    /// Compose layers: No-op for b00t-embed (stateless backend)
    async fn compose_layers(&mut self, _blessing_ids: &[&str]) -> Result<()> {
        Ok(())
    }

    /// Check if backend model loaded successfully
    fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    /// Return cached model metadata
    fn model_info(&self) -> ModelInfo {
        self.model_info.clone()
    }
}
