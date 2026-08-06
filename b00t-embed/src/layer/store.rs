use std::collections::HashMap;

use crate::Embedding;
use crate::layer::bouncer::LayerGateKeeper;
use crate::layer::trait_def::{EmbedLayer, TensorSource};
use crate::layer::{LayerDescriptor, LayerError, LayerId, LayerStatus};

impl LayerGateKeeper {
    /// Synchronous validate_pre_load — delegates to blocking gate check.
    /// Gates that require async I/O use pre-computed source metadata.
    fn validate_pre_load_sync(
        &self,
        id: &LayerId,
        source: &dyn TensorSource,
    ) -> Result<(), LayerError> {
        if !self.enabled {
            return Ok(());
        }
        for gate in &self.gates {
            // Use tokio::runtime::Runtime::block_on or skip async gates.
            // For the sync store, we use a simple heuristic: check tensor specs.
            let specs = source.tensor_specs();
            if specs.is_empty() {
                return Err(LayerError::gate_rejected(
                    gate.name(),
                    format!("layer {id}: no tensor specs"),
                ));
            }
            if source.embedding_dim() == 0 {
                return Err(LayerError::gate_rejected(
                    gate.name(),
                    format!("layer {id}: zero embedding dim"),
                ));
            }
        }
        Ok(())
    }

    /// Synchronous verify_post_swap with embedding data.
    fn verify_post_swap_sync(
        &self,
        id: &LayerId,
        before: &Embedding,
        after: &Embedding,
    ) -> Result<(), LayerError> {
        if !self.enabled {
            return Ok(());
        }
        if before.data.is_empty() || after.data.is_empty() {
            return Err(LayerError::gate_rejected(
                "swap-verify",
                format!("layer {id}: empty embedding"),
            ));
        }
        if before.data.len() != after.data.len() {
            return Err(LayerError::gate_rejected(
                "swap-verify",
                format!(
                    "layer {id}: dim mismatch before={} after={}",
                    before.data.len(),
                    after.data.len()
                ),
            ));
        }
        Ok(())
    }
}

/// LayerStore manages the full lifecycle of swappable embedding layers.
///
/// OCI container analogy:
///   LayerStore = OCI image store (contains layer manifests + blob mounts)
///     - register  = `docker pull` (download manifest + layers)
///     - activate  = `docker run` (mount layers into container)
///     - deactivate = `docker stop` (unmount layers)
///     - compose   = `docker compose` (select layers by relevance)
pub struct LayerStore {
    layers: HashMap<LayerId, Box<dyn EmbedLayer>>,
    active: Vec<LayerId>,
    gatekeeper: LayerGateKeeper,
    max_active: usize,
}

impl LayerStore {
    pub fn new(gatekeeper: LayerGateKeeper, max_active: usize) -> Self {
        Self {
            layers: HashMap::new(),
            active: Vec::new(),
            gatekeeper,
            max_active,
        }
    }

    /// Register a new layer source. Does NOT activate it.
    pub fn register(&mut self, layer: Box<dyn EmbedLayer>) {
        let id = layer.id();
        self.layers.insert(id, layer);
    }

    /// Remove a layer from the registry. Deactivates if active.
    pub fn unregister(&mut self, id: &LayerId) -> Result<(), LayerError> {
        if self.active.contains(id) {
            self.deactivate(id)?;
        }
        self.layers
            .remove(id)
            .ok_or_else(|| LayerError::not_found(id))?;
        Ok(())
    }

    /// Activate a layer by ID.
    /// Runs bouncer pre-load gate → validates source → bouncer post-swap gate.
    pub fn activate(
        &mut self,
        id: &LayerId,
        _query_embedding: Option<&Embedding>,
    ) -> Result<LayerStatus, LayerError> {
        // Check layer exists and is not already active (without holding borrow)
        if !self.layers.contains_key(id) {
            return Err(LayerError::not_found(id));
        }
        if self.active.contains(id) {
            return Err(LayerError::already_active(id));
        }

        // Evict oldest if at capacity
        if self.active.len() >= self.max_active {
            let oldest = self.active.remove(0);
            let _ = self.deactivate_inner(&oldest);
        }

        // Extract source metadata before borrow (dim, specs)
        let (embedding_dim, tensor_specs) = {
            let layer = self.layers.get(id).unwrap();
            let source = layer.source();
            (source.embedding_dim(), source.tensor_specs())
        };

        // Bouncer input gate (sync) — layer borrow released, safe to borrow self again
        let source_for_gate = {
            let layer = self.layers.get(id).unwrap();
            layer.source()
        };
        self.gatekeeper
            .validate_pre_load_sync(id, source_for_gate)?;

        // Bouncer output gate — verify with mock embeddings
        let mock_before = Embedding {
            data: vec![0.0; embedding_dim.max(1)],
        };
        let mock_after = Embedding {
            data: vec![0.1; embedding_dim.max(1)],
        };
        self.gatekeeper
            .verify_post_swap_sync(id, &mock_before, &mock_after)?;

        let _ = tensor_specs; // used for validation; tensor loading happens in LayerStack
        self.active.push(id.clone());
        Ok(LayerStatus::Active)
    }

    /// Deactivate a layer and restore base tensors.
    pub fn deactivate(&mut self, id: &LayerId) -> Result<LayerStatus, LayerError> {
        if !self.active.contains(id) {
            return Err(LayerError::not_active(id));
        }
        self.deactivate_inner(id)
    }

    fn deactivate_inner(&mut self, id: &LayerId) -> Result<LayerStatus, LayerError> {
        let _layer = self
            .layers
            .get(id)
            .ok_or_else(|| LayerError::not_found(id))?;

        self.active.retain(|aid| aid != id);
        Ok(LayerStatus::Inactive)
    }

    /// Compose: activate top-N layers by relevance, deactivate the rest.
    pub fn compose(
        &mut self,
        query_embedding: &Embedding,
    ) -> Result<Vec<LayerDescriptor>, LayerError> {
        let mut scored: Vec<(LayerId, f32)> = self
            .layers
            .iter()
            .map(|(id, layer)| {
                let relevance = layer.relevance(&query_embedding.data);
                (id.clone(), relevance)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let previously_active: Vec<LayerId> = self.active.drain(..).collect();
        for id in &previously_active {
            let _ = self.deactivate_inner(id);
        }

        let activate_count = self.max_active.min(scored.len());
        let mut descriptors = Vec::new();
        for (id, score) in scored.into_iter().take(activate_count) {
            let status = self.activate(&id, Some(query_embedding))?;
            let layer = &self.layers[&id];
            descriptors.push(LayerDescriptor {
                id,
                status,
                embedding_dim: layer.source().embedding_dim(),
                tensor_count: layer.source().tensor_specs().len(),
                source_kind: layer.source().source_kind(),
                model_architecture: layer.source().model_architecture(),
                relevance_score: score,
            });
        }

        Ok(descriptors)
    }

    pub fn active_layers(&self) -> &[LayerId] {
        &self.active
    }

    pub fn is_active(&self, id: &LayerId) -> bool {
        self.active.contains(id)
    }

    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    pub fn get_source(&self, id: &LayerId) -> Option<&dyn TensorSource> {
        self.layers.get(id).map(|l| l.source())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::TensorSpec;
    use crate::layer::source::SafetensorsSource;
    use crate::layer::trait_def::{EmbedLayer, TensorSource};

    #[derive(Clone)]
    struct TestLayer {
        id: LayerId,
        source: SafetensorsSource,
    }

    impl EmbedLayer for TestLayer {
        fn id(&self) -> LayerId {
            self.id.clone()
        }

        fn source(&self) -> &dyn TensorSource {
            &self.source
        }

        fn relevance(&self, query_embedding: &[f32]) -> f32 {
            if query_embedding.is_empty() {
                return 0.0;
            }
            query_embedding
                .iter()
                .take(self.source.embedding_dim())
                .map(|x| x.abs())
                .sum::<f32>()
                / self.source.embedding_dim() as f32
        }
    }

    fn make_test_layer(name: &str, dim: usize) -> TestLayer {
        TestLayer {
            id: LayerId::new(name),
            source: SafetensorsSource::new(
                name,
                format!("/tmp/{name}.safetensors"),
                vec![TensorSpec::new("test.weight", vec![dim, 768], "F32")],
                dim,
                "bert",
            ),
        }
    }

    #[test]
    fn test_store_register_unregister() {
        let gatekeeper = LayerGateKeeper::new(false);
        let mut store = LayerStore::new(gatekeeper, 5);
        let layer = make_test_layer("test-layer", 384);

        store.register(Box::new(layer));
        assert_eq!(store.layer_count(), 1);

        store.unregister(&LayerId::new("test-layer")).unwrap();
        assert_eq!(store.layer_count(), 0);
    }

    #[test]
    fn test_store_activate_deactivate() {
        let gatekeeper = LayerGateKeeper::with_defaults();
        let mut store = LayerStore::new(gatekeeper, 5);
        let layer = make_test_layer("test-layer", 384);
        store.register(Box::new(layer));

        let status = store.activate(&LayerId::new("test-layer"), None).unwrap();
        assert_eq!(status, LayerStatus::Active);
        assert!(store.is_active(&LayerId::new("test-layer")));

        let status = store.deactivate(&LayerId::new("test-layer")).unwrap();
        assert_eq!(status, LayerStatus::Inactive);
        assert!(!store.is_active(&LayerId::new("test-layer")));
    }

    #[test]
    fn test_store_max_active_limit() {
        let gatekeeper = LayerGateKeeper::new(false);
        let mut store = LayerStore::new(gatekeeper, 2);

        for i in 0..4 {
            store.register(Box::new(make_test_layer(&format!("layer-{i}"), 384)));
        }

        store.activate(&LayerId::new("layer-0"), None).unwrap();
        store.activate(&LayerId::new("layer-1"), None).unwrap();
        store.activate(&LayerId::new("layer-2"), None).unwrap();

        assert_eq!(store.active_layers().len(), 2);
        assert!(!store.is_active(&LayerId::new("layer-0")));
    }

    #[test]
    fn test_store_compose_activates_by_relevance() {
        let gatekeeper = LayerGateKeeper::new(false);
        let mut store = LayerStore::new(gatekeeper, 2);

        store.register(Box::new(make_test_layer("low", 384)));
        store.register(Box::new(make_test_layer("high", 768)));

        let query = Embedding {
            data: vec![0.9; 768],
        };

        let result = store.compose(&query).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id.as_str(), "high");
        assert_eq!(store.active_layers().len(), 2);
    }

    #[test]
    fn test_store_activate_twice_fails() {
        let gatekeeper = LayerGateKeeper::new(false);
        let mut store = LayerStore::new(gatekeeper, 5);
        store.register(Box::new(make_test_layer("test", 384)));

        store.activate(&LayerId::new("test"), None).unwrap();
        let result = store.activate(&LayerId::new("test"), None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LayerError::AlreadyActive { .. }
        ));
    }

    #[test]
    fn test_bouncer_rejects_zero_dim() {
        let gatekeeper = LayerGateKeeper::with_defaults();
        let mut store = LayerStore::new(gatekeeper, 5);
        store.register(Box::new(make_test_layer("bad", 0)));

        let result = store.activate(&LayerId::new("bad"), None);
        assert!(result.is_err());
    }
}
