//! Generic fanout query bus for parallel multi-source knowledge retrieval.
//!
//! # Design
//! `QueryBus` runs registered `QuerySource` impls concurrently via `futures::join_all`.
//! In-process fanout is zero-cost (direct async calls, no IPC overhead).
//! Cross-process extension: implement `QuerySource` backed by `b00t-ipc` NATS transport
//!   → publish query to `b00t:learn:query`, subscribe to `b00t:learn:response:<id>`.
//!
//! # Predicate Namespace (OWL2 / BFO / SysMLv2 forward-compatible)
//!
//! | b00t predicate     | OWL2 type         | BFO 2.0 mapping        | SysMLv2 mapping              |
//! |--------------------|-------------------|------------------------|------------------------------|
//! | b00t:dependsOn     | ObjectProperty    | bfo:depends-on         | DependencyRelationship       |
//! | b00t:requires      | ObjectProperty    | bfo:has-constraint     | RequirementConstraint        |
//! | b00t:hasPart       | ObjectProperty    | bfo:has-part           | PartUsage / CompositePort    |
//! | b00t:relatedTo     | ObjectProperty    | owl:topObjectProperty  | AssociationRelationship      |
//! | b00t:hasKeyword    | AnnotationProperty| skos:altLabel          | comment / documentation      |
//! | b00t:hasSkill      | AnnotationProperty| skos:related           | FeatureTyping                |
//! | b00t:hasType       | AnnotationProperty| rdf:type               | Classifier                   |
//! | rdfs:label         | AnnotationProperty| rdfs:label             | documentation name           |
//!
//! CLIF/CLIF-style axiom (future):
//!   (forall (x y) (if (b00t:dependsOn x y) (b00t:relatedTo x y)))
//!
//! OWL2 Manchester Syntax stub (for horned-owl integration):
//!   ObjectProperty: b00t:dependsOn
//!     SubPropertyOf: b00t:relatedTo
//!     Characteristics: Transitive

use anyhow::Result;
use async_trait::async_trait;
use futures::future::join_all;
use std::collections::HashMap;

// ── Trust hierarchy ───────────────────────────────────────────────────────────

/// Source trust grades, ordered best→worst.
/// Lower ordinal = higher trust (datum:user is most authoritative).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrustGrade {
    /// User-authored TOML datum — canonical ground truth for this hive
    DatumUser,
    /// Compiled into binary at build time (b00t-compiled datums)
    DatumCompiled,
    /// Assimilated via grok RAG (ingested, not hand-authored)
    Rag,
    /// From web crawl — treat as hypothesis, not fact
    WebUnverified,
}

impl TrustGrade {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DatumUser     => "datum:user",
            Self::DatumCompiled => "datum:compiled",
            Self::Rag           => "rag",
            Self::WebUnverified => "web:unverified",
        }
    }
}

// ── Query context ─────────────────────────────────────────────────────────────

/// Context passed to every `QuerySource::query` call.
#[derive(Debug, Clone)]
pub struct QueryContext {
    /// Raw query text from the user/agent
    pub text: String,
    /// Tokenized words (len ≥ 3, lowercase) for per-word matching
    pub words: Vec<String>,
    /// Max results the caller wants shown (bus holds 4× internally for sm0l gate)
    pub limit: usize,
    /// Pre-compiled SPO triples for graph-based sources.
    /// Populated by `compile_datum_triples`; empty if graph not loaded yet.
    pub triples: Vec<(String, String, String)>,
}

impl QueryContext {
    pub fn new(
        text: impl Into<String>,
        limit: usize,
        triples: Vec<(String, String, String)>,
    ) -> Self {
        let text = text.into();
        let words = text
            .split_whitespace()
            .filter(|w| w.len() >= 3)
            .map(|w| w.to_lowercase())
            .collect();
        Self { text, words, limit, triples }
    }
}

// ── Result type ───────────────────────────────────────────────────────────────

/// Unified result from any `QuerySource`.
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// Datum key or node URI (e.g. `rust.cli`, `b00t:datum/docker.cli`)
    pub key: String,
    /// Short hint / description for display
    pub summary: String,
    /// Name of the source that produced this result
    pub source: &'static str,
    /// Trust grade of this result's provenance
    pub trust: TrustGrade,
    /// Raw relevance score before weight multiplication
    pub score: u32,
    /// Human-readable match explanation
    pub match_reason: Option<String>,
}

// ── QuerySource trait ─────────────────────────────────────────────────────────

/// Implemented by each knowledge source (datum search, graph adjacency, RAG, SPARQL…).
///
/// All sources run concurrently in `QueryBus::fanout` via `join_all`.
/// To add a cross-process NATS source: implement this trait to publish/subscribe
///   on `b00t:learn:{query,response}` subjects via `b00t-ipc::transport::NatsTransport`.
#[async_trait]
pub trait QuerySource: Send + Sync {
    /// Stable identifier shown in result metadata
    fn name(&self) -> &'static str;

    /// Multiplier applied to all scores from this source during collation.
    /// Higher weight = source results rank higher in merged output.
    fn weight(&self) -> u32 { 1 }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>>;
}

// ── QueryBus ─────────────────────────────────────────────────────────────────

/// Fan-out bus: runs all registered sources in parallel, collates by key,
/// deduplicates (highest trust + accumulated score), and returns ranked results.
pub struct QueryBus {
    sources: Vec<Box<dyn QuerySource>>,
}

impl QueryBus {
    pub fn new() -> Self {
        Self { sources: Vec::new() }
    }

    /// Register a query source. Returns self for chaining.
    pub fn with_source(mut self, s: impl QuerySource + 'static) -> Self {
        self.sources.push(Box::new(s));
        self
    }

    /// Run all sources concurrently, collate by key, return sorted results.
    ///
    /// Collation rules:
    /// - Same key from multiple sources → scores accumulate, best (lowest-ordinal) trust wins
    /// - Results sorted: score desc, then key asc (deterministic tie-break)
    /// - Bus holds `ctx.limit * 4` internally so callers can apply a sm0l gate
    pub async fn fanout(&self, ctx: &QueryContext) -> Vec<QueryResult> {
        let futures: Vec<_> = self.sources.iter().map(|s| s.query(ctx)).collect();
        let responses = join_all(futures).await;

        let mut agg: HashMap<String, QueryResult> = HashMap::new();

        for (idx, response) in responses.into_iter().enumerate() {
            let weight = self.sources[idx].weight();
            for mut r in response.unwrap_or_default() {
                r.score = r.score.saturating_mul(weight);
                agg.entry(r.key.clone())
                    .and_modify(|existing| {
                        existing.score = existing.score.saturating_add(r.score);
                        // Keep highest-trust (lowest ordinal) provenance
                        if r.trust < existing.trust {
                            existing.trust = r.trust.clone();
                            existing.source = r.source;
                        }
                    })
                    .or_insert(r);
            }
        }

        let mut ranked: Vec<QueryResult> = agg.into_values().collect();
        ranked.sort_by(|a, b| b.score.cmp(&a.score).then(a.key.cmp(&b.key)));
        // Hold 4× limit so callers can apply sm0l summarization before final truncation
        ranked.truncate(ctx.limit.saturating_mul(4).max(20));
        ranked
    }
}

impl Default for QueryBus {
    fn default() -> Self { Self::new() }
}

// ── OWL2 / oxigraph SPARQL stub (feature-gated) ──────────────────────────────

/// Stub SPARQL source backed by oxigraph in-memory store.
///
/// Future integration path for SysMLv2 MBSE, CLIF axioms, and BFO/UFO ontologies:
///   1. Load compiled datum triples into `oxigraph::store::Store`
///   2. Import BFO2 / UFO OWL2 ontology axioms
///   3. Execute property-path SPARQL:
///      ```sparql
///      PREFIX b00t: <http://b00t.promptexecution.com/ontology#>
///      SELECT ?adj ?label WHERE {
///        <b00t:datum/{topic}> (b00t:dependsOn|b00t:hasPart|b00t:relatedTo)+ ?adj .
///        OPTIONAL { ?adj rdfs:label ?label }
///      } LIMIT 20
///      ```
///   4. OWL2 DL reasoning via pellet-rs or konclude for subproperty/transitivity closure
///
/// Until the full ontology is loaded, this source returns empty results gracefully.
#[cfg(feature = "store-oxigraph")]
const DEFAULT_ADJACENCY_TEMPLATE: &str = r#"
PREFIX b00t: <http://b00t.promptexecution.com/ontology#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT ?adj ?label WHERE {
  {SUBJECT} (b00t:dependsOn|b00t:hasPart|b00t:relatedTo)+ ?adj .
  OPTIONAL { ?adj rdfs:label ?label }
} LIMIT 40
"#;

#[cfg(feature = "store-oxigraph")]
pub struct OxigraphSparqlSource {
    store: std::sync::Arc<crate::irontology_bridge::OxigraphStore>,
    /// SPARQL SELECT with a single `{SUBJECT}` placeholder for the topic IRI.
    pub query_template: String,
    /// Informational max property-path traversal depth (the template uses `+`).
    pub depth: usize,
}

#[cfg(feature = "store-oxigraph")]
impl OxigraphSparqlSource {
    pub fn new(store: std::sync::Arc<crate::irontology_bridge::OxigraphStore>) -> Self {
        Self {
            store,
            query_template: DEFAULT_ADJACENCY_TEMPLATE.to_string(),
            depth: 4,
        }
    }

    /// Open the store at `~/.b00t/oxigraph/<ns>` where `<ns>` = `$B00T_OXIGRAPH_NS`
    /// (default `"graph"`).
    pub fn from_env() -> Result<Self> {
        use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};
        let ns = std::env::var("B00T_OXIGRAPH_NS").unwrap_or_else(|_| "graph".to_string());
        let store = OxigraphStore::try_new(StoreConfig {
            endpoint: String::new(),
            namespace: ns,
            data_path: None,
        })?;
        Ok(Self::new(std::sync::Arc::new(store)))
    }

    fn slug(text: &str) -> String {
        text.trim().to_lowercase().replace([' ', '/'], "-")
    }
}

#[cfg(feature = "store-oxigraph")]
#[async_trait]
impl QuerySource for OxigraphSparqlSource {
    fn name(&self) -> &'static str { "oxigraph:sparql" }
    fn weight(&self) -> u32 { 2 } // Graph reasoning outweighs keyword search

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        use oxigraph::sparql::QueryResults;

        let subject_iri = format!(
            "<{}datum/{}>",
            crate::graph_load::B00T_NS,
            Self::slug(&ctx.text)
        );
        let sparql = self.query_template.replace("{SUBJECT}", &subject_iri);

        let results = match self.store.store().query(&sparql) {
            Ok(r) => r,
            Err(_) => return Ok(vec![]), // malformed template / empty store — degrade gracefully
        };

        let mut out = Vec::new();
        if let QueryResults::Solutions(solutions) = results {
            for sol in solutions {
                let sol = match sol {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let adj = match sol.get("adj") {
                    Some(oxigraph::model::Term::NamedNode(n)) => n.as_str().to_string(),
                    _ => continue,
                };
                let key = adj.rsplit(['#', '/']).next().unwrap_or(&adj).to_string();
                let label = match sol.get("label") {
                    Some(oxigraph::model::Term::Literal(l)) => l.value().to_string(),
                    _ => String::new(),
                };
                out.push(QueryResult {
                    key,
                    summary: label,
                    source: "oxigraph:sparql",
                    trust: TrustGrade::DatumCompiled,
                    score: 5,
                    match_reason: Some(format!("graph path from {}", ctx.text)),
                });
            }
        }
        let _ = self.depth; // reserved for a bounded-length path variant
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FixedSource { name: &'static str, results: Vec<QueryResult> }

    #[async_trait]
    impl QuerySource for FixedSource {
        fn name(&self) -> &'static str { self.name }
        async fn query(&self, _ctx: &QueryContext) -> Result<Vec<QueryResult>> {
            Ok(self.results.clone())
        }
    }

    fn r(key: &str, score: u32, trust: TrustGrade) -> QueryResult {
        QueryResult {
            key: key.into(),
            summary: format!("hint for {key}"),
            source: "test",
            trust,
            score,
            match_reason: None,
        }
    }

    #[tokio::test]
    async fn test_fanout_accumulates_scores() {
        let bus = QueryBus::new()
            .with_source(FixedSource { name: "a", results: vec![r("foo", 2, TrustGrade::DatumUser)] })
            .with_source(FixedSource { name: "b", results: vec![r("foo", 3, TrustGrade::Rag)] });

        let ctx = QueryContext::new("foo", 5, vec![]);
        let results = bus.fanout(&ctx).await;

        let foo = results.iter().find(|r| r.key == "foo").expect("foo missing");
        assert_eq!(foo.score, 5, "scores should accumulate across sources");
        assert_eq!(foo.trust, TrustGrade::DatumUser, "best trust should win");
    }

    #[tokio::test]
    async fn test_fanout_deduplicates_keys() {
        let bus = QueryBus::new()
            .with_source(FixedSource { name: "a", results: vec![r("x", 1, TrustGrade::DatumUser), r("y", 1, TrustGrade::Rag)] })
            .with_source(FixedSource { name: "b", results: vec![r("x", 1, TrustGrade::DatumCompiled)] });

        let ctx = QueryContext::new("test", 10, vec![]);
        let results = bus.fanout(&ctx).await;

        // x appears in both sources but should be deduplicated
        let x_count = results.iter().filter(|r| r.key == "x").count();
        assert_eq!(x_count, 1);
        let x = results.iter().find(|r| r.key == "x").unwrap();
        assert_eq!(x.score, 2);
        assert_eq!(x.trust, TrustGrade::DatumUser);
    }

    #[tokio::test]
    async fn test_fanout_ranks_by_score_desc() {
        let bus = QueryBus::new()
            .with_source(FixedSource { name: "a", results: vec![
                r("low", 1, TrustGrade::Rag),
                r("high", 5, TrustGrade::DatumUser),
                r("mid", 3, TrustGrade::DatumCompiled),
            ]});

        let ctx = QueryContext::new("test", 10, vec![]);
        let results = bus.fanout(&ctx).await;
        assert_eq!(results[0].key, "high");
        assert_eq!(results[1].key, "mid");
        assert_eq!(results[2].key, "low");
    }

    #[tokio::test]
    async fn test_empty_context_words_filter_short() {
        let ctx = QueryContext::new("go by it", 5, vec![]);
        // "go", "by", "it" all < 3 chars
        assert!(ctx.words.is_empty());
    }

    #[test]
    fn test_trust_grade_ordering() {
        assert!(TrustGrade::DatumUser < TrustGrade::DatumCompiled);
        assert!(TrustGrade::DatumCompiled < TrustGrade::Rag);
        assert!(TrustGrade::Rag < TrustGrade::WebUnverified);
    }
}

#[cfg(all(test, feature = "store-oxigraph"))]
mod oxigraph_source_tests {
    use super::*;
    use crate::graph_load::load_graph;
    use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};
    use std::sync::Arc;

    fn store_with(triples: &[(&str, &str, &str)]) -> Arc<OxigraphStore> {
        let dir = std::env::temp_dir().join(format!(
            "sp5-src-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let s = OxigraphStore::try_new(StoreConfig {
            endpoint: String::new(),
            namespace: "t".into(),
            data_path: Some(dir),
        })
        .unwrap();
        let owned: Vec<(String, String, String)> = triples
            .iter()
            .map(|(a, b, c)| (a.to_string(), b.to_string(), c.to_string()))
            .collect();
        load_graph(&s, &owned).unwrap();
        Arc::new(s)
    }

    #[tokio::test]
    async fn empty_store_returns_no_results_gracefully() {
        let src = OxigraphSparqlSource::new(store_with(&[]));
        let ctx = QueryContext::new("anything", 10, vec![]);
        assert!(src.query(&ctx).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn walks_a_dependson_path_from_the_topic_subject() {
        let src = OxigraphSparqlSource::new(store_with(&[
            ("b00t:datum/rust.cli", "b00t:dependsOn", "b00t:datum/cargo"),
            ("b00t:datum/cargo", "b00t:dependsOn", "b00t:datum/rustup"),
            ("b00t:datum/rustup", "rdfs:label", "Rustup"),
        ]));
        let ctx = QueryContext::new("rust.cli", 10, vec![]);
        let hits = src.query(&ctx).await.unwrap();
        let keys: Vec<&str> = hits.iter().map(|h| h.key.as_str()).collect();
        assert!(keys.contains(&"cargo"), "got {keys:?}");
        assert!(keys.contains(&"rustup"), "got {keys:?}");
        assert!(hits.iter().all(|h| h.source == "oxigraph:sparql"));
    }
}
