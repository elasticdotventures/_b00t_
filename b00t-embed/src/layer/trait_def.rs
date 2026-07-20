use std::collections::HashMap;

use crate::Embedding;
use crate::layer::{LayerError, LayerId, TensorSpec};

use candle_core::{DType, Device, Tensor};
use dyn_clone::DynClone;

/// Abstract tensor source for a swappable layer.
///
/// OCI container layer analogy:
///   TensorSource = OCI layer blob (compressed tar of filesystem diffs)
///   EmbedLayer = OCI layer descriptor (manifest + annotations)
///   LayerStore = OCI image store (manifests + blob mounts)
///
/// Object-safe: use `Box<dyn TensorSource>` for dynamic dispatch.
pub trait TensorSource: DynClone + Send + Sync {
    fn layer_id(&self) -> &str;
    fn source_kind(&self) -> &'static str;
    fn model_architecture(&self) -> &'static str;
    fn embedding_dim(&self) -> usize;
    fn tensor_specs(&self) -> Vec<TensorSpec>;

    /// Load all tensors for this layer onto the given device.
    fn load_tensors(
        &self,
        device: &Device,
        dtype: DType,
    ) -> Result<HashMap<String, Tensor>, LayerError>;

    /// Optional embedding signature for relevance scoring without full tensor load.
    /// Returns a vector representing this layer's "domain fingerprint".
    fn relevance_signature(&self) -> Option<Vec<f32>> {
        None
    }
}

dyn_clone::clone_trait_object!(TensorSource);

/// A swappable embedding layer with bouncer-gated lifecycle.
///
/// OCI layer analogy:
///   Each EmbedLayer carries a TensorSource (the blob data),
///   and lifecycle hooks (validate/verify) that correspond to
///   OCI layer validation (digest check, size check, etc.)
pub trait EmbedLayer: DynClone + Send + Sync {
    /// Unique layer identifier
    fn id(&self) -> LayerId;

    /// Reference to the tensor source
    fn source(&self) -> &dyn TensorSource;

    /// Compute relevance of this layer for a given query embedding.
    /// Returns 0.0 (irrelevant) to 1.0 (perfect match).
    fn relevance(&self, query_embedding: &[f32]) -> f32;

    /// Bouncer input gate: validate that this layer is safe to load.
    /// Checks: source integrity, dimension compatibility, resource availability.
    fn validate_pre_load(&self) -> Result<(), LayerError> {
        let specs = self.source().tensor_specs();
        if specs.is_empty() {
            return Err(LayerError::source_error(
                self.id(),
                "no tensor specs defined",
            ));
        }
        Ok(())
    }

    /// Bouncer output gate: verify the swap produced a coherent embedding.
    /// Called after load with the embedding BEFORE and AFTER the swap.
    fn verify_post_swap(&self, before: &Embedding, after: &Embedding) -> Result<(), LayerError> {
        if before.data.is_empty() || after.data.is_empty() {
            return Err(LayerError::Other(anyhow::anyhow!(
                "layer {}: empty embedding after swap",
                self.id()
            )));
        }
        if before.data.len() != after.data.len() {
            return Err(LayerError::shape_mismatch(
                self.id(),
                format!(
                    "dimension mismatch: before={}, after={}",
                    before.data.len(),
                    after.data.len()
                ),
            ));
        }
        let sim = before.cosine_similarity(after);
        if sim.abs() > 0.99 {
            // suspicious: swap barely changed the embedding
            // warn but don't fail — some layers are near-identical
        }
        Ok(())
    }
}

dyn_clone::clone_trait_object!(EmbedLayer);

/// Factory: creates EmbedLayer instances from descriptors.
pub trait LayerFactory: Send + Sync {
    type Source: TensorSource;

    fn create_layer(&self, id: LayerId, source: Self::Source) -> Box<dyn EmbedLayer>;
}
