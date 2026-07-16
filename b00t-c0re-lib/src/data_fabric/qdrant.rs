//! Optional Qdrant REST adapter for the data fabric.
//!
//! Enable with: `cargo build --features qdrant`.
//! ⚠️ SOUL.tomllm: qdrant.node_status = "optional-disabled" on this node.

use super::{
    EdgeRecord, FactRecord, KnowledgeStoreBackend, QueryResult, SemanticQuery, StoreConfig,
};
use anyhow::Result;

#[derive(Clone)]
pub struct QdrantStore {
    endpoint: String,
    collection: String,
    client: reqwest::Client,
}

impl QdrantStore {
    async fn ensure_collection(&self) -> Result<()> {
        let url = format!("{}/collections/{}", self.endpoint, self.collection);
        if self.client.get(&url).send().await?.status() == 404 {
            self.client
                .put(&url)
                .json(&serde_json::json!({"vectors":{"size":1536,"distance":"Cosine"}}))
                .send()
                .await?
                .error_for_status()?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl KnowledgeStoreBackend for QdrantStore {
    fn try_new(config: StoreConfig) -> Result<Self> {
        Ok(Self {
            endpoint: if config.endpoint.is_empty() {
                "http://localhost:6333".into()
            } else {
                config.endpoint
            },
            collection: config.namespace,
            client: reqwest::Client::new(),
        })
    }

    async fn query(&self, q: SemanticQuery) -> Result<QueryResult> {
        let url = format!(
            "{}/collections/{}/points/scroll",
            self.endpoint, self.collection
        );
        let mut must = vec![];
        if let Some(s) = &q.subject {
            must.push(serde_json::json!({"key":"subject","match":{"value":s}}));
        }
        if let Some(p) = &q.predicate {
            must.push(serde_json::json!({"key":"predicate","match":{"value":p}}));
        }
        let body = serde_json::json!({
            "limit": 100,
            "with_payload": true,
            "filter": if must.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!({"must": must})
            }
        });
        let json: serde_json::Value = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await?
            .json()
            .await?;
        let facts = json["result"]["points"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| {
                let pl = p.get("payload")?;
                Some(FactRecord {
                    subject: pl["subject"].as_str()?.to_string(),
                    predicate: pl["predicate"].as_str()?.to_string(),
                    object: pl["object"].clone(),
                })
            })
            .collect();
        Ok(QueryResult { facts })
    }

    async fn upsert_facts(&self, facts: Vec<FactRecord>) -> Result<()> {
        self.ensure_collection().await?;
        let pts: Vec<_> = facts
            .iter()
            .enumerate()
            .map(|(i, f)| {
                serde_json::json!({
                    "id": i,
                    "vector": vec![0.0f32; 1536],
                    "payload": {
                        "subject": f.subject,
                        "predicate": f.predicate,
                        "object": f.object
                    }
                })
            })
            .collect();
        self.client
            .put(format!(
                "{}/collections/{}/points",
                self.endpoint, self.collection
            ))
            .json(&serde_json::json!({"points": pts}))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn upsert_edges(&self, _: Vec<EdgeRecord>) -> Result<()> {
        Ok(())
    }
}
