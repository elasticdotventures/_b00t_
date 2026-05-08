//! Irontology bridge — maps b00t datum schema to irontology semantic layer
//!
//! # Design
//! - `DatumNode`: canonical b00t datum struct (topic, class, content, tags, predicates)
//! - `b00t_datum!` macro: declarative DSL → `DatumNode` literal
//! - `IrontologyBridgeClient`: wraps the active store backend for ingest/query without MCP subprocess
//!   (MCP transport pending: vendor/irontology-mcp has no main.rs yet)
//! - Storage: Cargo-selected backend at `~/.b00t/neumann/<namespace>/`
//!
//! # Semantic mapping
//! b00t datum → irontology `FactRecord` triples:
//!   subject   = `b00t:datum/<topic>/<uuid>`
//!   predicate = `b00t:hasContent` | `b00t:hasTag` | `b00t:hasClass` | `<custom_predicate>`
//!   object    = JSON Value (String for content/tags, object for complex predicates)
//!
//! 🤓 Embeddings deferred: NeumannStore accepts EmbeddingRecord but generation requires
//!    active inference stack (vllm/ollama). Facts-only path works today; vector search
//!    is additive once `b00t hive activate inference-qwen3` runs.

use serde::{Deserialize, Serialize};

#[cfg(all(feature = "store-neumann", feature = "store-oxigraph"))]
compile_error!("Enable exactly one b00t-c0re-lib storage backend: store-neumann OR store-oxigraph.");

#[cfg(not(any(feature = "store-neumann", feature = "store-oxigraph")))]
compile_error!("Enable one b00t-c0re-lib storage backend: store-neumann OR store-oxigraph.");

// ── Core datum type ───────────────────────────────────────────────────────────

/// Canonical b00t datum — portable across raglite and irontology backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumNode {
    /// b00t topic / datum name (must be in available_topics)
    pub topic: String,
    /// OWL class label (e.g. "ProgrammingConcept", "OperationalFact", "Skill")
    pub class: String,
    /// Primary content to be ingested
    pub content: String,
    /// Searchable tags
    pub tags: Vec<String>,
    /// Additional RDF-style predicate→value pairs
    pub predicates: Vec<(String, String)>,
}

impl DatumNode {
    pub fn new(
        topic: impl Into<String>,
        class: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self {
            topic: topic.into(),
            class: class.into(),
            content: content.into(),
            tags: Vec::new(),
            predicates: Vec::new(),
        }
    }

    /// Canonical irontology subject URI for this datum instance
    pub fn subject_uri(&self, id: &str) -> String {
        format!("b00t:datum/{}/{}", self.topic, id)
    }
}

// ── Bridge types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactRecord {
    pub subject: String,
    pub predicate: String,
    pub object: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRecord {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
    pub weight: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EdgeKind {
    ClassifiedAs,
    DependsOn,
    StoredIn,
    Related,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticQuery {
    pub subject: Option<String>,
    pub predicate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreConfig {
    pub endpoint: String,
    pub namespace: String,
    pub data_path: Option<std::path::PathBuf>,
}

pub type NeumannConfig = StoreConfig;

#[async_trait::async_trait]
pub trait KnowledgeStoreBackend: Clone + Send + Sync + 'static {
    fn try_new(config: StoreConfig) -> anyhow::Result<Self>
    where
        Self: Sized;

    async fn query(&self, query: SemanticQuery) -> anyhow::Result<QueryResult>;
    async fn upsert_facts(&self, facts: Vec<FactRecord>) -> anyhow::Result<()>;
    async fn upsert_edges(&self, edges: Vec<EdgeRecord>) -> anyhow::Result<()>;
}

cfg_if::cfg_if! {
    if #[cfg(feature = "store-oxigraph")] {
        pub type ActiveKnowledgeStore = OxigraphStore;
        pub const COMPILED_KNOWLEDGE_BACKEND: &str = "oxigraph";
    } else if #[cfg(feature = "store-neumann")] {
        pub type ActiveKnowledgeStore = NeumannStore;
        pub const COMPILED_KNOWLEDGE_BACKEND: &str = "neumann";
    } else {
        pub type ActiveKnowledgeStore = MissingKnowledgeStore;
        pub const COMPILED_KNOWLEDGE_BACKEND: &str = "none";
    }
}

pub fn compiled_knowledge_backend() -> &'static str {
    COMPILED_KNOWLEDGE_BACKEND
}

#[cfg(not(any(feature = "store-neumann", feature = "store-oxigraph")))]
#[derive(Debug, Clone)]
pub struct MissingKnowledgeStore;

#[cfg(not(any(feature = "store-neumann", feature = "store-oxigraph")))]
#[async_trait::async_trait]
impl KnowledgeStoreBackend for MissingKnowledgeStore {
    fn try_new(_config: StoreConfig) -> anyhow::Result<Self> {
        anyhow::bail!("no b00t-c0re-lib storage backend enabled")
    }

    async fn query(&self, _query: SemanticQuery) -> anyhow::Result<QueryResult> {
        anyhow::bail!("no b00t-c0re-lib storage backend enabled")
    }

    async fn upsert_facts(&self, _facts: Vec<FactRecord>) -> anyhow::Result<()> {
        anyhow::bail!("no b00t-c0re-lib storage backend enabled")
    }

    async fn upsert_edges(&self, _edges: Vec<EdgeRecord>) -> anyhow::Result<()> {
        anyhow::bail!("no b00t-c0re-lib storage backend enabled")
    }
}

#[cfg(feature = "store-neumann")]
#[derive(Clone)]
pub struct NeumannStore {
    inner: std::sync::Arc<storage_neumann::NeumannStore>,
}

#[cfg(feature = "store-neumann")]
impl NeumannStore {
    pub fn try_new(config: StoreConfig) -> anyhow::Result<Self> {
        Ok(Self {
            inner: std::sync::Arc::new(storage_neumann::NeumannStore::try_new(
                config.into_vendor_neumann_config(),
            )?),
        })
    }
}

#[cfg(feature = "store-neumann")]
impl StoreConfig {
    fn into_vendor_neumann_config(self) -> storage_neumann::NeumannConfig {
        let data_path = self.data_path.or_else(|| {
            dirs::home_dir()
                .map(|home| home.join(".b00t").join("neumann").join(&self.namespace))
        });
        storage_neumann::NeumannConfig {
            endpoint: self.endpoint,
            namespace: self.namespace,
            data_path,
        }
    }
}

#[cfg(feature = "store-neumann")]
impl From<FactRecord> for storage_neumann::FactRecord {
    fn from(value: FactRecord) -> Self {
        Self {
            subject: value.subject,
            predicate: value.predicate,
            object: value.object,
        }
    }
}

#[cfg(feature = "store-neumann")]
impl From<storage_neumann::FactRecord> for FactRecord {
    fn from(value: storage_neumann::FactRecord) -> Self {
        Self {
            subject: value.subject,
            predicate: value.predicate,
            object: value.object,
        }
    }
}

#[cfg(feature = "store-neumann")]
impl From<EdgeKind> for storage_neumann::EdgeKind {
    fn from(value: EdgeKind) -> Self {
        match value {
            EdgeKind::ClassifiedAs => Self::ClassifiedAs,
            EdgeKind::DependsOn => Self::DependsOn,
            EdgeKind::StoredIn => Self::StoredIn,
            EdgeKind::Related => Self::Related,
        }
    }
}

#[cfg(feature = "store-neumann")]
impl From<storage_neumann::EdgeKind> for EdgeKind {
    fn from(value: storage_neumann::EdgeKind) -> Self {
        match value {
            storage_neumann::EdgeKind::ClassifiedAs => Self::ClassifiedAs,
            storage_neumann::EdgeKind::DependsOn => Self::DependsOn,
            storage_neumann::EdgeKind::StoredIn => Self::StoredIn,
            storage_neumann::EdgeKind::Related
            | storage_neumann::EdgeKind::Defines
            | storage_neumann::EdgeKind::Calls
            | storage_neumann::EdgeKind::Tests
            | storage_neumann::EdgeKind::Contains => Self::Related,
        }
    }
}

#[cfg(feature = "store-neumann")]
impl From<EdgeRecord> for storage_neumann::EdgeRecord {
    fn from(value: EdgeRecord) -> Self {
        Self {
            from: value.from,
            to: value.to,
            kind: value.kind.into(),
            weight: value.weight.max(0.0).round() as u32,
        }
    }
}

#[cfg(feature = "store-neumann")]
impl From<storage_neumann::EdgeRecord> for EdgeRecord {
    fn from(value: storage_neumann::EdgeRecord) -> Self {
        Self {
            from: value.from,
            to: value.to,
            kind: value.kind.into(),
            weight: value.weight as f32,
        }
    }
}

#[cfg(feature = "store-neumann")]
#[async_trait::async_trait]
impl KnowledgeStoreBackend for NeumannStore {
    fn try_new(config: StoreConfig) -> anyhow::Result<Self> {
        NeumannStore::try_new(config)
    }

    async fn query(&self, query: SemanticQuery) -> anyhow::Result<QueryResult> {
        use storage_neumann::KnowledgeStore;

        let result = self
            .inner
            .query(storage_neumann::SemanticQuery::Facts {
                subject: query.subject,
                predicate: query.predicate,
            })
            .await?;
        Ok(QueryResult {
            facts: result.facts.into_iter().map(Into::into).collect(),
        })
    }

    async fn upsert_facts(&self, facts: Vec<FactRecord>) -> anyhow::Result<()> {
        use storage_neumann::KnowledgeStore;

        self.inner
            .upsert_facts(facts.into_iter().map(Into::into).collect())
            .await
    }

    async fn upsert_edges(&self, edges: Vec<EdgeRecord>) -> anyhow::Result<()> {
        use storage_neumann::KnowledgeStore;

        self.inner
            .upsert_edges(edges.into_iter().map(Into::into).collect())
            .await
    }
}

#[cfg(feature = "store-oxigraph")]
#[derive(Clone)]
pub struct OxigraphStore {
    store: oxigraph::store::Store,
}

#[cfg(feature = "store-oxigraph")]
impl OxigraphStore {
    fn fact_to_quad(fact: &FactRecord) -> anyhow::Result<oxigraph::model::Quad> {
        use oxigraph::model::{Literal, NamedNode, Quad, Term};

        let subject = NamedNode::new(fact.subject.as_str())?;
        let predicate = NamedNode::new(fact.predicate.as_str())?;
        let object: Term = match &fact.object {
            serde_json::Value::String(value) => Literal::new_simple_literal(value.as_str()).into(),
            other => Literal::new_simple_literal(other.to_string()).into(),
        };
        Ok(Quad::new(subject, predicate, object, oxigraph::model::GraphName::DefaultGraph))
    }

    fn edge_to_fact(edge: EdgeRecord) -> FactRecord {
        FactRecord {
            subject: edge.from,
            predicate: format!("b00t:edge:{:?}", edge.kind),
            object: serde_json::json!({
                "to": edge.to,
                "weight": edge.weight,
            }),
        }
    }
}

#[cfg(feature = "store-oxigraph")]
#[async_trait::async_trait]
impl KnowledgeStoreBackend for OxigraphStore {
    fn try_new(config: StoreConfig) -> anyhow::Result<Self> {
        let data_path = config.data_path.unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join(".b00t")
                .join("oxigraph")
                .join(config.namespace)
        });
        std::fs::create_dir_all(&data_path)?;
        Ok(Self {
            store: oxigraph::store::Store::open(data_path)?,
        })
    }

    async fn query(&self, query: SemanticQuery) -> anyhow::Result<QueryResult> {
        use oxigraph::model::{NamedNode, NamedOrBlankNode, Term};

        let subject = query
            .subject
            .as_deref()
            .map(NamedNode::new)
            .transpose()?;
        let predicate = query
            .predicate
            .as_deref()
            .map(NamedNode::new)
            .transpose()?;
        let mut facts = Vec::new();
        for quad in self.store.quads_for_pattern(
            subject.as_ref().map(|node| node.as_ref().into()),
            predicate.as_ref().map(|node| node.as_ref()),
            None,
            None,
        ) {
            let quad = quad?;
            let object = match quad.object {
                Term::Literal(literal) => serde_json::Value::String(literal.value().to_string()),
                other => serde_json::Value::String(other.to_string()),
            };
            facts.push(FactRecord {
                subject: match quad.subject {
                    NamedOrBlankNode::NamedNode(node) => node.into_string(),
                    other => other.to_string(),
                },
                predicate: quad.predicate.into_string(),
                object,
            });
        }
        Ok(QueryResult { facts })
    }

    async fn upsert_facts(&self, facts: Vec<FactRecord>) -> anyhow::Result<()> {
        for fact in facts {
            self.store.insert(&Self::fact_to_quad(&fact)?)?;
        }
        Ok(())
    }

    async fn upsert_edges(&self, edges: Vec<EdgeRecord>) -> anyhow::Result<()> {
        let facts = edges.into_iter().map(Self::edge_to_fact).collect();
        self.upsert_facts(facts).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub facts: Vec<FactRecord>,
}

// ── Conversion traits ─────────────────────────────────────────────────────────

/// Convert a datum to irontology `FactRecord` triples
pub trait IntoIrontologyRecord {
    fn to_fact_records(&self, id: &str) -> Vec<FactRecord>;
    fn to_edge_records(&self, id: &str) -> Vec<EdgeRecord>;
}

impl IntoIrontologyRecord for DatumNode {
    fn to_fact_records(&self, id: &str) -> Vec<FactRecord> {
        let subject = self.subject_uri(id);
        let mut facts = vec![
            FactRecord {
                subject: subject.clone(),
                predicate: "b00t:hasContent".to_string(),
                object: serde_json::Value::String(self.content.clone()),
            },
            FactRecord {
                subject: subject.clone(),
                predicate: "b00t:hasClass".to_string(),
                object: serde_json::Value::String(self.class.clone()),
            },
            FactRecord {
                subject: subject.clone(),
                predicate: "b00t:hasTopic".to_string(),
                object: serde_json::Value::String(self.topic.clone()),
            },
        ];
        for tag in &self.tags {
            facts.push(FactRecord {
                subject: subject.clone(),
                predicate: "b00t:hasTag".to_string(),
                object: serde_json::Value::String(tag.clone()),
            });
        }
        for (pred, val) in &self.predicates {
            facts.push(FactRecord {
                subject: subject.clone(),
                predicate: format!("b00t:{}", pred),
                object: serde_json::Value::String(val.clone()),
            });
        }
        facts
    }

    fn to_edge_records(&self, id: &str) -> Vec<EdgeRecord> {
        let subject = self.subject_uri(id);
        let mut edges = vec![EdgeRecord {
            from: subject.clone(),
            to: format!("b00t:class/{}", self.class),
            kind: EdgeKind::ClassifiedAs,
            weight: 1.0,
        }];
        for (pred, val) in &self.predicates {
            let kind = match pred.as_str() {
                "requires" | "dependsOn" | "depends_on" => EdgeKind::DependsOn,
                "storedAt" => EdgeKind::StoredIn,
                "implements" | "hasPart" => EdgeKind::Related,
                _ => continue,
            };
            edges.push(EdgeRecord {
                from: subject.clone(),
                to: format!("b00t:service/{}", val),
                kind,
                weight: 1.0,
            });
        }
        edges
    }
}

/// Convert a datum to a raglite-compatible document source path + content
pub trait IntoRagDocument {
    /// Returns (content, topic)
    fn to_rag_content(&self) -> (&str, &str);
}

impl IntoRagDocument for DatumNode {
    fn to_rag_content(&self) -> (&str, &str) {
        (&self.content, &self.topic)
    }
}

// ── Irontology bridge client ──────────────────────────────────────────────────

/// Result of a single irontology ingest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrontologyIngestResult {
    pub subject_prefix: String,
    pub facts_stored: usize,
    pub edges_stored: usize,
}

/// Single result item from irontology query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrontologyQueryItem {
    pub subject: String,
    pub topic: String,
    pub content: String,
    pub tags: Vec<String>,
    pub score: f32,
}

/// Client wrapping a shared `NeumannStore` for b00t grok operations
#[derive(Clone)]
pub struct IrontologyBridgeClient {
    store: std::sync::Arc<ActiveKnowledgeStore>,
    namespace: String,
}

impl IrontologyBridgeClient {
    /// Create with sled persistence at `~/.b00t/neumann/<namespace>/`
    pub fn new(namespace: impl Into<String>) -> anyhow::Result<Self> {
        let ns: String = namespace.into();
        let data_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot resolve $HOME"))?
            .join(".b00t")
            .join("neumann")
            .join(&ns);

        std::fs::create_dir_all(&data_dir)?;

        let config = StoreConfig {
            endpoint: "http://localhost:7777".to_string(),
            namespace: ns.clone(),
            data_path: Some(data_dir),
        };
        let store = std::sync::Arc::new(
            <ActiveKnowledgeStore as KnowledgeStoreBackend>::try_new(config)?,
        );
        Ok(Self {
            store,
            namespace: ns,
        })
    }

    /// Ingest a `DatumNode` into the neumann store
    pub async fn ingest(&self, datum: &DatumNode) -> anyhow::Result<IrontologyIngestResult> {
        let id = uuid::Uuid::new_v4().to_string();
        let facts = datum.to_fact_records(&id);
        let edges = datum.to_edge_records(&id);
        let fact_count = facts.len();
        let edge_count = edges.len();

        self.store.upsert_facts(facts).await?;
        self.store.upsert_edges(edges).await?;

        Ok(IrontologyIngestResult {
            subject_prefix: format!("b00t:datum/{}/{}", datum.topic, &id[..8]),
            facts_stored: fact_count,
            edges_stored: edge_count,
        })
    }

    /// Query facts by topic + lexical content filter
    pub async fn query(
        &self,
        query: &str,
        topic: Option<&str>,
        limit: Option<usize>,
    ) -> anyhow::Result<Vec<IrontologyQueryItem>> {
        let topic_prefix = topic.map(|t| format!("b00t:datum/{}/", t));
        let qr = self
            .store
            .query(SemanticQuery {
                subject: None,
                predicate: None,
            })
            .await?;

        let mut subjects: std::collections::HashMap<String, (String, String, Vec<String>)> =
            std::collections::HashMap::new();

        for fact in &qr.facts {
            if let Some(ref prefix) = topic_prefix {
                if !fact.subject.starts_with(prefix.as_str()) {
                    continue;
                }
            }
            let entry = subjects.entry(fact.subject.clone()).or_insert_with(|| {
                let topic_str = fact
                    .subject
                    .split('/')
                    .nth(1)
                    .unwrap_or("unknown")
                    .to_string();
                (topic_str, String::new(), Vec::new())
            });
            match fact.predicate.as_str() {
                "b00t:hasContent" => {
                    if let serde_json::Value::String(s) = &fact.object {
                        entry.1 = s.clone();
                    }
                }
                "b00t:hasTag" => {
                    if let serde_json::Value::String(s) = &fact.object {
                        entry.2.push(s.clone());
                    }
                }
                _ => {}
            }
        }

        let query_lower = query.to_lowercase();
        let max = limit.unwrap_or(10);

        let mut results: Vec<IrontologyQueryItem> = subjects
            .into_iter()
            .filter(|(_, (_, content, tags))| {
                content.to_lowercase().contains(&query_lower)
                    || tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            })
            .map(|(subject, (t, content, tags))| {
                let score = content.to_lowercase().matches(&query_lower).count() as f32 + 1.0;
                IrontologyQueryItem {
                    subject,
                    topic: t,
                    content,
                    tags,
                    score,
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(max);
        Ok(results)
    }

    pub fn namespace(&self) -> &str {
        &self.namespace
    }
}

// ── b00t_datum! macro ────────────────────────────────────────────────────────

#[macro_export]
macro_rules! b00t_datum {
    (
        topic: $topic:expr,
        class: $class:expr,
        content: $content:expr
        $(, tags: [$($tag:expr),* $(,)?])?
        $(, predicates: { $($pk:ident : $pv:expr),* $(,)? })?
        $(,)?
    ) => {
        $crate::irontology_bridge::DatumNode {
            topic: $topic.to_string(),
            class: $class.to_string(),
            content: $content.to_string(),
            tags: vec![$($($tag.to_string()),*)?],
            predicates: {
                #[allow(unused_mut)]
                let mut _p: Vec<(String, String)> = Vec::new();
                $($( _p.push((stringify!($pk).to_string(), $pv.to_string())); )*)?
                _p
            },
        }
    };

    (
        topic: $topic:expr,
        content: $content:expr
        $(, tags: [$($tag:expr),* $(,)?])?
        $(,)?
    ) => {
        $crate::b00t_datum! {
            topic: $topic,
            class: "Concept",
            content: $content
            $(, tags: [$($tag),*])?
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_datum_node_constructors() {
        let n = DatumNode::new("rust", "ProgrammingConcept", "Rust memory safety");
        assert_eq!(n.topic, "rust");
        assert_eq!(n.class, "ProgrammingConcept");
        assert!(n.tags.is_empty());
    }

    #[test]
    fn test_subject_uri_format() {
        let n = DatumNode::new("rust", "Concept", "test");
        let uri = n.subject_uri("abc123");
        assert_eq!(uri, "b00t:datum/rust/abc123");
    }

    #[test]
    fn test_b00t_datum_macro_full_form() {
        let node = b00t_datum! {
            topic: "rust",
            class: "ProgrammingConcept",
            content: "Rust memory safety via ownership",
            tags: ["ownership", "safety"],
            predicates: { implements: "MemorySafety", hasPart: "BorrowChecker" }
        };
        assert_eq!(node.topic, "rust");
        assert_eq!(node.class, "ProgrammingConcept");
        assert_eq!(node.tags.len(), 2);
        assert_eq!(node.predicates.len(), 2);
        assert!(node.predicates.iter().any(|(k, _)| k == "implements"));
    }

    #[test]
    fn test_b00t_datum_macro_short_form() {
        let node = b00t_datum! {
            topic: "python",
            content: "Python uses duck typing",
        };
        assert_eq!(node.class, "Concept");
        assert_eq!(node.topic, "python");
        assert!(node.tags.is_empty());
    }

    #[test]
    fn test_compiled_knowledge_backend_is_active_backend() {
        #[cfg(feature = "store-neumann")]
        assert_eq!(compiled_knowledge_backend(), "neumann");
        #[cfg(feature = "store-oxigraph")]
        assert_eq!(compiled_knowledge_backend(), "oxigraph");
    }

    #[tokio::test]
    async fn test_active_store_persists_facts_for_later_queries() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = StoreConfig {
            endpoint: "local".to_string(),
            namespace: "test".to_string(),
            data_path: Some(tmp.path().to_path_buf()),
        };
        let store = ActiveKnowledgeStore::try_new(config.clone()).expect("store");
        store
            .upsert_facts(vec![FactRecord {
                subject: "b00t:datum/mcp/orchid".to_string(),
                predicate: "b00t:hasContent".to_string(),
                object: serde_json::Value::String("ORCHID-BOOT-75913".to_string()),
            }])
            .await
            .expect("upsert");

        drop(store);
        let reloaded = ActiveKnowledgeStore::try_new(config).expect("reload");
        let result = reloaded
            .query(SemanticQuery {
                subject: None,
                predicate: Some("b00t:hasContent".to_string()),
            })
            .await
            .expect("query");

        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].object, "ORCHID-BOOT-75913");
    }

    #[tokio::test]
    async fn test_bridge_ingest_then_query_returns_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = ActiveKnowledgeStore::try_new(StoreConfig {
            endpoint: "local".to_string(),
            namespace: "test".to_string(),
            data_path: Some(tmp.path().to_path_buf()),
        })
        .expect("store");
        let client = IrontologyBridgeClient {
            store: std::sync::Arc::new(store),
            namespace: "test".to_string(),
        };

        let mut datum = DatumNode::new(
            "mcp",
            "OperationalFact",
            "Subagent grok sharing verification token ORCHID-BOOT-75913",
        );
        datum.tags.push("subagent-grok-share".to_string());
        client.ingest(&datum).await.expect("ingest");

        let result = client
            .query("ORCHID-BOOT-75913", Some("mcp"), Some(3))
            .await
            .expect("query");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].topic, "mcp");
        assert!(result[0].content.contains("ORCHID-BOOT-75913"));
    }
}
