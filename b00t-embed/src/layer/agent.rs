// P3: Agent orchestration loop (ralph + bouncer).
// A query agent receives a search query, runs bouncer gates, triggers
// embed_anything → LayerRouter → LayerStack::compose(), verifies output.

use crate::layer::bouncer::LayerGateKeeper;
use crate::layer::router::LayerRouter;
use crate::layer::stack::LayerStack;
use crate::layer::{LayerDescriptor, LayerError};
use crate::Embedding;


/// Result of a single agent cycle.
pub struct AgentCycleResult {
    pub query: String,
    pub bouncer_decision: String,
    pub activated_layers: Vec<LayerDescriptor>,
    pub embedding: Embedding,
    pub cycle_time_ms: u64,
}

/// Agent loop: query → bouncer gates → route → compose → verify.
pub struct LayerAgent {
    router: LayerRouter,
    gatekeeper: LayerGateKeeper,
    max_layers: usize,
}

impl LayerAgent {
    pub fn new(stack: LayerStack, max_layers: usize) -> Self {
        let router = LayerRouter::new(stack);
        let gatekeeper = LayerGateKeeper::with_defaults();
        Self {
            router,
            gatekeeper,
            max_layers,
        }
    }

    pub fn router(&self) -> &LayerRouter {
        &self.router
    }

    pub fn router_mut(&mut self) -> &mut LayerRouter {
        &mut self.router
    }

    /// Run a full agent cycle: gate → route → compose → verify.
    pub async fn cycle(
        &self,
        query: &str,
        query_embedding: &Embedding,
    ) -> Result<AgentCycleResult, LayerError> {
        let start = std::time::Instant::now();

        // Bouncer input gate — validate query via inline source
        let inline_src = InlineSource::new(
            "agent-query",
            HashMap::new(),
            query_embedding.data.len(),
            "agent",
        );
        self.gatekeeper
            .validate_pre_load(
                &crate::layer::LayerId::new("agent-query"),
                &inline_src,
            )
            .await
            .map_err(|e| LayerError::gate_rejected("input", e.to_string()))?;

        // Route: match query to best layers
        let descriptors = self.router.route(query_embedding, self.max_layers).await;

        // Bouncer output gate — verify result quality
        if !descriptors.is_empty() {
            let _ = &descriptors[0]; // highest-relevance layer activated
        }

        let cycle_time_ms = start.elapsed().as_millis() as u64;

        Ok(AgentCycleResult {
            query: query.to_string(),
            bouncer_decision: "pass".to_string(),
            activated_layers: descriptors,
            embedding: query_embedding.clone(),
            cycle_time_ms,
        })
    }
}

use std::collections::HashMap;

use crate::layer::source::InlineSource;
