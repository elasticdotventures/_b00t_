//! Concrete QuerySource implementations for the b00t learn fanout bus.
//!
//! # Sources
//!
//! | Source                | Weight | Method                                      |
//! |-----------------------|--------|---------------------------------------------|
//! | DatumSearchSource     |   3    | Keyword/regex match across datum fields      |
//! | GraphAdjacencySource  |   2    | Horn FOL traversal over compiled triples     |
//!
//! # Extension points
//! - NatsQuerySource: implement QuerySource to publish over b00t-ipc NatsTransport
//! - GrokRagSource: implement QuerySource to query irontology RAG backend
//! - OxigraphSparqlSource: see b00t_c0re_lib::query_bus (feature-gated)

use anyhow::Result;
use async_trait::async_trait;
use b00t_c0re_lib::query_bus::{QueryContext, QueryResult, QuerySource, TrustGrade};
use b00t_c0re_lib::reasoning::{adjacency::find_adjacent, graph_rules};

use crate::datum_triples::compile_datum_triples;
use crate::datum_utils::search_datums;

// ── DatumSearchSource ─────────────────────────────────────────────────────────

/// Keyword / regex search across all datum fields (key, name, hint, lfmf_category, path).
///
/// Scoring:
///   +3  full-phrase match (specificity bonus)
///   +1  per-word match (each word that hits any field)
///
/// Trust: DatumUser for file-backed datums, DatumCompiled for compiled ones.
pub struct DatumSearchSource {
    pub b00t_path: String,
}

impl DatumSearchSource {
    pub fn new(b00t_path: impl Into<String>) -> Self {
        Self { b00t_path: b00t_path.into() }
    }
}

#[async_trait]
impl QuerySource for DatumSearchSource {
    fn name(&self) -> &'static str { "datum:search" }
    fn weight(&self) -> u32 { 3 }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        let mut scores: std::collections::HashMap<String, (u32, crate::datum_utils::DatumSearchResult)> =
            std::collections::HashMap::new();

        // Full-phrase match → +3
        let phrase_pat = regex::escape(&ctx.text);
        if let Ok(hits) = search_datums(&self.b00t_path, &phrase_pat, None, None) {
            for r in hits {
                scores.entry(r.key.clone()).or_insert((0, r)).0 += 3;
            }
        }

        // Per-word match → +1 each
        for word in &ctx.words {
            let word_pat = regex::escape(word);
            if let Ok(hits) = search_datums(&self.b00t_path, &word_pat, None, None) {
                for r in hits {
                    let e = scores.entry(r.key.clone()).or_insert((0, r));
                    e.0 += 1;
                }
            }
        }

        let results = scores
            .into_values()
            .map(|(score, r)| {
                let trust = if r.datum_type.is_some() {
                    TrustGrade::DatumUser
                } else {
                    TrustGrade::DatumCompiled
                };
                QueryResult {
                    key: r.key.clone(),
                    summary: if r.hint.is_empty() { r.name.clone() } else { r.hint.clone() },
                    source: "datum:search",
                    trust,
                    score,
                    match_reason: r.match_reason,
                }
            })
            .collect();

        Ok(results)
    }
}

// ── GraphAdjacencySource ──────────────────────────────────────────────────────

/// FOL graph traversal: compiles datum triples → Horn rules → find_adjacent.
///
/// Uses `b00t:dependsOn`, `b00t:hasPart`, `b00t:requires` edges from compiled
/// datum fields. Horn rules derive transitive closure (DependsOn, Reachable).
/// `find_adjacent` scores by goal-text overlap + reachability distance.
///
/// This is the deterministic multi-hop reasoning path — no LLM required.
/// A sm0l model can consume the output to orient a frontier model cheaply.
pub struct GraphAdjacencySource {
    pub b00t_path: String,
    /// Max results from adjacency traversal
    pub top: usize,
}

impl GraphAdjacencySource {
    pub fn new(b00t_path: impl Into<String>, top: usize) -> Self {
        Self { b00t_path: b00t_path.into(), top }
    }
}

#[async_trait]
impl QuerySource for GraphAdjacencySource {
    fn name(&self) -> &'static str { "graph:adjacency" }
    fn weight(&self) -> u32 { 2 }

    async fn query(&self, ctx: &QueryContext) -> Result<Vec<QueryResult>> {
        // Use pre-compiled triples from context if available, else compile fresh.
        // 🤓 Compiling from disk is O(|datums|); callers should populate ctx.triples
        //    when calling fanout in a loop to amortize the filesystem read.
        let triples = if ctx.triples.is_empty() {
            compile_datum_triples(&self.b00t_path).unwrap_or_default()
        } else {
            ctx.triples.clone()
        };

        if triples.is_empty() {
            return Ok(vec![]);
        }

        let horn = graph_rules::derive(triples);
        let goal_words: Vec<&str> = ctx.words.iter().map(|s| s.as_str()).collect();

        let adjacent = find_adjacent(
            // find_adjacent wants raw triples for goalText lookup; pass empty since
            // datum-graph triples don't have ooda:goalText — rely on horn.reachable alone.
            &[],
            &horn,
            &goal_words,
            self.top,
        );

        // Additionally: collect direct b00t:dependsOn + b00t:hasPart neighbors
        // for any datum key that overlaps ctx.words (1-hop expansion).
        let word_set: std::collections::HashSet<String> = ctx.words.iter().cloned().collect();
        let mut direct_neighbors: Vec<QueryResult> = horn
            .depends_on
            .iter()
            .filter_map(|(from, to)| {
                // 🤓 from = "b00t:datum/<key>"; extract key for word-overlap check
                let from_key = from.strip_prefix("b00t:datum/").unwrap_or(from.as_str());
                let to_key = to.strip_prefix("b00t:datum/").unwrap_or(to.as_str());
                let from_words: Vec<&str> = from_key.split(['.', '-', '_']).collect();
                let overlaps = from_words.iter().any(|w| word_set.contains(*w));
                if overlaps && !to.starts_with("b00t:type/") {
                    Some(QueryResult {
                        key: to_key.to_string(),
                        summary: format!("prerequisite of {from_key}"),
                        source: "graph:adjacency",
                        trust: TrustGrade::DatumUser,
                        score: 2,
                        match_reason: Some(format!("b00t:dependsOn from {from_key}")),
                    })
                } else {
                    None
                }
            })
            .collect();

        // Merge find_adjacent results
        let mut results: Vec<QueryResult> = adjacent
            .into_iter()
            .map(|(key, score)| QueryResult {
                key: key.strip_prefix("b00t:datum/").unwrap_or(&key).to_string(),
                summary: String::new(), // enriched by bus collation with datum:search
                source: "graph:adjacency",
                trust: TrustGrade::DatumUser,
                score,
                match_reason: Some("horn:reachable".to_string()),
            })
            .collect();

        results.append(&mut direct_neighbors);
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use b00t_c0re_lib::query_bus::QueryBus;
    use std::fs;
    use tempfile::TempDir;

    fn setup_datums(dir: &TempDir) {
        let b00t = dir.path();
        fs::write(b00t.join("rust.cli.toml"), "[b00t]\nname = \"rust\"\ntype = \"cli\"\nhint = \"Rust programming language\"\ndepends_on = [\"cargo.cli\"]\nkeywords = [\"systems\", \"memory-safe\"]\n").unwrap();
        fs::write(b00t.join("cargo.cli.toml"), "[b00t]\nname = \"cargo\"\ntype = \"cli\"\nhint = \"Rust package manager and build tool\"\nkeywords = [\"build\", \"packages\"]\n").unwrap();
        fs::write(b00t.join("uv.cli.toml"), "[b00t]\nname = \"uv\"\ntype = \"cli\"\nhint = \"Fast Python package manager\"\ndepends_on = [\"python.cli\"]\n").unwrap();
    }

    #[tokio::test]
    async fn test_datum_search_finds_rust() {
        let dir = TempDir::new().unwrap();
        setup_datums(&dir);
        let path = dir.path().to_str().unwrap().to_string();
        let source = DatumSearchSource::new(&path);
        let ctx = QueryContext::new("rust systems", 10, vec![]);
        let results = source.query(&ctx).await.unwrap();
        assert!(results.iter().any(|r| r.key.contains("rust")));
    }

    #[tokio::test]
    async fn test_graph_adjacency_finds_cargo_via_depends_on() {
        let dir = TempDir::new().unwrap();
        setup_datums(&dir);
        let path = dir.path().to_str().unwrap().to_string();

        // Pre-compile triples
        let triples = compile_datum_triples(&path).unwrap();
        assert!(!triples.is_empty());

        let source = GraphAdjacencySource::new(&path, 10);
        let ctx = QueryContext::new("rust build", 10, triples);
        let results = source.query(&ctx).await.unwrap();

        // cargo.cli should appear as prerequisite of rust.cli
        assert!(
            results.iter().any(|r| r.key.contains("cargo")),
            "graph adjacency should find cargo as rust dependency: {:?}",
            results.iter().map(|r| &r.key).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn test_query_bus_fanout_merges_sources() {
        let dir = TempDir::new().unwrap();
        setup_datums(&dir);
        let path = dir.path().to_str().unwrap().to_string();
        let triples = compile_datum_triples(&path).unwrap();

        let bus = QueryBus::new()
            .with_source(DatumSearchSource::new(&path))
            .with_source(GraphAdjacencySource::new(&path, 10));

        let ctx = QueryContext::new("rust", 10, triples);
        let results = bus.fanout(&ctx).await;

        assert!(!results.is_empty());
        // rust.cli must appear, with score boosted by both sources
        let rust = results.iter().find(|r| r.key.contains("rust"));
        assert!(rust.is_some(), "rust should appear in merged results");
    }
}
