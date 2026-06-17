//! Load SPO triples from a live neumann namespace into the reasoning engine.

use super::{ReasoningEngine, ReasoningResult};
use crate::irontology_bridge::IrontologyBridgeClient;

impl ReasoningEngine {
    /// Load all triples from a neumann namespace and run both reasoning layers.
    pub async fn from_namespace(namespace: &str) -> anyhow::Result<ReasoningResult> {
        let client = IrontologyBridgeClient::new(namespace)?;
        let facts = client.query_triples(None, None).await?;
        let triples: Vec<(String, String, String)> = facts
            .into_iter()
            .map(|f| {
                let obj = match &f.object {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                (f.subject, f.predicate, obj)
            })
            .collect();
        Ok(Self::run(triples))
    }

    /// Load triples from multiple namespaces and merge before reasoning.
    pub async fn from_namespaces(namespaces: &[&str]) -> anyhow::Result<ReasoningResult> {
        let mut all = Vec::new();
        for ns in namespaces {
            let client = IrontologyBridgeClient::new(*ns)?;
            let facts = client.query_triples(None, None).await?;
            for f in facts {
                let obj = match &f.object {
                    serde_json::Value::String(s) => s.clone(),
                    v => v.to_string(),
                };
                all.push((f.subject, f.predicate, obj));
            }
        }
        Ok(Self::run(all))
    }
}
