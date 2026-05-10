#[derive(Debug, Clone)]
pub enum EmbedProvider {
    HuggingFace {
        model_id: String,
        revision: Option<String>,
    },
    ONNX {
        model_id: String,
    },
    Cloud {
        provider: String,
        model_id: String,
        api_key: Option<String>,
    },
}

impl Default for EmbedProvider {
    fn default() -> Self {
        Self::HuggingFace {
            model_id: "jinaai/jina-embeddings-v2-small-en".into(),
            revision: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct EmbedConfig {
    pub provider: EmbedProvider,
    pub batch_size: usize,
    pub chunk_size: usize,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        Self {
            provider: EmbedProvider::default(),
            batch_size: 32,
            chunk_size: 512,
        }
    }
}
