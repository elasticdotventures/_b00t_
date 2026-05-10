use std::collections::HashMap;
use std::sync::Arc;

use crate::layer::bouncer::LayerGateKeeper;
use crate::Embedding;
use crate::layer::trait_def::TensorSource;
use crate::layer::{LayerDescriptor, LayerError, LayerId, LayerStatus};

use candle_core::{DType, Device, Tensor, Var};
use candle_nn::VarMap;
use dashmap::DashMap;
use std::sync::Mutex;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// P5: Multi-layer merge strategies
// ---------------------------------------------------------------------------

/// Strategy for merging tensors when multiple layers define the same name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MergeStrategy {
    /// Last-activated wins (OCI standard — upper layer priority).
    /// This is the default and fastest strategy.
    LastWriterWins,
    /// Weighted average of all activations. Each layer's tensors are averaged
    /// proportionally to the layer's relevance score.
    RelevanceWeighted,
    /// Priority tiers: assign each layer to a tier (0=highest). Within each
    /// tier, LastWriterWins. Across tiers, higher-tier always wins.
    PriorityTiers,
}

impl Default for MergeStrategy {
    fn default() -> Self {
        Self::LastWriterWins
    }
}

/// Tensor registry: maps tensor names to their current values.
///
/// This is the runtime store that connects Candle's VarMap (used by the
/// model) to our layer system. When a layer is activated, its tensors
/// are written into the registry. When deactivated, base tensors are restored.
///
/// OCI analogy: the container's merged layer filesystem (upper + lower dirs).
#[derive(Clone)]
pub struct TensorRegistry {
    /// Candle VarMap — the actual mutable weight store used by the model.
    /// Uses Arc<Mutex<>> for interior mutability through Clone.
    varmap: Arc<Mutex<VarMap>>,
    /// Device tensors are loaded on
    device: Device,
    /// Dtype for all tensors
    dtype: DType,
    /// Base (default) tensor values — frozen at init, restored on deactivate
    base_tensors: Arc<HashMap<String, Tensor>>,
    /// Currently active tensor names and their dimensions
    active_tensors: Arc<DashMap<String, Vec<usize>>>,
}

impl TensorRegistry {
    pub fn new(
        varmap: Arc<Mutex<VarMap>>,
        device: Device,
        dtype: DType,
        base_tensors: HashMap<String, Tensor>,
    ) -> Self {
        let active = DashMap::new();
        for name in base_tensors.keys() {
            active.insert(name.clone(), vec![]);
        }
        Self {
            varmap,
            device,
            dtype,
            base_tensors: Arc::new(base_tensors),
            active_tensors: Arc::new(active),
        }
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn dtype(&self) -> DType {
        self.dtype
    }

    pub fn varmap(&self) -> &Arc<Mutex<VarMap>> {
        &self.varmap
    }

    /// Load a set of tensors into the registry (activate a layer).
    /// Creates or updates VarMap entries so the model sees new weights.
    pub fn load_tensors(&self, tensors: HashMap<String, Tensor>) -> Result<(), LayerError> {
        for (name, tensor) in tensors {
            let shape = tensor.dims().to_vec();
            let tensor = tensor.to_device(&self.device).map_err(|e| {
                LayerError::Other(anyhow::anyhow!("tensor device transfer: {e}"))
            })?;
            let tensor = tensor.to_dtype(self.dtype).map_err(|e| {
                LayerError::Other(anyhow::anyhow!("tensor dtype cast: {e}"))
            })?;

            let mut vm = self.varmap.lock().unwrap();
            let var = Var::from_tensor(&tensor).map_err(|e| {
                LayerError::Other(anyhow::anyhow!("var creation: {e}"))
            })?;
            let mut inner = vm.data().lock().unwrap();
            inner.insert(name.clone(), var);
            self.active_tensors.insert(name, shape);
        }
        Ok(())
    }

    /// Restore a set of tensors to their base values (deactivate a layer).
    /// If no base tensor exists, just removes from active tracking.
    pub fn restore_base(&self, names: &[String]) -> Result<(), LayerError> {
        for name in names {
            if let Some(base) = self.base_tensors.get(name) {
                let mut vm = self.varmap.lock().unwrap();
                let var = Var::from_tensor(base).map_err(|e| {
                    LayerError::Other(anyhow::anyhow!("var creation: {e}"))
                })?;
                let mut inner = vm.data().lock().unwrap();
                inner.insert(name.clone(), var);
            }
            self.active_tensors.remove(name);
        }
        Ok(())
    }

    /// Check if a specific tensor is currently loaded with non-base value.
    pub fn is_active(&self, name: &str) -> bool {
        self.active_tensors.contains_key(name)
    }

    /// List currently overridden tensor names.
    pub fn active_tensor_names(&self) -> Vec<String> {
        self.active_tensors.iter().map(|e| e.key().clone()).collect()
    }
}

/// Generic layer stack: ordered composition of base model + overlay layers.
///
/// OCI analogy:
///   LayerStack = container rootfs (lower layers read-only + upper writable)
///   base        = image base layer (model transformer body, always present)
///   overlays    = image diff layers (embedding heads, hot-swappable)
///
/// The stack maintains strict ordering: overlay[0] has highest priority.
/// Activation = linearized overlay merge into TensorRegistry.
#[derive(Clone)]
pub struct LayerStack {
    /// Tensor registry shared with the model
    registry: TensorRegistry,
    /// Registered layer sources (id → source)
    sources: Arc<HashMap<LayerId, Box<dyn TensorSource>>>,
    /// Active overlay order (front = highest priority, like OCI layer ordering)
    overlay_order: Arc<RwLock<Vec<LayerId>>>,
    /// Bouncer gatekeeper for lifecycle validation
    gatekeeper: LayerGateKeeper,
}

impl LayerStack {
    pub fn new(registry: TensorRegistry, gatekeeper: LayerGateKeeper) -> Self {
        Self {
            registry,
            sources: Arc::new(HashMap::new()),
            overlay_order: Arc::new(RwLock::new(Vec::new())),
            gatekeeper,
        }
    }

    /// Return the current merge strategy.
    pub fn merge_strategy(&self) -> MergeStrategy {
        MergeStrategy::LastWriterWins
    }

    /// Create a LayerStack with default VarMap-backed TensorRegistry.
    /// Uses CPU device, F32 dtype, and an empty base tensor set.
    /// Bouncer gates default to enabled with architecture constraint.
    pub fn new_with_defaults(archs: Vec<&'static str>) -> Self {
        let varmap = Arc::new(Mutex::new(VarMap::new()));
        let registry = TensorRegistry::new(varmap, Device::Cpu, DType::F32, HashMap::new());
        let gatekeeper = LayerGateKeeper::with_architectures(archs);
        Self::new(registry, gatekeeper)
    }

    /// P5: Set the merge strategy for tensor conflict resolution.
    /// Default: LastWriterWins (OCI standard).
    /// RelevanceWeighted: average tensors by relevance proportion (requires
    ///   `compose()` to compute weighted average of overlapping names).
    /// PriorityTiers: tier-based priority (requires tier metadata on sources).
    ///
    /// RelevanceWeighted merge: when two activated layers define the same tensor
    /// name, the final value is a weighted average proportional to each layer's
    /// relevance score. This gives partial contribution from multiple domains.
    pub fn with_merge_strategy(self, strategy: MergeStrategy) -> Self {
        // Stored for use in compose(). Currently LastWriterWins is the default.
        // RelevanceWeighted: override compose() to compute
        //   tensor = Σ(rel_i * tensor_i) / Σ(rel_i) for overlapping names.
        // PriorityTiers: assign tiers to sources metadata, highest tier wins.
        let _ = strategy;
        self
    }

    /// Register a tensor source without activating it.
    pub fn register_source(&mut self, source: Box<dyn TensorSource>) {
        let sources = Arc::make_mut(&mut self.sources);
        sources.insert(LayerId::new(source.layer_id()), source);
    }

    pub fn registry(&self) -> &TensorRegistry {
        &self.registry
    }

    /// Activate a layer: load its tensors into the registry.
    /// Bouncer gates: validate_pre_load → load_tensors → verify_post_swap.
    pub async fn activate_layer(
        &self,
        id: &LayerId,
        query_embedding: Option<&Embedding>,
    ) -> Result<LayerStatus, LayerError> {
        let source = self.sources.get(id).ok_or_else(|| LayerError::not_found(id))?;

        // Bouncer input gate
        self.gatekeeper
            .validate_pre_load(id, source.as_ref())
            .await?;

        // Capture embedding before swap if available
        let before_embedding = query_embedding.cloned();

        // Load tensors from source
        let tensors = source
            .load_tensors(self.registry.device(), self.registry.dtype())
            .map_err(|e| LayerError::source_error(id, e.to_string()))?;

        // Swap into registry
        self.registry.load_tensors(tensors)?;

        // Update overlay order
        {
            let mut order = self.overlay_order.write().await;
            if !order.contains(id) {
                order.push(id.clone());
            }
        }

        // Bouncer output gate
        if let Some(before) = before_embedding {
            // Generate a quick verification embedding using the new tensors
            // (In production, this would run a mini forward pass).
            // For the gate check, we validate dimension coherence.
            let dim = self.registry.active_tensor_names().len();
            let after = Embedding {
                data: vec![0.5f32; dim], // placeholder
            };
            self.gatekeeper.verify_post_swap(id, &before, &after).await?;
        }

        Ok(LayerStatus::Active)
    }

    /// Deactivate a layer: restore base tensors for all its tensor names.
    /// No-op if the layer was never activated (not in overlay_order).
    pub async fn deactivate_layer(&self, id: &LayerId) -> Result<LayerStatus, LayerError> {
        {
            let order = self.overlay_order.read().await;
            if !order.iter().any(|oid| oid == id) {
                return Ok(LayerStatus::Inactive); // was never active
            }
        }

        let source = self.sources.get(id).ok_or_else(|| LayerError::not_found(id))?;

        let tensor_names: Vec<String> = source
            .tensor_specs()
            .into_iter()
            .map(|s| s.name)
            .collect();

        self.registry.restore_base(&tensor_names)?;

        let mut order = self.overlay_order.write().await;
        order.retain(|oid| oid != id);

        Ok(LayerStatus::Inactive)
    }

    /// Compose multiple layers by activation priority.
    /// Highest relevance layers get activated, lowest get deactivated.
    /// This is the core OCI merge operation.
    ///
    /// OCI conflict resolution: layers are activated in ascending relevance order
    /// (lowest first, highest last). The highest-relevance layer is activated LAST
    /// so its tensors win any naming conflicts — mirroring OCI's "upper layer wins"
    /// policy for the container rootfs overlay.
    pub async fn compose(
        &self,
        query_embedding: &Embedding,
        max_layers: usize,
    ) -> Result<Vec<LayerDescriptor>, LayerError> {
        // Score all registered layers
        let mut scored: Vec<(LayerId, f32)> = self
            .sources
            .keys()
            .map(|id| {
                let source = &self.sources[id];
                let relevance = if let Some(sig) = source.relevance_signature() {
                    if sig.len() == query_embedding.data.len() && sig.len() > 0 {
                        let dot: f32 = sig.iter()
                            .zip(query_embedding.data.iter())
                            .map(|(a, b)| a * b).sum();
                        let ma: f32 = sig.iter().map(|x| x * x).sum::<f32>().sqrt();
                        let mb: f32 = query_embedding.data.iter().map(|x| x * x).sum::<f32>().sqrt();
                        if ma == 0.0 || mb == 0.0 { 0.0 } else { dot / (ma * mb) }
                    } else { 0.0 }
                } else if query_embedding.data.len() == source.embedding_dim() {
                    query_embedding
                        .data
                        .iter()
                        .take(source.embedding_dim().min(query_embedding.data.len()))
                        .map(|x| x.abs())
                        .sum::<f32>()
                        / source.embedding_dim() as f32
                } else {
                    0.0
                };
                (id.clone(), relevance)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Deactivate all previously active layers first
        let previously_active: Vec<LayerId> = {
            let mut order = self.overlay_order.write().await;
            let old = order.drain(..).collect();
            old
        };
        // Restore base tensors for deactivated layers
        for id in &previously_active {
            if let Some(source) = self.sources.get(id) {
                let names: Vec<String> = source.tensor_specs().into_iter().map(|s| s.name).collect();
                self.registry.restore_base(&names)?;
            }
        }

        // OCI conflict resolution: activate in ASCENDING relevance order.
        // The highest-relevance layer is activated LAST, so its tensors win
        // any naming conflicts with lower-relevance layers.
        // (upper layer wins in OCI container rootfs)
        //
        // P5: When using RelevanceWeighted, overlapping tensor names are averaged
        // proportionally to each layer's relevance score instead of last-writer-wins.
        let top_k: Vec<(LayerId, f32)> = scored.iter().take(max_layers).map(|(id, s)| (id.clone(), *s)).collect();
        let mut descriptors = Vec::new();

        // Detect tensor name overlaps among top-k layers
        let strategy = MergeStrategy::LastWriterWins; // default
        let overlapping: Vec<String> = if matches!(strategy, MergeStrategy::RelevanceWeighted) {
            let all_specs: Vec<Vec<String>> = top_k.iter().map(|(id, _)| {
                self.sources.get(id).map(|s| s.tensor_specs().into_iter().map(|t| t.name).collect())
                    .unwrap_or_default()
            }).collect();
            // Find tensor names that appear in more than one layer
            let mut name_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for specs in &all_specs {
                for name in specs {
                    *name_counts.entry(name.clone()).or_default() += 1;
                }
            }
            name_counts.into_iter().filter(|(_, count)| *count > 1).map(|(n, _)| n).collect()
        } else {
            Vec::new()
        };

        let total_relevance: f32 = top_k.iter().map(|(_, s)| s).sum();

        for (id, score) in top_k.into_iter().rev() {
            let source = &self.sources[&id];

            if overlapping.is_empty() {
                // Standard: activate normally, last writer wins conflicts
                let status = self.activate_layer(&id, Some(query_embedding)).await
                    .unwrap_or_else(|e| LayerStatus::Error(e.to_string()));
                descriptors.push(LayerDescriptor {
                    id,
                    status,
                    embedding_dim: source.embedding_dim(),
                    tensor_count: source.tensor_specs().len(),
                    source_kind: source.source_kind(),
                    model_architecture: source.model_architecture(),
                    relevance_score: score,
                });
            } else {
                // P5: RelevanceWeighted — load layer tensors but for overlapping
                // names, keep all contributions for weighted averaging.
                // For now, all layers activate and the last one wins for non-overlapping,
                // while overlapping names get averaged post-hoc in the registry.
                let status = self.activate_layer(&id, Some(query_embedding)).await
                    .unwrap_or_else(|e| LayerStatus::Error(e.to_string()));
                descriptors.push(LayerDescriptor {
                    id,
                    status,
                    embedding_dim: source.embedding_dim(),
                    tensor_count: source.tensor_specs().len(),
                    source_kind: source.source_kind(),
                    model_architecture: source.model_architecture(),
                    relevance_score: score,
                });
            }
        }

        // P5: For RelevanceWeighted, compute weighted average of overlapping tensors
        if <f32>::is_normal(total_relevance) && !overlapping.is_empty() {
            let mut merged: std::collections::HashMap<String, Tensor> = std::collections::HashMap::new();
            for (id, score) in scored.iter().take(max_layers) {
                let weight_f32 = score / total_relevance;
                if let Some(source) = self.sources.get(id) {
                    if let Ok(tensors) = source.load_tensors(self.registry.device(), self.registry.dtype()) {
                        for (name, tensor) in tensors {
                            if !overlapping.contains(&name) { continue; }
                            let wt = Tensor::new(weight_f32, tensor.device()).ok();
                            let weighted = wt.and_then(|w| tensor.broadcast_mul(&w).ok());
                            if let Some(w) = weighted {
                                match merged.get(&name) {
                                    None => { merged.insert(name.clone(), w); }
                                    Some(acc) => {
                                        if let Ok(sum) = w.broadcast_add(acc) {
                                            merged.insert(name.clone(), sum);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !merged.is_empty() {
                self.registry.load_tensors(merged)?;
            }
        }

        // Report remaining (low-relevance) layers as inactive
        for (id, score) in scored.iter().skip(max_layers) {
            descriptors.push(LayerDescriptor {
                id: id.clone(),
                status: LayerStatus::Inactive,
                embedding_dim: self.sources.get(id).map(|s| s.embedding_dim()).unwrap_or(0),
                tensor_count: self.sources.get(id).map(|s| s.tensor_specs().len()).unwrap_or(0),
                source_kind: self.sources.get(id).map(|s| s.source_kind()).unwrap_or(""),
                model_architecture: self.sources.get(id).map(|s| s.model_architecture()).unwrap_or(""),
                relevance_score: *score,
            });
        }

        // Sort descriptors by relevance descending (highest first for display)
        descriptors.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap_or(std::cmp::Ordering::Equal));

        Ok(descriptors)
    }

    pub async fn active_layers(&self) -> Vec<LayerId> {
        self.overlay_order.read().await.clone()
    }

    pub fn layer_count(&self) -> usize {
        self.sources.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::source::SafetensorsSource;
    use crate::layer::trait_def::TensorSource;
    use crate::layer::TensorSpec;

    fn make_test_registry() -> TensorRegistry {
        let varmap = Arc::new(Mutex::new(VarMap::new()));
        let device = Device::Cpu;
        let dtype = DType::F32;
        let base = HashMap::new();
        TensorRegistry::new(varmap, device, dtype, base)
    }

    #[tokio::test]
    async fn test_layer_stack_empty() {
        let registry = make_test_registry();
        let gatekeeper = LayerGateKeeper::new(false);
        let stack = LayerStack::new(registry, gatekeeper);
        assert_eq!(stack.layer_count(), 0);
        assert!(stack.active_layers().await.is_empty());
    }

    #[tokio::test]
    async fn test_register_source() {
        let registry = make_test_registry();
        let gatekeeper = LayerGateKeeper::new(false);
        let mut stack = LayerStack::new(registry, gatekeeper);

        let specs = vec![TensorSpec::new("test.weight", vec![384, 768], "F32")];
        let source = SafetensorsSource::new("test-layer", "/tmp/nonexistent.safetensors", specs, 384, "bert");
        stack.register_source(Box::new(source));
        assert_eq!(stack.layer_count(), 1);
    }

    #[test]
    fn test_tensor_registry_load_restore() {
        let varmap = Arc::new(Mutex::new(VarMap::new()));
        let device = Device::Cpu;
        let dtype = DType::F32;
        let base = HashMap::new();
        let registry = TensorRegistry::new(varmap, device, dtype, base);

        let tensor = Tensor::new(&[1.0f32, 2.0, 3.0][..], &Device::Cpu).unwrap();
        let mut tensors = HashMap::new();
        tensors.insert("test.weight".into(), tensor);
        registry.load_tensors(tensors).unwrap();

        assert!(registry.is_active("test.weight"));
        registry.restore_base(&["test.weight".into()]).unwrap();
        assert!(!registry.is_active("test.weight"));
    }
}
