//! ZvecStore — in-process vector DB via zvec FFI (Apache-2.0).
//! Dense + sparse, HNSW, WAL persistence. Edges are no-op (grafeo handles graph topology).
//! github: alibaba/zvec / zvec-ai/zvec-rust

use anyhow::Result;
use std::sync::Arc;
use zvec::{Collection, CollectionSchema, DataType, Doc, FieldSchema, IndexParams, MetricType, SearchQuery};

use super::{
    DataFabricBackend, DataFabricQuery, DataFabricResult, DataFabricSource, DataFabricStream,
    EdgeRecord, FabricRecord, FactRecord, KnowledgeStoreBackend, QueryResult, SemanticQuery,
    StoreConfig, VectorHit,
};

// Embedding dimension — must match the ingestion pipeline.
// 🤓 Override via namespace prefix "dim:N:your-ns" to change per-collection.
const DEFAULT_DIM: u32 = 1536; // OpenAI ada-002 / nomic-embed-text compatible

// 🤓 zvec FFI global init is NOT idempotent — guard with OnceLock so double-init never occurs.
// 🤓 get_or_try_init is unstable; store Result<(),String> and convert on each call.
static ZVEC_INIT: std::sync::OnceLock<std::result::Result<(), String>> = std::sync::OnceLock::new();

fn ensure_zvec_init() -> Result<()> {
    ZVEC_INIT.get_or_init(|| {
        zvec::initialize(None).map_err(|e| format!("zvec init: {e}"))
    }).as_ref().map(|_| ()).map_err(|e| anyhow::anyhow!("{e}"))
}

pub(crate) fn parse_dim(namespace: &str) -> u32 {
    namespace
        .strip_prefix("dim:")
        .and_then(|s| s.split(':').next())
        .and_then(|d| d.parse::<u32>().ok())
        .filter(|&d| d > 0)
        .unwrap_or(DEFAULT_DIM)
}

/// Wraps zvec `Collection` in `Arc` — zvec FFI is single-process-safe.
#[derive(Clone)]
pub struct ZvecStore {
    collection: Arc<Collection>,
    dim: u32,
}

fn build_schema(name: &str, dim: u32) -> Result<CollectionSchema> {
    CollectionSchema::builder(name)
        .add_field(
            FieldSchema::new("subject", DataType::String, false, 0)
                .map_err(|e| anyhow::anyhow!("zvec schema field subject: {e}"))?,
        )
        .add_field(
            FieldSchema::new("predicate", DataType::String, false, 0)
                .map_err(|e| anyhow::anyhow!("zvec schema field predicate: {e}"))?,
        )
        .add_field(
            FieldSchema::new("object", DataType::String, false, 0)
                .map_err(|e| anyhow::anyhow!("zvec schema field object: {e}"))?,
        )
        .add_vector_field(
            "embedding",
            DataType::VectorFp32,
            dim,
            IndexParams::hnsw(MetricType::Cosine, 16, 200)
                .map_err(|e| anyhow::anyhow!("zvec hnsw params: {e}"))?,
        )
        .build()
        .map_err(|e| anyhow::anyhow!("zvec schema build: {e}"))
}

#[async_trait::async_trait]
impl DataFabricBackend for ZvecStore {
    fn try_new(config: &StoreConfig) -> Result<Self> {
        ensure_zvec_init()?;
        let dim = parse_dim(&config.namespace);
        let data_path = config.data_path.clone().unwrap_or_else(|| {
            dirs::home_dir().unwrap_or_default().join("._b00t_/zvec")
        });
        let col_path = data_path.join(&config.namespace);
        let schema = build_schema(&config.namespace, dim)?;
        let collection = if col_path.exists() {
            Collection::open(col_path.to_str().unwrap_or("."), None)
                .map_err(|e| anyhow::anyhow!("zvec open: {e}"))?
        } else {
            Collection::create_and_open(col_path.to_str().unwrap_or("."), &schema, None)
                .map_err(|e| anyhow::anyhow!("zvec create: {e}"))?
        };
        Ok(Self { collection: Arc::new(collection), dim })
    }

    async fn fabric_query(&self, query: DataFabricQuery) -> Result<DataFabricResult> {
        let collection = Arc::clone(&self.collection);
        let vector = query.vector.clone();
        let topk = query.topk.max(1);
        let subj = query.subject.clone();

        tokio::task::spawn_blocking(move || {
            let mut vector_hits = Vec::new();
            if let Some(vec) = vector {
                let filter_expr = subj.as_deref()
                    .map(|s| format!("subject = '{}'", s.replace('\'', "\\'")));
                let q = SearchQuery::builder()
                    .field_name("embedding")
                    .vector(&vec)
                    .topk(topk as i32)
                    .output_fields(&["subject", "predicate", "object"]);
                let q = if let Some(f) = filter_expr { q.filter(&f) } else { q };
                let q = q.build().map_err(|e| anyhow::anyhow!("zvec query build: {e}"))?;

                let results = collection.query(&q).map_err(|e| anyhow::anyhow!("zvec query: {e}"))?;
                for r in results {
                    let id = r.get_pk().unwrap_or("").to_string();
                    let score = r.get_score();
                    let subject = r.get_string("subject").ok().flatten().unwrap_or_default();
                    let predicate = r.get_string("predicate").ok().flatten().unwrap_or_default();
                    let obj_raw = r.get_string("object").ok().flatten().unwrap_or_default();
                    let object = serde_json::from_str(&obj_raw)
                        .unwrap_or(serde_json::Value::String(obj_raw));
                    vector_hits.push(VectorHit { id, score, subject, predicate, object });
                }
            }
            Ok(DataFabricResult {
                facts: Vec::new(),
                vector_hits,
                source: Some(DataFabricSource::Zvec),
            })
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {e}"))?
    }

    async fn fabric_upsert(&self, records: Vec<FabricRecord>) -> Result<()> {
        let collection = Arc::clone(&self.collection);
        let with_vecs: Vec<_> = records.into_iter().filter(|r| r.embedding.is_some()).collect();
        tokio::task::spawn_blocking(move || {
            let docs: Vec<Doc> = with_vecs.iter().filter_map(|r| {
                let vec = r.embedding.as_ref()?;
                let mut doc = Doc::new().ok()?;
                doc.set_pk(&format!("{}:{}", r.subject, r.predicate));
                doc.add_string("subject", &r.subject).ok()?;
                doc.add_string("predicate", &r.predicate).ok()?;
                doc.add_string("object", &serde_json::to_string(&r.object).unwrap_or_default()).ok()?;
                doc.add_vector_f32("embedding", vec).ok()?;
                Some(doc)
            }).collect();
            if !docs.is_empty() {
                let refs: Vec<&Doc> = docs.iter().collect();
                collection.upsert(&refs).map_err(|e| anyhow::anyhow!("zvec upsert: {e}"))?;
            }
            Ok(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("spawn_blocking: {e}"))?
    }

    async fn fabric_upsert_edges(&self, _edges: Vec<EdgeRecord>) -> Result<()> {
        Ok(()) // vector-only; grafeo owns graph topology
    }

    async fn fabric_stream(&self, query: DataFabricQuery) -> Result<DataFabricStream<FabricRecord>> {
        let result = self.fabric_query(query).await?;
        let records = result.vector_hits.into_iter().map(|h| FabricRecord {
            subject: h.subject, predicate: h.predicate, object: h.object, embedding: None,
        }).collect();
        Ok(DataFabricStream::from_vec(records))
    }
}

// Bridge: ZvecStore → KnowledgeStoreBackend
#[async_trait::async_trait]
impl KnowledgeStoreBackend for ZvecStore {
    fn try_new(config: StoreConfig) -> Result<Self> {
        <ZvecStore as DataFabricBackend>::try_new(&config)
    }
    async fn query(&self, q: SemanticQuery) -> Result<QueryResult> {
        if q.subject.is_none() && q.predicate.is_none() {
            return Ok(QueryResult { facts: Vec::new() });
        }
        Ok(self.fabric_query(q.into()).await?.into())
    }
    async fn upsert_facts(&self, facts: Vec<FactRecord>) -> Result<()> {
        self.fabric_upsert(facts.into_iter().map(Into::into).collect()).await
    }
    async fn upsert_edges(&self, _: Vec<EdgeRecord>) -> Result<()> {
        Ok(())
    }
}
