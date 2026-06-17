//! # Data Fabric — unified graph + vector data pipeline
//!
//! Entangles Grafeo (embedded graph DB, LPG+RDF+vector) and Zvec (in-process vector DB)
//! behind a single `DataFabricBackend` trait with `DataFabricStream<T>` async iterators.
//!
//! Feature flags:
//! - `store-grafeo`  — GrafeoStore (graph + vector)
//! - `store-zvec`    — ZvecStore (vector only)
//! - `data-fabric`   — DataFabricPipeline (grafeo + zvec fanout)
//! - `qdrant`        — optional Qdrant REST adapter (disabled on this node)

use anyhow::Result;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

pub use crate::irontology_bridge::{
    EdgeKind, EdgeRecord, FactRecord, KnowledgeStoreBackend, QueryResult, SemanticQuery, StoreConfig,
};

#[cfg(feature = "store-grafeo")]
pub mod grafeo;
#[cfg(feature = "store-zvec")]
pub mod zvec;
#[cfg(feature = "data-fabric")]
pub mod pipeline;
#[cfg(feature = "qdrant")]
pub mod qdrant;

#[cfg(test)]
mod tests;

// ─── Query / result types ───────────────────────────────────────────────────

/// Richer query — extends SemanticQuery with ANN vector search.
///
/// # Example
/// ```rust,no_run
/// # use b00t_c0re_lib::data_fabric::{DataFabricQuery, SemanticQuery};
/// // Triple-pattern match only
/// let q = DataFabricQuery {
///     subject: Some("b00t:datum/rust/001".into()),
///     topk: 10,
///     ..Default::default()
/// };
/// // ANN search
/// let q_ann = DataFabricQuery { vector: Some(vec![0.1; 1536]), topk: 5, min_score: Some(0.75), ..Default::default() };
/// // From SemanticQuery (topk defaults to 10)
/// let fq: DataFabricQuery = SemanticQuery { subject: Some("s".into()), predicate: None }.into();
/// assert_eq!(fq.topk, 10);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFabricQuery {
    pub subject: Option<String>,
    pub predicate: Option<String>,
    /// Embedding for ANN similarity search
    pub vector: Option<Vec<f32>>,
    /// Field carrying the vector (default: "embedding")
    pub vector_field: Option<String>,
    /// Max ANN results
    pub topk: usize,
    /// Min cosine similarity (0.0–1.0)
    pub min_score: Option<f32>,
}

impl Default for DataFabricQuery {
    fn default() -> Self {
        Self { subject: None, predicate: None, vector: None, vector_field: None, topk: 10, min_score: None }
    }
}

impl From<SemanticQuery> for DataFabricQuery {
    fn from(q: SemanticQuery) -> Self {
        Self { subject: q.subject, predicate: q.predicate, topk: 10, ..Default::default() }
    }
}

/// Hit from vector ANN search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorHit {
    pub id: String,
    pub score: f32,
    pub subject: String,
    pub predicate: String,
    pub object: serde_json::Value,
}

/// Which backend(s) produced a result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFabricSource {
    Grafeo,
    Zvec,
    Both,
}

/// Combined query result: triple-store facts + ANN vector hits.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DataFabricResult {
    pub facts: Vec<FactRecord>,
    pub vector_hits: Vec<VectorHit>,
    pub source: Option<DataFabricSource>,
}

impl From<DataFabricResult> for QueryResult {
    fn from(r: DataFabricResult) -> Self {
        QueryResult { facts: r.facts }
    }
}

/// Dual-ingestion record: FactRecord fields + optional embedding.
///
/// Records with `embedding` go to both grafeo (graph) and zvec (ANN index).
/// Records without `embedding` are stored in grafeo only; zvec silently skips them.
///
/// # Example
/// ```rust,no_run
/// # use b00t_c0re_lib::data_fabric::{FabricRecord, FactRecord};
/// # use serde_json::json;
/// // Both backends
/// let r = FabricRecord {
///     subject: "b00t:datum/rust/001".into(),
///     predicate: "b00t:hasContent".into(),
///     object: json!("Rust ownership"),
///     embedding: Some(vec![0.1_f32; 1536]),
/// };
/// // Grafeo only
/// let r2 = FabricRecord { embedding: None, ..r.clone() };
/// // From FactRecord (embedding dropped)
/// let fact: FactRecord = r.into();
/// let fr: FabricRecord = fact.into();
/// assert!(fr.embedding.is_none());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricRecord {
    pub subject: String,
    pub predicate: String,
    pub object: serde_json::Value,
    /// Embedding for both zvec and grafeo vector index
    pub embedding: Option<Vec<f32>>,
}

impl From<FactRecord> for FabricRecord {
    fn from(f: FactRecord) -> Self {
        Self { subject: f.subject, predicate: f.predicate, object: f.object, embedding: None }
    }
}
impl From<FabricRecord> for FactRecord {
    fn from(r: FabricRecord) -> Self {
        FactRecord { subject: r.subject, predicate: r.predicate, object: r.object }
    }
}

// ─── DataFabricStream ───────────────────────────────────────────────────────

/// Async streaming iterator with map / filter / flat_map / collect / for_each.
///
/// Backed by a pinned `futures::Stream<Item = Result<T>>`.
/// Errors propagate: `filter` passes errors through; `for_each` / `collect`
/// short-circuit on first error.
///
/// # Example
/// ```rust,no_run
/// # use b00t_c0re_lib::data_fabric::{DataFabricStream, FabricRecord};
/// # use serde_json::json;
/// # #[tokio::main]
/// # async fn main() -> anyhow::Result<()> {
/// let records: Vec<FabricRecord> = (0..5).map(|i| FabricRecord {
///     subject: format!("s{i}"), predicate: "p".into(), object: json!(i), embedding: None,
/// }).collect();
/// let subjects = DataFabricStream::from_vec(records)
///     .filter(|r| r.subject != "s2")
///     .map(|r| r.subject.clone())
///     .collect().await?;
/// assert_eq!(subjects, vec!["s0", "s1", "s3", "s4"]);
/// # Ok(()) }
/// ```
pub struct DataFabricStream<T: Send + 'static> {
    inner: Pin<Box<dyn Stream<Item = Result<T>> + Send + 'static>>,
}

impl<T: Send + 'static> DataFabricStream<T> {
    pub fn new<S: Stream<Item = Result<T>> + Send + 'static>(s: S) -> Self {
        Self { inner: Box::pin(s) }
    }

    pub fn from_vec(items: Vec<T>) -> Self {
        Self::new(futures::stream::iter(items.into_iter().map(Ok)))
    }

    pub fn map<U: Send + 'static, F: Fn(T) -> U + Send + 'static>(
        self, f: F,
    ) -> DataFabricStream<U> {
        DataFabricStream::new(self.inner.map(move |r| r.map(|v| f(v))))
    }

    pub fn filter<F: Fn(&T) -> bool + Send + 'static>(self, f: F) -> DataFabricStream<T> {
        DataFabricStream::new(self.inner.filter(move |r| {
            let keep = r.as_ref().map(|v| f(v)).unwrap_or(true);
            async move { keep }
        }))
    }

    pub fn flat_map<U: Send + 'static, F: Fn(T) -> Vec<U> + Send + 'static>(
        self, f: F,
    ) -> DataFabricStream<U> {
        DataFabricStream::new(self.inner.flat_map(move |r| {
            let items: Vec<Result<U>> = match r {
                Ok(v) => f(v).into_iter().map(Ok).collect(),
                Err(e) => vec![Err(e)],
            };
            futures::stream::iter(items)
        }))
    }

    pub async fn collect(self) -> Result<Vec<T>> {
        self.inner.collect::<Vec<_>>().await.into_iter().collect()
    }

    pub async fn for_each<F: FnMut(T)>(self, mut f: F) -> Result<()> {
        let mut s = self.inner;
        while let Some(item) = s.next().await { f(item?); }
        Ok(())
    }
}

// ─── DataFabricBackend trait ─────────────────────────────────────────────────

#[async_trait::async_trait]
pub trait DataFabricBackend: Clone + Send + Sync + 'static {
    fn try_new(config: &StoreConfig) -> Result<Self> where Self: Sized;
    async fn fabric_query(&self, query: DataFabricQuery) -> Result<DataFabricResult>;
    async fn fabric_upsert(&self, records: Vec<FabricRecord>) -> Result<()>;
    async fn fabric_upsert_edges(&self, edges: Vec<EdgeRecord>) -> Result<()>;
    async fn fabric_stream(&self, query: DataFabricQuery) -> Result<DataFabricStream<FabricRecord>>;

    /// Bridge to KnowledgeStoreBackend::query
    async fn as_semantic_query(&self, q: SemanticQuery) -> Result<QueryResult> {
        Ok(self.fabric_query(q.into()).await?.into())
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

// 🤓 inline_tests: kept separate from mod tests (tests.rs) to avoid name collision.
#[cfg(test)]
mod inline_tests {
    use super::*;

    #[test]
    fn test_fabric_query_from_semantic() {
        let sq = SemanticQuery {
            subject: Some("b00t:rust".to_string()),
            predicate: Some("b00t:type".to_string()),
        };
        let fq: DataFabricQuery = sq.into();
        assert_eq!(fq.subject.as_deref(), Some("b00t:rust"));
        assert!(fq.vector.is_none());
    }

    #[test]
    fn test_fabric_record_roundtrip() {
        let fact = FactRecord {
            subject: "s".into(), predicate: "p".into(), object: serde_json::json!("o"),
        };
        let r: FabricRecord = fact.clone().into();
        let f2: FactRecord = r.into();
        assert_eq!(f2.subject, fact.subject);
    }

    #[tokio::test]
    async fn test_stream_map_filter_collect() {
        let items: Vec<FabricRecord> = (0..5).map(|i| FabricRecord {
            subject: format!("s{i}"), predicate: "p".into(),
            object: serde_json::json!(i), embedding: None,
        }).collect();

        let result = DataFabricStream::from_vec(items)
            .filter(|r| r.subject != "s2")
            .map(|r| r.subject.clone())
            .collect().await.unwrap();

        assert_eq!(result, vec!["s0", "s1", "s3", "s4"]);
    }

    #[tokio::test]
    async fn test_stream_flat_map() {
        let items = vec![
            FabricRecord { subject: "a".into(), predicate: "p".into(), object: serde_json::json!(1), embedding: None },
        ];
        let result = DataFabricStream::from_vec(items)
            .flat_map(|r| vec![r.subject.clone(), r.predicate.clone()])
            .collect().await.unwrap();
        assert_eq!(result, vec!["a", "p"]);
    }
}
