#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    #[error("embedding backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("embedding inference failed: {0}")]
    InferenceFailed(#[from] anyhow::Error),
    #[error("model {0} not loaded")]
    ModelNotLoaded(String),
    #[error("dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}
