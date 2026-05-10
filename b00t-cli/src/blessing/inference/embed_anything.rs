// b00t-cli/src/blessing/inference/embed_anything.rs
// b00t-embed backend: Wraps b00t_embed::EmbedBackend for unified embedding
// Layer composition: runtime activation of OCI-style embedding layers via
//   b00t_embed::layer::{LayerStack, TensorRegistry, LayerGateKeeper}
//   Bouncer-gated lifecycle: validate_pre_load → tensor swap → verify_post_swap

use super::{Embedding, LLMInference, ModelInfo};
use anyhow::{Context, Result};
use async_trait::async_trait;
use b00t_embed::layer::stack::LayerStack;
use b00t_embed::layer::TensorSource as _;
use b00t_embed::EmbedBackend;

use std::sync::Arc;
use tokio::sync::RwLock;

/// EmbedAnything backend with OCI-style layer composition support.
///
/// Architecture (OCI container layer model):
///   Base model = frozen transformer body (loaded via embed_anything)
///   Layers     = swappable embedding head tensors (GGUF/safetensors bundles)
///   Stack      = ordered merge of base + active layers (like container rootfs)
///   Bouncer    = gatekeeper validating each lifecycle transition
///
pub struct EmbedAnythingBackend {
    /// Inner b00t-embed concrete backend
    inner: b00t_embed::EmbedAnythingBackend,
    /// Cached model metadata
    model_info: ModelInfo,
    /// Tensor registry + layer stack for runtime head swapping
    layer_stack: Arc<RwLock<Option<LayerStack>>>,
    /// Active layer IDs
    active_layers: Arc<RwLock<Vec<String>>>,
}

impl EmbedAnythingBackend {
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

        // Initialize layer stack with default VarMap-backed tensor registry
        let stack = LayerStack::new_with_defaults(vec!["bert", "jina", "qwen3"]);

        Ok(Self {
            inner,
            model_info,
            layer_stack: Arc::new(RwLock::new(Some(stack))),
            active_layers: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Register a layer from a TensorSource directly.
    pub async fn register_source(&self, source: Box<dyn b00t_embed::layer::TensorSource>) {
        let mut guard = self.layer_stack.write().await;
        if let Some(stack) = guard.as_mut() {
            stack.register_source(source);
        }
    }

    pub async fn layer_count(&self) -> usize {
        self.layer_stack
            .read()
            .await
            .as_ref()
            .map(|s| s.layer_count())
            .unwrap_or(0)
    }
}

#[async_trait]
impl LLMInference for EmbedAnythingBackend {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let b00t_embed::Embedding { data } = self
            .inner
            .embed(text)
            .await
            .context("b00t-embed inference failed")?;
        Ok(Embedding { data })
    }

    /// Compose layers by resolving blessing_ids → embedding layers.
    ///
    /// Each blessing_id maps to a registered layer. The compose operation:
    ///   1. Converts blessing IDs to a composite query embedding
    ///   2. Scores registered layers by relevance
    ///   3. Activates top-k layers (bouncer-gated)
    ///   4. Updates the active layer set
    ///
    /// This is the OCI container "merge" operation for embedding heads.
    async fn compose_layers(&mut self, blessing_ids: &[&str]) -> Result<()> {
        let mut guard = self.layer_stack.write().await;
        let stack = guard
            .as_mut()
            .context("layer stack not initialized")?;

        if blessing_ids.is_empty() {
            // No layers to compose — deactivate all
            let active = self.active_layers.read().await.clone();
            for id in &active {
                stack
                    .deactivate_layer(&id.as_str().into())
                    .await
                    .context("layer deactivation failed")?;
            }
            self.active_layers.write().await.clear();
            return Ok(());
        }

        // Build a query embedding from blessing IDs
        let query_text = blessing_ids.join(" ");
        let query_embed = self
            .inner
            .embed(&query_text)
            .await
            .context("query embedding for layer composition failed")?;
        let query_embedding = b00t_embed::Embedding {
            data: query_embed.data.clone(),
        };

        // Compose layers by relevance to blessing context
        let descriptors = stack
            .compose(&query_embedding, blessing_ids.len())
            .await
            .context("layer composition failed")?;

        let mut active = self.active_layers.write().await;
        active.clear();
        for d in &descriptors {
            if matches!(d.status, b00t_embed::layer::LayerStatus::Active) {
                active.push(d.id.as_str().to_string());
            }
        }

        Ok(())
    }

    fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    fn model_info(&self) -> ModelInfo {
        self.model_info.clone()
    }

    fn clear_layers(&mut self) -> Result<()> {
        // Clear active layer tracking.
        // Actual tensor restoration happens lazily on next compose_layers().
        self.active_layers.blocking_write().clear();
        Ok(())
    }
}
