pub mod backends;
pub mod config;
pub mod error;
pub mod layer;
pub mod qwen3;

pub use backends::EmbedAnythingBackend;
pub use config::{EmbedConfig, EmbedProvider};
pub use error::EmbedError;

use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct Embedding {
    pub data: Vec<f32>,
}

impl Embedding {
    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        if self.data.is_empty() || other.data.is_empty() {
            return 0.0;
        }
        let dot: f32 = self.data.iter().zip(&other.data).map(|(a, b)| a * b).sum();
        let ma = self.data.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mb = other.data.iter().map(|x| x * x).sum::<f32>().sqrt();
        if ma == 0.0 || mb == 0.0 {
            0.0
        } else {
            dot / (ma * mb)
        }
    }
}

impl From<Vec<f32>> for Embedding {
    fn from(data: Vec<f32>) -> Self {
        Self { data }
    }
}

#[async_trait]
pub trait EmbedBackend: Send + Sync {
    async fn embed(&self, text: &str) -> Result<Embedding>;
    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>>;
    fn embedding_dim(&self) -> usize;
    fn model_id(&self) -> &str;
    fn is_available(&self) -> bool;

    /// Compose OCI-style layers: activate top-k by query relevance.
    /// Default no-op for backends without layer support.
    async fn compose_layers(
        &self,
        _query: &str,
        _max_layers: usize,
    ) -> Result<Vec<crate::layer::LayerDescriptor>> {
        Ok(Vec::new())
    }

    /// Clear all composed layers, restore base weights.
    fn clear_layers(&self) -> Result<()> {
        Ok(())
    }
}
