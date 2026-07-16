use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use embed_anything::embeddings::embed::Embedder;

use crate::{EmbedBackend, EmbedConfig, Embedding, EmbedProvider};

pub struct EmbedAnythingBackend {
    embedder: Arc<Embedder>,
    config: EmbedConfig,
    dim: usize,
    model_id: String,
    available: bool,
}

impl EmbedAnythingBackend {
    pub async fn new(config: EmbedConfig) -> Result<Self> {
        let model_id = match &config.provider {
            EmbedProvider::HuggingFace { model_id, .. } => model_id.clone(),
            EmbedProvider::ONNX { model_id } => model_id.clone(),
            EmbedProvider::Cloud { model_id, .. } => model_id.clone(),
        };

        let embedder = match &config.provider {
            EmbedProvider::HuggingFace { model_id, revision } => {
                Embedder::from_pretrained_hf(model_id, revision.as_deref(), None, None)?
            }
            EmbedProvider::ONNX { model_id } => {
                Embedder::from_pretrained_onnx("bert", None, None, Some(model_id), None, None)?
            }
            EmbedProvider::Cloud {
                provider,
                model_id,
                api_key,
            } => Embedder::from_pretrained_cloud(provider, model_id, api_key.clone())?,
        };

        let sample: Vec<f32> = embedder
            .embed(&["ping"], Some(1), None)
            .await?
            .into_iter()
            .find_map(|r| r.to_dense().ok())
            .unwrap_or_default();
        let dim = sample.len();

        Ok(Self {
            embedder: Arc::new(embedder),
            config,
            dim,
            model_id,
            available: dim > 0,
        })
    }
}

#[async_trait]
impl EmbedBackend for EmbedAnythingBackend {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let results = self
            .embedder
            .embed(&[text], Some(1), None)
            .await?;
        let vec = results
            .into_iter()
            .next()
            .and_then(|r| r.to_dense().ok())
            .unwrap_or_default();
        Ok(Embedding { data: vec })
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let results = self
            .embedder
            .embed(texts, Some(self.config.batch_size), None)
            .await?;
        Ok(results
            .into_iter()
            .filter_map(|r| r.to_dense().ok())
            .map(|data| Embedding { data })
            .collect())
    }

    fn embedding_dim(&self) -> usize {
        self.dim
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn is_available(&self) -> bool {
        self.available
    }
}
