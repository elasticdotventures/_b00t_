//! DataFabricPipeline — fanout via b00t-ipc Transport bus (NATS preferred).
//!
//! Write path: publish → b00t-ipc bus → all subscribers (grafeo, zvec) handle independently.
//! Read path:  direct async calls to grafeo (graph) and zvec (vector ANN), merged.
//!
//! When `ipc-fanout` feature is OFF: falls back to tokio::join! (in-process only).
//!
//! NATS subjects:
//!   b00t.data_fabric.{namespace}.upsert — FabricRecord[] fanout
//!   b00t.data_fabric.{namespace}.edges  — EdgeRecord[] fanout
//!   b00t.data_fabric.{namespace}.query  — reserved for future req-reply queries

use anyhow::Result;
use std::collections::HashSet;
use tracing;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(feature = "ipc-fanout")]
use b00t_ipc::transport::{K0mmand3rMessage, Transport};

use super::grafeo::GrafeoStore;
use super::zvec::ZvecStore;
use super::{
    DataFabricBackend, DataFabricQuery, DataFabricResult, DataFabricSource, DataFabricStream,
    EdgeRecord, FabricRecord, FactRecord, KnowledgeStoreBackend, QueryResult, SemanticQuery,
    StoreConfig,
};

// ─── NATS subject helpers ────────────────────────────────────────────────────

fn subj_upsert(ns: &str) -> String { format!("b00t.data_fabric.{ns}.upsert") }
fn subj_edges(ns: &str)  -> String { format!("b00t.data_fabric.{ns}.edges") }

// ─── DataFabricPipeline ──────────────────────────────────────────────────────

/// Unified fanout: grafeo owns graph+triples, zvec owns vector ANN.
/// Write path goes through b00t-ipc Transport bus when `ipc-fanout` feature is active.
pub struct DataFabricPipeline {
    pub grafeo: GrafeoStore,
    pub zvec: ZvecStore,
    pub namespace: String,
    /// b00t-ipc Transport bus for write fanout. None = in-process fallback.
    #[cfg(feature = "ipc-fanout")]
    pub bus: Option<Arc<dyn Transport + Send + Sync>>,
    /// Guards against publish-before-subscribe data loss when bus is configured.
    /// 🤓 bus_upsert() returns Err if workers haven't been started yet.
    #[cfg(feature = "ipc-fanout")]
    workers_started: Arc<AtomicBool>,
}

// Arc<dyn Transport> is Clone since Arc itself is Clone
impl Clone for DataFabricPipeline {
    fn clone(&self) -> Self {
        Self {
            grafeo: self.grafeo.clone(),
            zvec: self.zvec.clone(),
            namespace: self.namespace.clone(),
            #[cfg(feature = "ipc-fanout")]
            bus: self.bus.clone(),
            #[cfg(feature = "ipc-fanout")]
            workers_started: Arc::clone(&self.workers_started),
        }
    }
}

impl DataFabricPipeline {
    pub fn try_new(config: &StoreConfig) -> Result<Self> {
        Ok(Self {
            grafeo: <GrafeoStore as DataFabricBackend>::try_new(config)?,
            zvec: <ZvecStore as DataFabricBackend>::try_new(config)?,
            namespace: config.namespace.clone(),
            #[cfg(feature = "ipc-fanout")]
            bus: None,
            #[cfg(feature = "ipc-fanout")]
            workers_started: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Attach a b00t-ipc Transport for NATS-based write fanout.
    #[cfg(feature = "ipc-fanout")]
    pub fn with_bus<T: Transport + Send + Sync + 'static>(mut self, transport: T) -> Self {
        self.bus = Some(Arc::new(transport));
        self
    }

    /// Publish upsert records onto the bus — all subscribers handle writes independently.
    #[cfg(feature = "ipc-fanout")]
    async fn bus_upsert(&self, records: &[FabricRecord]) -> Result<()> {
        if let Some(bus) = &self.bus {
            // 🤓 Guard: NATS core has no persistence — publishing before workers subscribe
            // silently drops data. Enforce start_workers() before first write.
            if !self.workers_started.load(Ordering::Acquire) {
                return Err(anyhow::anyhow!(
                    "DataFabricPipeline: call start_workers() before fabric_upsert() \
                     to avoid publish-before-subscribe data loss on NATS bus"
                ));
            }
            let json = serde_json::to_string(records)?;
            let msg = K0mmand3rMessage {
                verb: "fabric.upsert".to_string(),
                params: Default::default(),
                content: Some(json),
                timestamp: chrono::Utc::now(),
                agent_id: None,
            };
            bus.publish(&subj_upsert(&self.namespace), &msg).await?;
            return Ok(());
        }
        // No bus: direct in-process join (fallback)
        let (r1, r2) = tokio::join!(
            self.grafeo.fabric_upsert(records.to_vec()),
            self.zvec.fabric_upsert(records.to_vec()),
        );
        r1?; r2?;
        Ok(())
    }

    /// Publish edge records onto the bus.
    #[cfg(feature = "ipc-fanout")]
    async fn bus_edges(&self, edges: &[EdgeRecord]) -> Result<()> {
        if let Some(bus) = &self.bus {
            let json = serde_json::to_string(edges)?;
            let msg = K0mmand3rMessage {
                verb: "fabric.edges".to_string(),
                params: Default::default(),
                content: Some(json),
                timestamp: chrono::Utc::now(),
                agent_id: None,
            };
            bus.publish(&subj_edges(&self.namespace), &msg).await?;
            return Ok(());
        }
        self.grafeo.fabric_upsert_edges(edges.to_vec()).await
    }

    /// Spawn a subscriber worker that drains the bus into `grafeo`.
    /// Call once on startup when bus is configured.
    #[cfg(feature = "ipc-fanout")]
    pub async fn start_grafeo_worker(&self) -> Result<()> {
        let Some(bus) = &self.bus else { return Ok(()); };
        let grafeo = self.grafeo.clone();
        let ns = self.namespace.clone();

        // Upsert worker
        let mut rx_upsert = bus.subscribe(&subj_upsert(&ns)).await?;
        let grafeo_u = grafeo.clone();
        tokio::spawn(async move {
            while let Some((_ch, msg)) = rx_upsert.recv().await {
                if let Some(json) = &msg.content {
                    if let Ok(records) = serde_json::from_str::<Vec<FabricRecord>>(json) {
                        let _ = grafeo_u.fabric_upsert(records).await;
                    }
                }
            }
        });

        // Edge worker
        let mut rx_edges = bus.subscribe(&subj_edges(&ns)).await?;
        tokio::spawn(async move {
            while let Some((_ch, msg)) = rx_edges.recv().await {
                if let Some(json) = &msg.content {
                    if let Ok(edges) = serde_json::from_str::<Vec<EdgeRecord>>(json) {
                        let _ = grafeo.fabric_upsert_edges(edges).await;
                    }
                }
            }
        });

        Ok(())
    }

    /// Spawn a subscriber worker that drains the bus into `zvec`.
    #[cfg(feature = "ipc-fanout")]
    pub async fn start_zvec_worker(&self) -> Result<()> {
        let Some(bus) = &self.bus else { return Ok(()); };
        let zvec = self.zvec.clone();
        let ns = self.namespace.clone();

        let mut rx = bus.subscribe(&subj_upsert(&ns)).await?;
        tokio::spawn(async move {
            while let Some((_ch, msg)) = rx.recv().await {
                if let Some(json) = &msg.content {
                    if let Ok(records) = serde_json::from_str::<Vec<FabricRecord>>(json) {
                        // zvec only stores records that have embeddings
                        let _ = zvec.fabric_upsert(records).await;
                    }
                }
            }
        });
        Ok(())
    }

    /// Start all backend subscriber workers. Call once after `with_bus()`.
    /// Sets the internal guard that allows `fabric_upsert` to publish to the bus.
    #[cfg(feature = "ipc-fanout")]
    pub async fn start_workers(&self) -> Result<()> {
        self.start_grafeo_worker().await?;
        self.start_zvec_worker().await?;
        // Arm the guard AFTER workers are subscribed — prevents publish-before-subscribe.
        self.workers_started.store(true, Ordering::Release);
        Ok(())
    }
}

// ─── DataFabricBackend impl ──────────────────────────────────────────────────

#[async_trait::async_trait]
impl DataFabricBackend for DataFabricPipeline {
    fn try_new(config: &StoreConfig) -> Result<Self> {
        DataFabricPipeline::try_new(config)
    }

    async fn fabric_query(&self, query: DataFabricQuery) -> Result<DataFabricResult> {
        let has_vector = query.vector.is_some();
        // Read path: always direct (query needs a response; request-reply overhead not worth it)
        let (g_res, z_res) = tokio::join!(
            self.grafeo.fabric_query(query.clone()),
            self.zvec.fabric_query(query.clone()),
        );
        let mut facts = match g_res {
            Ok(r) => r.facts,
            Err(e) => { tracing::warn!("grafeo query failed: {e:#}"); vec![] }
        };
        let vector_hits = if has_vector {
            match z_res {
                Ok(r) => r.vector_hits,
                Err(e) => { tracing::warn!("zvec query failed: {e:#}"); vec![] }
            }
        } else { vec![] };
        let mut seen = HashSet::new();
        facts.retain(|f| seen.insert((f.subject.clone(), f.predicate.clone())));
        Ok(DataFabricResult { facts, vector_hits, source: Some(DataFabricSource::Both) })
    }

    async fn fabric_upsert(&self, records: Vec<FabricRecord>) -> Result<()> {
        #[cfg(feature = "ipc-fanout")]
        return self.bus_upsert(&records).await;

        #[cfg(not(feature = "ipc-fanout"))]
        {
            let (r1, r2) = tokio::join!(
                self.grafeo.fabric_upsert(records.clone()),
                self.zvec.fabric_upsert(records),
            );
            r1?; r2?;
            Ok(())
        }
    }

    async fn fabric_upsert_edges(&self, edges: Vec<EdgeRecord>) -> Result<()> {
        #[cfg(feature = "ipc-fanout")]
        return self.bus_edges(&edges).await;

        #[cfg(not(feature = "ipc-fanout"))]
        self.grafeo.fabric_upsert_edges(edges).await
    }

    async fn fabric_stream(&self, query: DataFabricQuery) -> Result<DataFabricStream<FabricRecord>> {
        let result = self.fabric_query(query).await?;
        let mut records: Vec<FabricRecord> = result.facts.into_iter().map(Into::into).collect();
        for h in result.vector_hits {
            records.push(FabricRecord {
                subject: h.subject, predicate: h.predicate, object: h.object, embedding: None,
            });
        }
        Ok(DataFabricStream::from_vec(records))
    }
}

// Bridge: DataFabricPipeline → KnowledgeStoreBackend drop-in
#[async_trait::async_trait]
impl KnowledgeStoreBackend for DataFabricPipeline {
    fn try_new(config: StoreConfig) -> Result<Self> {
        DataFabricPipeline::try_new(&config)
    }
    async fn query(&self, q: SemanticQuery) -> Result<QueryResult> {
        self.as_semantic_query(q).await
    }
    async fn upsert_facts(&self, facts: Vec<FactRecord>) -> Result<()> {
        self.fabric_upsert(facts.into_iter().map(Into::into).collect()).await
    }
    async fn upsert_edges(&self, edges: Vec<EdgeRecord>) -> Result<()> {
        self.fabric_upsert_edges(edges).await
    }
}

// ─── Optional Qdrant REST stub ───────────────────────────────────────────────
// Enable with: cargo build --features qdrant
// ⚠️ SOUL.tomllm: qdrant.node_status = "optional-disabled" on this node.

#[cfg(feature = "qdrant")]
pub mod qdrant {
    use super::super::{EdgeRecord, FactRecord, KnowledgeStoreBackend, QueryResult, SemanticQuery, StoreConfig};
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
                self.client.put(&url)
                    .json(&serde_json::json!({"vectors":{"size":1536,"distance":"Cosine"}}))
                    .send().await?.error_for_status()?;
            }
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl KnowledgeStoreBackend for QdrantStore {
        fn try_new(config: StoreConfig) -> Result<Self> {
            Ok(Self {
                endpoint: if config.endpoint.is_empty() { "http://localhost:6333".into() } else { config.endpoint },
                collection: config.namespace,
                client: reqwest::Client::new(),
            })
        }
        async fn query(&self, q: SemanticQuery) -> Result<QueryResult> {
            let url = format!("{}/collections/{}/points/scroll", self.endpoint, self.collection);
            let mut must = vec![];
            if let Some(s) = &q.subject { must.push(serde_json::json!({"key":"subject","match":{"value":s}})); }
            if let Some(p) = &q.predicate { must.push(serde_json::json!({"key":"predicate","match":{"value":p}})); }
            let body = serde_json::json!({
                "limit": 100, "with_payload": true,
                "filter": if must.is_empty() { serde_json::Value::Null } else { serde_json::json!({"must":must}) }
            });
            let json: serde_json::Value = self.client.post(&url).json(&body).send().await?.json().await?;
            let facts = json["result"]["points"].as_array().cloned().unwrap_or_default()
                .into_iter().filter_map(|p| {
                    let pl = p.get("payload")?;
                    Some(FactRecord {
                        subject: pl["subject"].as_str()?.to_string(),
                        predicate: pl["predicate"].as_str()?.to_string(),
                        object: pl["object"].clone(),
                    })
                }).collect();
            Ok(QueryResult { facts })
        }
        async fn upsert_facts(&self, facts: Vec<FactRecord>) -> Result<()> {
            self.ensure_collection().await?;
            let pts: Vec<_> = facts.iter().enumerate().map(|(i, f)| serde_json::json!({
                "id": i, "vector": vec![0.0f32; 1536],
                "payload": {"subject":f.subject,"predicate":f.predicate,"object":f.object}
            })).collect();
            self.client
                .put(&format!("{}/collections/{}/points", self.endpoint, self.collection))
                .json(&serde_json::json!({"points":pts}))
                .send().await?.error_for_status()?;
            Ok(())
        }
        async fn upsert_edges(&self, _: Vec<EdgeRecord>) -> Result<()> { Ok(()) }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::super::{DataFabricStream, FabricRecord};

    #[test]
    fn test_subj_format() {
        assert_eq!(super::subj_upsert("b00t-core"), "b00t.data_fabric.b00t-core.upsert");
        assert_eq!(super::subj_edges("b00t-core"),  "b00t.data_fabric.b00t-core.edges");
    }

    #[tokio::test]
    async fn test_dedup_logic() {
        let records = vec![
            FabricRecord { subject: "a".into(), predicate: "type".into(), object: serde_json::json!("x"), embedding: None },
            FabricRecord { subject: "a".into(), predicate: "type".into(), object: serde_json::json!("x"), embedding: None },
            FabricRecord { subject: "b".into(), predicate: "type".into(), object: serde_json::json!("y"), embedding: None },
        ];
        let mut seen = std::collections::HashSet::new();
        let deduped: Vec<_> = records.into_iter()
            .filter(|r| seen.insert((r.subject.clone(), r.predicate.clone())))
            .collect();
        assert_eq!(deduped.len(), 2);
        let result = DataFabricStream::from_vec(deduped)
            .map(|r| r.subject.clone()).collect().await.unwrap();
        assert_eq!(result, vec!["a", "b"]);
    }
}
