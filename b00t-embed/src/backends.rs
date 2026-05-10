use std::sync::Arc;

use anyhow::Result;
use anyhow::anyhow;
use async_trait::async_trait;
use embed_anything::embeddings::embed::Embedder;

use crate::{EmbedBackend, EmbedConfig, EmbedProvider, Embedding};

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

        let sample_result = embedder
            .embed(&["ping"], Some(1), None)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("embedder returned no vectors for probe input"))?;
        let sample: Vec<f32> = sample_result
            .to_dense()
            .map_err(|e| anyhow!("failed to convert probe embedding to dense vector: {e}"))?;
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
        let results = self.embedder.embed(&[text], Some(1), None).await?;
        let vec = results
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("embedder returned no vectors for input text"))?
            .to_dense()
            .map_err(|e| anyhow!("failed to convert embedding to dense vector: {e}"))?;
        Ok(Embedding { data: vec })
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let results = self
            .embedder
            .embed(texts, Some(self.config.batch_size), None)
            .await?;
        if results.len() != texts.len() {
            return Err(anyhow!(
                "embedder returned {} vectors for {} inputs",
                results.len(),
                texts.len()
            ));
        }
        let mut out = Vec::with_capacity(results.len());
        for (idx, result) in results.into_iter().enumerate() {
            let data = result.to_dense().map_err(|e| {
                anyhow!("failed to convert batch embedding {idx} to dense vector: {e}")
            })?;
            out.push(Embedding { data });
        }
        Ok(out)
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
