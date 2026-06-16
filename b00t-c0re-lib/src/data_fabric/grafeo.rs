//! GrafeoStore — embedded LPG+RDF+vector via grafeo crate.
//! Sync session API wrapped in `spawn_blocking` for async compat.
//! features: ai (vector+hybrid+cdc), rdf (SPARQL+triple-store)

use anyhow::Result;
use grafeo::GrafeoDB;
use std::sync::Arc;

use super::{
    DataFabricBackend, DataFabricQuery, DataFabricResult, DataFabricSource, DataFabricStream,
    EdgeRecord, FabricRecord, FactRecord, KnowledgeStoreBackend, QueryResult, SemanticQuery,
    StoreConfig, VectorHit,
};

/// Wraps `grafeo::GrafeoDB` in `Arc<Mutex>` for async safety.
#[derive(Clone)]
pub struct GrafeoStore {
    inner: Arc<tokio::sync::Mutex<GrafeoDB>>,
    namespace: String,
    data_path: Option<std::path::PathBuf>,
}

impl GrafeoStore {
    fn label(&self) -> String {
        format!("Fact_{}", self.namespace.replace(['-', ':'], "_"))
    }
}

#[async_trait::async_trait]
impl DataFabricBackend for GrafeoStore {
    fn try_new(config: &StoreConfig) -> Result<Self> {
        let db = if let Some(path) = &config.data_path {
            GrafeoDB::open(path).map_err(|e| anyhow::anyhow!("grafeo open: {e}"))?
        } else {
            GrafeoDB::new_in_memory()
        };
        Ok(Self {
            inner: Arc::new(tokio::sync::Mutex::new(db)),
            namespace: config.namespace.clone(),
            data_path: config.data_path.clone(),
        })
    }

    async fn fabric_query(&self, query: DataFabricQuery) -> Result<DataFabricResult> {
        let db = Arc::clone(&self.inner);
        let label = self.label();
        let subj = query.subject.clone();
        let pred = query.predicate.clone();
        let vector = query.vector.clone();
        let topk = query.topk.max(1);
        let min_score = query.min_score.unwrap_or(0.0);

        tokio::task::spawn_blocking(move || {
            let db = db.blocking_lock();
            let mut session = db.session();
            let mut facts = Vec::new();
            let mut vector_hits = Vec::new();

            // Triple-pattern match
            let where_clause = match (&subj, &pred) {
                (Some(s), Some(p)) => format!(
                    "WHERE f.subject = '{}' AND f.predicate = '{}'",
                    s.replace('\'', "\\'"), p.replace('\'', "\\'")
                ),
                (Some(s), None) => format!("WHERE f.subject = '{}'", s.replace('\'', "\\'")),
                (None, Some(p)) => format!("WHERE f.predicate = '{}'", p.replace('\'', "\\'")),
                (None, None) => String::new(),
            };
            let gql = format!(
                "MATCH (f:{label}) {where_clause} RETURN f.subject, f.predicate, f.object LIMIT 1000"
            );
            if let Ok(result) = session.execute(&gql) {
                for row in result.rows() {
                    if row.len() >= 3 {
                        let subject = row[0].to_string().trim_matches('"').to_string();
                        let predicate = row[1].to_string().trim_matches('"').to_string();
                        let obj_s = row[2].to_string();
                        let object = serde_json::from_str(&obj_s)
                            .unwrap_or(serde_json::Value::String(obj_s));
                        facts.push(FactRecord { subject, predicate, object });
                    }
                }
            }

            // Vector ANN (requires grafeo `ai` feature)
            if let Some(vec) = vector {
                let vec_str = vec.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
                let vgql = format!(
                    "MATCH (f:{label}) \
                     WHERE f.embedding IS NOT NULL \
                     AND cosine_similarity(f.embedding, vector([{vec_str}])) > {min_score:.4} \
                     RETURN f.subject, f.predicate, f.object, \
                            cosine_similarity(f.embedding, vector([{vec_str}])) AS score \
                     ORDER BY score DESC LIMIT {topk}"
                );
                if let Ok(result) = session.execute(&vgql) {
                    for row in result.rows() {
                        if row.len() >= 4 {
                            let subject = row[0].to_string().trim_matches('"').to_string();
                            let predicate = row[1].to_string().trim_matches('"').to_string();
                            let obj_s = row[2].to_string();
                            let object = serde_json::from_str(&obj_s)
                                .unwrap_or(serde_json::Value::String(obj_s));
                            let score: f32 = row[3].to_string().parse().unwrap_or(0.0);
                            vector_hits.push(VectorHit {
                                id: format!("{subject}:{predicate}"),
                                score, subject, predicate, object,
                            });
                        }
                    }
                }
            }

            Ok(DataFabricResult { facts, vector_hits, source: Some(DataFabricSource::Grafeo) })
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {e}"))?
    }

    async fn fabric_upsert(&self, records: Vec<FabricRecord>) -> Result<()> {
        let db = Arc::clone(&self.inner);
        let label = self.label();
        tokio::task::spawn_blocking(move || {
            let db = db.blocking_lock();
            let mut session = db.session();
            for r in &records {
                let s = r.subject.replace('\'', "\\'");
                let p = r.predicate.replace('\'', "\\'");
                let o = serde_json::to_string(&r.object).unwrap_or_default().replace('\'', "\\'");
                let gql = if let Some(vec) = &r.embedding {
                    let vs = vec.iter().map(|f| f.to_string()).collect::<Vec<_>>().join(", ");
                    format!(
                        "MERGE (f:{label} {{subject: '{s}', predicate: '{p}'}}) \
                         SET f.object = '{o}', f.embedding = vector([{vs}])"
                    )
                } else {
                    format!(
                        "MERGE (f:{label} {{subject: '{s}', predicate: '{p}'}}) SET f.object = '{o}'"
                    )
                };
                let _ = session.execute(&gql); // MERGE = upsert; continue on single-record errors
            }
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {e}"))?
    }

    async fn fabric_upsert_edges(&self, edges: Vec<EdgeRecord>) -> Result<()> {
        let db = Arc::clone(&self.inner);
        let label = self.label();
        tokio::task::spawn_blocking(move || {
            let db = db.blocking_lock();
            let mut session = db.session();
            for e in &edges {
                let from = e.from.replace('\'', "\\'");
                let to = e.to.replace('\'', "\\'");
                let kind = format!("{:?}", e.kind);
                let gql = format!(
                    "MERGE (a:{label} {{subject: '{from}'}}) \
                     MERGE (b:{label} {{subject: '{to}'}}) \
                     MERGE (a)-[r:{kind} {{weight: {}}}]->(b)",
                    e.weight
                );
                let _ = session.execute(&gql);
            }
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {e}"))?
    }

    async fn fabric_stream(&self, query: DataFabricQuery) -> Result<DataFabricStream<FabricRecord>> {
        let result = self.fabric_query(query).await?;
        Ok(DataFabricStream::from_vec(result.facts.into_iter().map(Into::into).collect()))
    }
}

// Bridge: GrafeoStore → KnowledgeStoreBackend drop-in
#[async_trait::async_trait]
impl KnowledgeStoreBackend for GrafeoStore {
    fn try_new(config: StoreConfig) -> Result<Self> {
        <GrafeoStore as DataFabricBackend>::try_new(&config)
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
