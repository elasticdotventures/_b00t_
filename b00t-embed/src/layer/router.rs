// P2: embed_anything search-driven layer activation.
// LayerRouter embeds search queries and matches against registered layer
// fingerprints via cosine similarity, then triggers compose().

use std::collections::HashMap;

use crate::layer::stack::LayerStack;
use crate::layer::trait_def::TensorSource;
use crate::layer::{LayerDescriptor, LayerStatus};
use crate::Embedding;

/// Router strategy: how query embeddings map to layer activations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RouterStrategy {
    /// Cosine similarity against each layer's fingerprint. Top-k activate.
    CosineTopK,
    /// Threshold-based: activate all layers above a similarity threshold.
    Threshold(f32),
}

impl Default for RouterStrategy {
    fn default() -> Self {
        Self::CosineTopK
    }
}

/// Routes search queries to embedding layers via fingerprint matching.
///
/// Architecture:
///   1. Search query → embed_anything → query embedding vector
///   2. Query vector → cosine sim → each registered layer's fingerprint
///   3. Top-k matches → LayerStack::compose() → VarMap swap
///   4. Model forward pass uses activated layer weights
pub struct LayerRouter {
    /// Layer stack for composition
    stack: LayerStack,
    /// Routing strategy
    strategy: RouterStrategy,
    /// Query embedding cache (query_hash → embedding)
    embed_cache: HashMap<u64, Embedding>,
}

impl LayerRouter {
    pub fn new(stack: LayerStack) -> Self {
        Self {
            stack,
            strategy: RouterStrategy::default(),
            embed_cache: HashMap::new(),
        }
    }

    pub fn with_strategy(mut self, strategy: RouterStrategy) -> Self {
        self.strategy = strategy;
        self
    }

    pub fn stack(&self) -> &LayerStack {
        &self.stack
    }

    /// Route a search query to the best matching layers.
    /// The query embedding is compared against each registered layer's
    /// domain fingerprint via cosine similarity. Top-k are composed.
    pub async fn route(&self, query_embedding: &Embedding, max_layers: usize) -> Vec<LayerDescriptor> {
        match self.strategy {
            RouterStrategy::CosineTopK | RouterStrategy::Threshold(_) => {
                self.stack.compose(query_embedding, max_layers).await
                    .unwrap_or_default()
            }
        }
    }

    /// Register a layer source with the router.
    pub fn register_source(&mut self, source: Box<dyn TensorSource>) {
        self.stack.register_source(source);
    }

    /// Clear the query embedding cache.
    pub fn clear_cache(&mut self) {
        self.embed_cache.clear();
    }
}
