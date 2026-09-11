# SP5 — SysML-v2 Coherence Substrate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the b00t datum + identity/authz graph a queryable Oxigraph SPARQL store with a SHACL-lite validation gate and a native procedurally-generated KerML view projection.

**Architecture:** The query substrate lives in `b00t-c0re-lib` behind the opt-in, mutually-exclusive `store-oxigraph` Cargo feature and operates on **plain `Vec<(String, String, String)>` SPO triples** — it has zero dependency on `BootDatum`. The triple *compilers* stay in `b00t-cli` (where `BootDatum`, `datum_utils`, `AgentProfileSpec`, `ShardKind` live). A new `b00t graph emit-triples` subcommand (normal `b00t-cli` build) writes the composed triple set as JSONL; a `b00t-c0re-lib/examples/b00t-graph` binary (built `--no-default-features --features store-oxigraph`) reads that JSONL and runs load / query / validate / kerml. The KerML text emitter is contributed upstream to `ufo-types` as `sysml_model::emit_kerml` and consumed by both SP5 and `b00t-cli/src/dispatch_sysml.rs`.

**Tech Stack:** Rust, `oxigraph 0.5.8` (embedded SPARQL 1.1 RDF store), `ufo-types` (KerML abstract-syntax types + `sysml-v2-parser` grammar validation), `clap` (CLI), `serde_json` (JSONL), GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-09-10-sp5-sysml-coherence-substrate-design.md`

## Global Constraints

- **`store-oxigraph` is opt-in and mutually exclusive with the default `store-helixdb`.** `b00t-c0re-lib/src/irontology_bridge.rs` has a `compile_error!` if both (or neither) storage backend is enabled. Every SP5 build/test of `b00t-c0re-lib` graph code runs `--no-default-features --features store-oxigraph`. `b00t-cli` is **never** built with `store-oxigraph`.
- **No local `cargo build` / `cargo test` on the dev box.** The b00t workspace cold-build is 10-20GB. Verification is the CI job (`graph-oxigraph`, added in Task 10) or a delegated build on sm3lly via the pi agent harness (`b00t-cli agent invoke pi "<brief>"`, qwen3.8-27b). Local `git push` uses `--no-verify` (the pre-push hook runs `cargo test` and OOM-kills).
- **`b00t:` namespace IRI** expands to `http://b00t.promptexecution.com/ontology#`. `rdfs:` expands to `http://www.w3.org/2000/01/rdf-schema#`.
- **Determinism:** every generated artifact (KerML text, JSONL triple output) must be byte-identical across runs — sort before emitting, never iterate a `HashMap` into output, no wall-clock / UUID / RNG.
- **KerML views are native procedural Rust** — code that walks typed slices and writes KerML syntax directly. No string templates, no template engine.
- **Delegate transcription-heavy steps to pi.** Steps marked `[PI-CANDIDATE]` are complete-content transcription (the step body has the full file) — write `$CLAUDE_JOB_DIR/tmp/<task>-brief.txt` with the exact content + "Do NOT run cargo. Do NOT create/switch git branches.", run `timeout 2400 b00t-cli agent invoke pi "$(cat brief)"`, review `git diff`, fix, commit. Frontier does spec-context wiring, conflict resolution, and every review.
- **Branch:** all b00t work on `spec/sp5-sysml-coherence` (already exists, off `main`, carries the spec + PR #1298). The `ufo-types` change (Task 1) is a separate PR in `PromptExecution/ufo-types`.

---

### Task 1: `ufo-types` — `sysml_model::emit_kerml`

**Repo:** `PromptExecution/ufo-types` (separate PR, not the b00t branch).

**Files:**
- Modify: `src/sysml_model.rs` (append `emit_kerml` + a `#[cfg(test)]` round-trip test)
- Modify: `CHANGELOG.md` (add an `### Added` line under `## [Unreleased]`)
- Modify: `Cargo.toml` (bump `version` to the next patch, e.g. `0.14.1`)

**Interfaces:**
- Consumes: `crate::sysml_model::{ElementId, ElementKind, Relation}` (existing); `crate::sysml::validate_sysml_v2` (existing, `#[cfg(feature = "sysml")]`).
- Produces: `pub fn emit_kerml(package: &str, elements: &[(ElementId, ElementKind)], relations: &[Relation]) -> String`

- [ ] **Step 1: Write the failing test** — append to `src/sysml_model.rs`:

```rust
#[cfg(all(test, feature = "sysml"))]
mod emit_kerml_tests {
    use super::*;
    use crate::sysml::validate_sysml_v2;

    #[test]
    fn emit_kerml_round_trips_a_small_part_graph() {
        let elements = vec![
            (ElementId::new("alpha"), ElementKind::PartDefinition),
            (ElementId::new("beta"), ElementKind::PartDefinition),
        ];
        let relations = vec![Relation::Dependency {
            client: ElementId::new("beta"),
            supplier: ElementId::new("alpha"),
        }];
        let text = emit_kerml("b00t_graph", &elements, &relations);
        assert!(text.starts_with("package b00t_graph {"));
        assert!(text.contains("part def alpha;"));
        assert!(text.contains("part def beta;"));
        // dependency rendered as a KerML dependency line
        assert!(text.contains("dependency from beta to alpha;"));
        validate_sysml_v2(&text).expect("emitted KerML must parse");
    }

    #[test]
    fn emit_kerml_is_deterministic_regardless_of_input_order() {
        let a = vec![
            (ElementId::new("z"), ElementKind::PartDefinition),
            (ElementId::new("a"), ElementKind::PartDefinition),
        ];
        let b = vec![
            (ElementId::new("a"), ElementKind::PartDefinition),
            (ElementId::new("z"), ElementKind::PartDefinition),
        ];
        assert_eq!(emit_kerml("p", &a, &[]), emit_kerml("p", &b, &[]));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Delegate to pi on sm3lly or run in the `ufo-types` checkout (small crate, local build OK here):
Run: `cargo test -p ufo-types --features sysml emit_kerml`
Expected: FAIL — `cannot find function emit_kerml in this scope`.

- [ ] **Step 3: Write the implementation** `[PI-CANDIDATE]` — append to `src/sysml_model.rs` (before the test module):

```rust
/// Render a typed element/relation graph as deterministic KerML v2 text.
///
/// Native procedural generation: walks the slices and writes KerML syntax
/// directly. Inputs are sorted internally so output is byte-identical
/// regardless of caller ordering. Every `ElementKind` maps to its KerML
/// keyword; only the subset SP5 uses (`PartDefinition`) is exercised today,
/// the rest fall through to a `// unsupported kind` comment line rather than
/// emitting invalid syntax.
pub fn emit_kerml(
    package: &str,
    elements: &[(ElementId, ElementKind)],
    relations: &[Relation],
) -> String {
    let mut els: Vec<&(ElementId, ElementKind)> = elements.iter().collect();
    els.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    els.dedup_by(|a, b| a.0.as_str() == b.0.as_str());

    let mut out = String::new();
    out.push_str("package ");
    out.push_str(package);
    out.push_str(" {\n");

    for (id, kind) in els {
        let name = id.as_str();
        match kind {
            ElementKind::PartDefinition => {
                out.push_str(&format!("  part def {name};\n"));
            }
            ElementKind::Package => {
                out.push_str(&format!("  package {name} {{}}\n"));
            }
            other => {
                out.push_str(&format!("  // unsupported kind {other:?} for {name}\n"));
            }
        }
    }

    let mut rels: Vec<String> = relations
        .iter()
        .filter_map(|r| match r {
            Relation::Dependency { client, supplier } => Some(format!(
                "  dependency from {} to {};\n",
                client.as_str(),
                supplier.as_str()
            )),
            Relation::FeatureMembership { owner, member } => Some(format!(
                "  // feature {} in {}\n",
                member.as_str(),
                owner.as_str()
            )),
            _ => None,
        })
        .collect();
    rels.sort();
    rels.dedup();
    for line in rels {
        out.push_str(&line);
    }

    out.push_str("}\n");
    out
}
```

> Note: if `validate_sysml_v2` rejects `dependency from X to Y;` (grammar detail — verify against `sysml-v2-parser`'s accepted forms), fall back to representing dependencies as a comment line `//  X depends on Y` and drop the `dependency from` assertion in the test. The round-trip-parses requirement is the hard constraint; the exact dependency syntax is not.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p ufo-types --features sysml emit_kerml`
Expected: PASS (both tests).

- [ ] **Step 5: Update CHANGELOG + version**

`CHANGELOG.md` under `## [Unreleased]` → `### Added`:
```
- `sysml_model::emit_kerml(package, elements, relations)` — deterministic
  KerML v2 text emitter for a typed element/relation graph, round-trip
  validated against `sysml-v2-parser` (feature `sysml`).
  Consumed by `elasticdotventures/_b00t_` SP5 and `dispatch_sysml.rs`.
```
`Cargo.toml`: bump `version = "0.14.0"` → `version = "0.14.1"`.

- [ ] **Step 6: Commit + open the PR**

```bash
git add src/sysml_model.rs CHANGELOG.md Cargo.toml
git commit -m "feat(sysml_model): emit_kerml — deterministic KerML v2 text emitter"
git push --no-verify -u origin feat/sysml-model-emit-kerml
gh pr create --repo PromptExecution/ufo-types --base main \
  --title "feat(sysml_model): emit_kerml — deterministic KerML v2 emitter" \
  --body "Native procedural KerML v2 text generation for a typed element/relation graph. Round-trip validated against sysml-v2-parser. Needed by elasticdotventures/_b00t_ SP5 (graph->KerML view) and adopted by b00t-cli/src/dispatch_sysml.rs."
```

- [ ] **Step 7: Merge the PR once CI is green**

```bash
gh pr checks <N> --repo PromptExecution/ufo-types --watch
gh pr merge <N> --repo PromptExecution/ufo-types --squash --admin --delete-branch
```

- [ ] **Step 8: Bump the b00t-side pin** — in the b00t workspace root `Cargo.toml`, update the `ufo-types` dependency to `= "0.14.1"` (or the git rev of the merge commit). Commit on `spec/sp5-sysml-coherence`:

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump ufo-types to 0.14.1 (emit_kerml)"
```

---

### Task 2: `b00t-c0re-lib` — `OxigraphStore::store()` accessor + `graph_load::load_graph`

**Files:**
- Modify: `b00t-c0re-lib/src/irontology_bridge.rs` (add a `pub fn store()` accessor to `impl OxigraphStore`, under the existing `#[cfg(feature = "store-oxigraph")]`)
- Create: `b00t-c0re-lib/src/graph_load.rs`
- Modify: `b00t-c0re-lib/src/lib.rs` (add `#[cfg(feature = "store-oxigraph")] pub mod graph_load;`)

**Interfaces:**
- Consumes: `crate::irontology_bridge::OxigraphStore` (existing); `oxigraph::store::Store`, `oxigraph::model::{NamedNode, Literal, Quad, Term, GraphName}`.
- Produces:
  - `impl OxigraphStore { pub fn store(&self) -> &oxigraph::store::Store }`
  - `pub const B00T_NS: &str = "http://b00t.promptexecution.com/ontology#";`
  - `pub const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";`
  - `pub fn expand_iri(term: &str) -> String`
  - `pub fn load_graph(store: &OxigraphStore, triples: &[(String, String, String)]) -> anyhow::Result<usize>`

- [ ] **Step 1: Add the `store()` accessor** — in `b00t-c0re-lib/src/irontology_bridge.rs`, inside `#[cfg(feature = "store-oxigraph")] impl OxigraphStore { ... }` (the block that currently has `fact_to_quad` / `edge_to_fact`), add:

```rust
    /// Borrow the underlying oxigraph store for direct SPARQL / quad access
    /// (SP5 graph substrate).
    pub fn store(&self) -> &oxigraph::store::Store {
        &self.store
    }
```

- [ ] **Step 2: Write the failing test** — create `b00t-c0re-lib/src/graph_load.rs` with only the test:

```rust
//! SP5-01 — assert a plain SPO triple set into an `OxigraphStore`.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irontology_bridge::OxigraphStore;
    use crate::irontology_bridge::{KnowledgeStoreBackend, StoreConfig};

    fn temp_store() -> OxigraphStore {
        let dir = std::env::temp_dir().join(format!("sp5-load-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        OxigraphStore::try_new(StoreConfig {
            endpoint: String::new(),
            namespace: "test".into(),
            data_path: Some(dir),
        })
        .unwrap()
    }

    #[test]
    fn expand_iri_maps_known_prefixes() {
        assert_eq!(
            expand_iri("b00t:datum/rust.cli"),
            "http://b00t.promptexecution.com/ontology#datum/rust.cli"
        );
        assert_eq!(
            expand_iri("rdfs:label"),
            "http://www.w3.org/2000/01/rdf-schema#label"
        );
        assert_eq!(expand_iri("http://example.com/x"), "http://example.com/x");
    }

    #[test]
    fn load_graph_asserts_quads_and_is_idempotent() {
        let store = temp_store();
        let triples = vec![
            (
                "b00t:datum/a".to_string(),
                "b00t:dependsOn".to_string(),
                "b00t:datum/b".to_string(),
            ),
            (
                "b00t:datum/a".to_string(),
                "rdfs:label".to_string(),
                "Datum A".to_string(),
            ),
        ];
        let n1 = load_graph(&store, &triples).unwrap();
        assert_eq!(n1, 2);
        // idempotent — re-asserting the same triples adds nothing new
        load_graph(&store, &triples).unwrap();
        let total = store.store().len().unwrap();
        assert_eq!(total, 2);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run (delegate to pi on sm3lly, or CI): `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph graph_load`
Expected: FAIL — `cannot find function load_graph` / `expand_iri`.

- [ ] **Step 4: Write the implementation** `[PI-CANDIDATE]` — prepend to `b00t-c0re-lib/src/graph_load.rs` (above the test module):

```rust
use crate::irontology_bridge::OxigraphStore;
use anyhow::{Context, Result};
use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Term};

pub const B00T_NS: &str = "http://b00t.promptexecution.com/ontology#";
pub const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";

/// Expand a CURIE-ish term to a full IRI. `b00t:foo` and `rdfs:foo` map to
/// the b00t / RDFS namespaces; anything already starting with `http` is
/// returned unchanged; everything else is treated as a bare b00t-namespace
/// local name.
pub fn expand_iri(term: &str) -> String {
    if let Some(rest) = term.strip_prefix("b00t:") {
        format!("{B00T_NS}{rest}")
    } else if let Some(rest) = term.strip_prefix("rdfs:") {
        format!("{RDFS_NS}{rest}")
    } else if term.starts_with("http://") || term.starts_with("https://") {
        term.to_string()
    } else {
        format!("{B00T_NS}{term}")
    }
}

/// Is this object position an IRI (a node reference) or a plain literal?
/// A term is an IRI iff it carries a known prefix or an http scheme.
fn object_is_iri(o: &str) -> bool {
    o.starts_with("b00t:")
        || o.starts_with("rdfs:")
        || o.starts_with("http://")
        || o.starts_with("https://")
}

/// Assert every `(s, p, o)` triple into `store`'s default graph.
///
/// `s` and `p` are always IRIs (expanded via [`expand_iri`]). `o` is an IRI
/// if [`object_is_iri`] says so, else a plain string literal. Returns the
/// number of triples processed (not the net new count — oxigraph dedups on
/// insert, so re-running is a no-op).
pub fn load_graph(store: &OxigraphStore, triples: &[(String, String, String)]) -> Result<usize> {
    for (s, p, o) in triples {
        let subject = NamedNode::new(expand_iri(s)).with_context(|| format!("bad subject {s}"))?;
        let predicate =
            NamedNode::new(expand_iri(p)).with_context(|| format!("bad predicate {p}"))?;
        let object: Term = if object_is_iri(o) {
            NamedNode::new(expand_iri(o))
                .with_context(|| format!("bad object iri {o}"))?
                .into()
        } else {
            Literal::new_simple_literal(o.as_str()).into()
        };
        let quad = Quad::new(subject, predicate, object, GraphName::DefaultGraph);
        store
            .store()
            .insert(&quad)
            .with_context(|| format!("insert ({s} {p} {o})"))?;
    }
    Ok(triples.len())
}
```

- [ ] **Step 5: Register the module** — in `b00t-c0re-lib/src/lib.rs`, next to the other `#[cfg(feature = "store-oxigraph")]` items (near `pub mod irontology_bridge;`), add:

```rust
#[cfg(feature = "store-oxigraph")]
pub mod graph_load;
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph graph_load`
Expected: PASS (3 tests).

- [ ] **Step 7: Commit**

```bash
git add b00t-c0re-lib/src/irontology_bridge.rs b00t-c0re-lib/src/graph_load.rs b00t-c0re-lib/src/lib.rs
git commit -m "feat(sp5-01): OxigraphStore::store() accessor + graph_load::load_graph"
```

---

### Task 3: `b00t-c0re-lib` — finish `OxigraphSparqlSource::query`

**Files:**
- Modify: `b00t-c0re-lib/src/query_bus.rs` (`OxigraphSparqlSource` struct + `impl QuerySource` + tests, all under the existing `#[cfg(feature = "store-oxigraph")]`)

**Interfaces:**
- Consumes: `crate::graph_load::{load_graph, expand_iri, B00T_NS}` (Task 2); `crate::irontology_bridge::{OxigraphStore, KnowledgeStoreBackend, StoreConfig}`; `crate::query_bus::{QueryContext, QueryResult, QuerySource, TrustGrade}` (existing).
- Produces:
  - `OxigraphSparqlSource { store: std::sync::Arc<OxigraphStore>, query_template: String, depth: usize }`
  - `impl OxigraphSparqlSource { pub fn new(store: Arc<OxigraphStore>) -> Self; pub fn from_env() -> anyhow::Result<Self> }`

- [ ] **Step 1: Write the failing test** — replace the existing `#[cfg(feature = "store-oxigraph")]` stub test block (if any) / add to the `#[cfg(test)] mod tests` in `query_bus.rs`:

```rust
#[cfg(all(test, feature = "store-oxigraph"))]
mod oxigraph_source_tests {
    use super::*;
    use crate::graph_load::load_graph;
    use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};
    use std::sync::Arc;

    fn store_with(triples: &[(&str, &str, &str)]) -> Arc<OxigraphStore> {
        let dir = std::env::temp_dir().join(format!("sp5-src-{}", std::process::id()));
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph oxigraph_source`
Expected: FAIL — `OxigraphSparqlSource::new` not found / `query` returns `vec![]` so the second test fails on `keys.contains`.

- [ ] **Step 3: Write the implementation** `[PI-CANDIDATE]` — replace the existing `#[cfg(feature = "store-oxigraph")] pub struct OxigraphSparqlSource { ... }` and its `impl QuerySource` in `query_bus.rs` with:

```rust
#[cfg(feature = "store-oxigraph")]
pub struct OxigraphSparqlSource {
    store: std::sync::Arc<crate::irontology_bridge::OxigraphStore>,
    /// SPARQL SELECT with a single `{SUBJECT}` placeholder for the topic IRI.
    query_template: String,
    /// Max property-path traversal depth (informational; the template uses `+`).
    depth: usize,
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
    pub fn from_env() -> anyhow::Result<Self> {
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
const DEFAULT_ADJACENCY_TEMPLATE: &str = r#"
PREFIX b00t: <http://b00t.promptexecution.com/ontology#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
SELECT ?adj ?label WHERE {
  {SUBJECT} (b00t:dependsOn|b00t:hasPart|b00t:relatedTo)+ ?adj .
  OPTIONAL { ?adj rdfs:label ?label }
} LIMIT 40
"#;

#[cfg(feature = "store-oxigraph")]
#[async_trait]
impl QuerySource for OxigraphSparqlSource {
    fn name(&self) -> &'static str {
        "oxigraph:sparql"
    }
    fn weight(&self) -> u32 {
        2
    }

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
                let key = adj
                    .rsplit(['#', '/'])
                    .next()
                    .unwrap_or(&adj)
                    .to_string();
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
        let _ = self.depth; // reserved for a future bounded-length variant
        Ok(out)
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph oxigraph_source`
Expected: PASS (2 tests).

- [ ] **Step 5: Verify the default build is untouched**

Run: `cargo check -p b00t-c0re-lib` (default features — `store-helixdb`)
Expected: compiles; `OxigraphSparqlSource` is `#[cfg]`-excluded so nothing changed for the default build.

- [ ] **Step 6: Commit**

```bash
git add b00t-c0re-lib/src/query_bus.rs
git commit -m "feat(sp5-03): OxigraphSparqlSource::query — real property-path SPARQL + from_env"
```

---

### Task 4: `b00t-c0re-lib` — SHACL-lite shape suite

**Files:**
- Create: `b00t-c0re-lib/src/graph_shapes.rs`
- Modify: `b00t-c0re-lib/src/lib.rs` (`#[cfg(feature = "store-oxigraph")] pub mod graph_shapes;`)

**Interfaces:**
- Consumes: `crate::irontology_bridge::OxigraphStore` + `.store()` (Task 2); `oxigraph::sparql::QueryResults`.
- Produces:
  - `pub enum Severity { Violation, Warning }`
  - `pub struct ShapeViolation { pub shape: &'static str, pub focus_node: String, pub message: String, pub severity: Severity }`
  - `pub fn validate_graph(store: &OxigraphStore) -> anyhow::Result<Vec<ShapeViolation>>`

- [ ] **Step 1: Write the failing test** — create `b00t-c0re-lib/src/graph_shapes.rs` with the test only:

```rust
//! SP5-04 — SHACL-lite: a fixed set of SPARQL constraint queries over the
//! loaded b00t graph, run as a CI gate.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_load::load_graph;
    use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};

    fn store_with(triples: &[(&str, &str, &str)]) -> OxigraphStore {
        let dir = std::env::temp_dir().join(format!("sp5-shape-{}-{}", std::process::id(), triples.len()));
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
        s
    }

    #[test]
    fn clean_graph_has_no_violations() {
        let s = store_with(&[
            ("b00t:tenant/pe", "b00t:hasR0le", "b00t:r0le/pe/worker"),
            ("b00t:r0le/pe/worker", "b00t:grantsShard", "b00t:shard/datum/all"),
            ("b00t:shard/datum/all", "b00t:shardMode", "rw"),
            ("b00t:r0le/pe/worker", "b00t:grantsShard", "b00t:shard/project/all"),
            ("b00t:shard/project/all", "b00t:shardMode", "rw"),
        ]);
        let v = validate_graph(&s).unwrap();
        assert!(v.iter().all(|x| x.severity == Severity::Warning), "{v:?}");
    }

    #[test]
    fn detects_a_dependson_cycle() {
        let s = store_with(&[
            ("b00t:datum/a", "b00t:dependsOn", "b00t:datum/b"),
            ("b00t:datum/b", "b00t:dependsOn", "b00t:datum/a"),
        ]);
        let v = validate_graph(&s).unwrap();
        assert!(v.iter().any(|x| x.shape == "dependsOn-acyclic" && x.severity == Severity::Violation));
    }

    #[test]
    fn detects_a_budgetless_service_and_a_role_missing_rw() {
        let s = store_with(&[
            ("b00t:svc/gh", "b00t:image", "ghcr.io/x/gh@sha256:deadbeef"),
            ("b00t:tenant/pe", "b00t:hasR0le", "b00t:r0le/pe/thin"),
            ("b00t:r0le/pe/thin", "b00t:grantsShard", "b00t:shard/tool/all"),
            ("b00t:shard/tool/all", "b00t:shardMode", "r"),
        ]);
        let v = validate_graph(&s).unwrap();
        assert!(v.iter().any(|x| x.shape == "every-svc-has-a-budget" && x.severity == Severity::Violation));
        assert!(v.iter().any(|x| x.shape == "r0le-rw-datum" && x.severity == Severity::Violation));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph graph_shapes`
Expected: FAIL — `validate_graph` / `Severity` not found.

- [ ] **Step 3: Write the implementation** `[PI-CANDIDATE]` — prepend to `b00t-c0re-lib/src/graph_shapes.rs`:

```rust
use crate::irontology_bridge::OxigraphStore;
use anyhow::Result;
use oxigraph::sparql::QueryResults;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Violation,
    Warning,
}

#[derive(Debug, Clone)]
pub struct ShapeViolation {
    pub shape: &'static str,
    pub focus_node: String,
    pub message: String,
    pub severity: Severity,
}

const PREFIXES: &str = r#"
PREFIX b00t: <http://b00t.promptexecution.com/ontology#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
"#;

/// Each shape is a SPARQL query. An `ASK`-style shape (`kind = AskFalse`)
/// fails if the query returns `true`. A `SELECT`-style shape (`kind = SelectAny`)
/// emits one violation per returned row, binding `?focus` and optionally `?msg`.
enum ShapeKind {
    AskFalse,
    SelectAny,
}

struct Shape {
    name: &'static str,
    kind: ShapeKind,
    severity: Severity,
    sparql: &'static str,
    message: &'static str,
}

const SHAPES: &[Shape] = &[
    Shape {
        name: "dependsOn-acyclic",
        kind: ShapeKind::AskFalse,
        severity: Severity::Violation,
        sparql: "ASK { ?x b00t:dependsOn+ ?x }",
        message: "b00t:dependsOn graph contains a cycle",
    },
    Shape {
        name: "every-svc-has-a-budget",
        kind: ShapeKind::SelectAny,
        severity: Severity::Violation,
        sparql: "SELECT ?focus WHERE { ?focus b00t:image ?i . FILTER NOT EXISTS { ?focus b00t:budgetCeiling ?b } }",
        message: "MCP service has an image but no b00t:budgetCeiling",
    },
    Shape {
        name: "every-r0le-has-a-tenant",
        kind: ShapeKind::SelectAny,
        severity: Severity::Violation,
        sparql: "SELECT ?focus WHERE { ?focus b00t:grantsShard ?s . FILTER NOT EXISTS { ?t b00t:hasR0le ?focus } }",
        message: "r0le grants shards but no tenant b00t:hasR0le it",
    },
    Shape {
        name: "r0le-rw-datum",
        kind: ShapeKind::SelectAny,
        severity: Severity::Violation,
        sparql: "SELECT ?focus WHERE { ?t b00t:hasR0le ?focus . FILTER NOT EXISTS { ?focus b00t:grantsShard ?sh . ?sh b00t:shardMode \"rw\" . FILTER(CONTAINS(STR(?sh), \"/shard/datum/\")) } }",
        message: "r0le has no rw grant on a datum shard",
    },
    Shape {
        name: "r0le-rw-soulscope",
        kind: ShapeKind::SelectAny,
        severity: Severity::Violation,
        sparql: "SELECT ?focus WHERE { ?t b00t:hasR0le ?focus . FILTER NOT EXISTS { ?focus b00t:grantsShard ?sh . ?sh b00t:shardMode \"rw\" . FILTER(REGEX(STR(?sh), \"/shard/(project|system|agent|skill|tool)/\")) } }",
        message: "r0le has no rw grant on any soulscope shard",
    },
    Shape {
        name: "no-cross-tenant-datum-key",
        kind: ShapeKind::SelectAny,
        severity: Severity::Warning,
        sparql: "SELECT ?focus WHERE { ?t1 b00t:hasR0le ?focus . ?t2 b00t:hasR0le ?focus . FILTER(?t1 != ?t2) }",
        message: "same r0le node reachable from two tenants",
    },
];

fn local_name(iri: &str) -> String {
    iri.rsplit(['#', '/']).next().unwrap_or(iri).to_string()
}

/// Run every shape against the loaded graph. Returns all violations
/// (Violation + Warning severities); callers decide the exit policy.
pub fn validate_graph(store: &OxigraphStore) -> Result<Vec<ShapeViolation>> {
    let mut out = Vec::new();
    for shape in SHAPES {
        let q = format!("{PREFIXES}\n{}", shape.sparql);
        let results = store.store().query(&q)?;
        match (&shape.kind, results) {
            (ShapeKind::AskFalse, QueryResults::Boolean(true)) => out.push(ShapeViolation {
                shape: shape.name,
                focus_node: String::new(),
                message: shape.message.to_string(),
                severity: shape.severity,
            }),
            (ShapeKind::AskFalse, _) => {}
            (ShapeKind::SelectAny, QueryResults::Solutions(sols)) => {
                for sol in sols {
                    let sol = sol?;
                    let focus = match sol.get("focus") {
                        Some(oxigraph::model::Term::NamedNode(n)) => local_name(n.as_str()),
                        Some(t) => t.to_string(),
                        None => String::new(),
                    };
                    out.push(ShapeViolation {
                        shape: shape.name,
                        focus_node: focus,
                        message: shape.message.to_string(),
                        severity: shape.severity,
                    });
                }
            }
            (ShapeKind::SelectAny, _) => {}
        }
    }
    out.sort_by(|a, b| (a.shape, &a.focus_node).cmp(&(b.shape, &b.focus_node)));
    Ok(out)
}
```

- [ ] **Step 4: Register the module** — in `b00t-c0re-lib/src/lib.rs`:

```rust
#[cfg(feature = "store-oxigraph")]
pub mod graph_shapes;
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph graph_shapes`
Expected: PASS (3 tests). If the `r0le-rw-*` SPARQL `REGEX`/`CONTAINS` filters misbehave in oxigraph 0.5.8, adjust the filter to `STRSTARTS`/`STRENDS` equivalents and re-run — the shape's intent (a `rw` grant whose shard IRI is under `/shard/datum/` or a soulscope kind) is the contract.

- [ ] **Step 6: Commit**

```bash
git add b00t-c0re-lib/src/graph_shapes.rs b00t-c0re-lib/src/lib.rs
git commit -m "feat(sp5-04): graph_shapes — SHACL-lite validate_graph (6 shapes incl. r0le rw datum+soulscope)"
```

---

### Task 5: `b00t-c0re-lib` — graph → KerML view + `dispatch_sysml` refactor

**Depends on Task 1 merged and pinned (Task 1 Step 8).**

**Files:**
- Create: `b00t-c0re-lib/src/graph_kerml.rs`
- Modify: `b00t-c0re-lib/src/lib.rs` (`#[cfg(feature = "store-oxigraph")] pub mod graph_kerml;`)
- Modify: `b00t-cli/src/dispatch_sysml.rs` (replace the private `dispatch_chain_to_sysml_v2` body with a call to `ufo_types::sysml_model::emit_kerml`)

**Interfaces:**
- Consumes: `crate::irontology_bridge::OxigraphStore` + `.store()`; `ufo_types::sysml_model::{ElementId, ElementKind, Relation, emit_kerml}` (Task 1); `ufo_types::sysml::validate_sysml_v2` (`#[cfg(feature = "sysml")]` on `ufo-types`); `oxigraph::sparql::QueryResults`.
- Produces:
  - `pub enum GraphView { DatumDeps, R0leGrants, ServiceBudgets }`
  - `pub fn graph_to_kerml(store: &OxigraphStore, view: GraphView) -> anyhow::Result<String>`

- [ ] **Step 1: Confirm `ufo-types` deps** — `b00t-c0re-lib/Cargo.toml` must have `ufo-types` with the `sysml` feature available for the round-trip assert. Check:

Run: `grep -n 'ufo-types' b00t-c0re-lib/Cargo.toml`
If `ufo-types` is present without `features = ["sysml"]`, add it:
```toml
ufo-types = { workspace = true, features = ["sysml"] }
```
Commit that line change with a `chore:` message if edited.

- [ ] **Step 2: Write the failing test** — create `b00t-c0re-lib/src/graph_kerml.rs` with the test only:

```rust
//! SP5-05 — project a SPARQL view of the b00t graph into native KerML text.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_load::load_graph;
    use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};

    fn store_with(triples: &[(&str, &str, &str)]) -> OxigraphStore {
        let dir = std::env::temp_dir().join(format!("sp5-kerml-{}", std::process::id()));
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
        s
    }

    #[test]
    fn datum_deps_view_emits_parts_and_round_trips() {
        let s = store_with(&[
            ("b00t:datum/rust.cli", "b00t:dependsOn", "b00t:datum/cargo"),
            ("b00t:datum/cargo", "b00t:dependsOn", "b00t:datum/rustup"),
        ]);
        let kerml = graph_to_kerml(&s, GraphView::DatumDeps).unwrap();
        assert!(kerml.contains("part def cargo;"));
        assert!(kerml.contains("part def rustup;"));
        assert!(kerml.contains("part def rust.cli;") || kerml.contains("part def rust_cli;"));
        // deterministic
        assert_eq!(kerml, graph_to_kerml(&s, GraphView::DatumDeps).unwrap());
        ufo_types::sysml::validate_sysml_v2(&kerml).expect("kerml must parse");
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph graph_kerml`
Expected: FAIL — `graph_to_kerml` / `GraphView` not found.

- [ ] **Step 4: Write the implementation** `[PI-CANDIDATE]` — prepend to `b00t-c0re-lib/src/graph_kerml.rs`:

```rust
use crate::irontology_bridge::OxigraphStore;
use anyhow::Result;
use oxigraph::sparql::QueryResults;
use ufo_types::sysml_model::{emit_kerml, ElementId, ElementKind, Relation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphView {
    /// datum -> datum `b00t:dependsOn` edges.
    DatumDeps,
    /// r0le -> tool-allowlist / shard-grant edges.
    R0leGrants,
    /// MCP service -> budget/image facts.
    ServiceBudgets,
}

const PREFIXES: &str = r#"
PREFIX b00t: <http://b00t.promptexecution.com/ontology#>
"#;

impl GraphView {
    fn sparql(&self) -> &'static str {
        match self {
            GraphView::DatumDeps => "SELECT ?s ?o WHERE { ?s b00t:dependsOn ?o }",
            GraphView::R0leGrants => {
                "SELECT ?s ?o WHERE { ?s b00t:grantsShard ?o } UNION { ?s b00t:allowsTool ?o }"
            }
            GraphView::ServiceBudgets => {
                "SELECT ?s ?o WHERE { ?s b00t:budgetCeiling ?o } UNION { ?s b00t:image ?o }"
            }
        }
    }
    fn package(&self) -> &'static str {
        match self {
            GraphView::DatumDeps => "b00t_datum_deps",
            GraphView::R0leGrants => "b00t_r0le_grants",
            GraphView::ServiceBudgets => "b00t_service_budgets",
        }
    }
}

fn local_name(iri_or_lit: &str) -> String {
    iri_or_lit
        .rsplit(['#', '/'])
        .next()
        .unwrap_or(iri_or_lit)
        .to_string()
}

/// Sanitize a graph local-name into a KerML identifier: KerML unquoted
/// names can't contain `.`; replace with `_`. (Quoted names `'a.b'` are an
/// alternative if the parser accepts them — verify; `_` is the safe default.)
fn ident(name: &str) -> String {
    name.replace(['.', '-', ' ', ':'], "_")
}

/// Run `view`'s SELECT, build a typed element/relation graph, and render it
/// as KerML via `ufo_types::sysml_model::emit_kerml`. The output is
/// round-trip-validated in debug builds.
pub fn graph_to_kerml(store: &OxigraphStore, view: GraphView) -> Result<String> {
    let q = format!("{PREFIXES}\n{}", view.sparql());
    let results = store.store().query(&q)?;

    let mut node_names: std::collections::BTreeSet<String> = Default::default();
    let mut relations: Vec<Relation> = Vec::new();

    if let QueryResults::Solutions(sols) = results {
        for sol in sols {
            let sol = sol?;
            let s = match sol.get("s") {
                Some(t) => local_name(&t.to_string().trim_matches(['<', '>', '"']).to_string()),
                None => continue,
            };
            let o = match sol.get("o") {
                Some(oxigraph::model::Term::NamedNode(n)) => local_name(n.as_str()),
                Some(oxigraph::model::Term::Literal(l)) => local_name(l.value()),
                _ => continue,
            };
            let (s, o) = (ident(&s), ident(&o));
            node_names.insert(s.clone());
            node_names.insert(o.clone());
            relations.push(Relation::Dependency {
                client: ElementId::new(s),
                supplier: ElementId::new(o),
            });
        }
    }

    let elements: Vec<(ElementId, ElementKind)> = node_names
        .into_iter()
        .map(|n| (ElementId::new(n), ElementKind::PartDefinition))
        .collect();

    let text = emit_kerml(view.package(), &elements, &relations);
    debug_assert!(
        ufo_types::sysml::validate_sysml_v2(&text).is_ok(),
        "graph_to_kerml produced invalid KerML:\n{text}"
    );
    Ok(text)
}
```

> Note: the test asserts `part def rust.cli;` OR `part def rust_cli;`. The `ident()` sanitizer makes it `rust_cli` — keep the test's OR so the plan stays robust if a later change quotes names instead.

- [ ] **Step 5: Register the module** — `b00t-c0re-lib/src/lib.rs`:

```rust
#[cfg(feature = "store-oxigraph")]
pub mod graph_kerml;
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p b00t-c0re-lib --no-default-features --features store-oxigraph graph_kerml`
Expected: PASS.

- [ ] **Step 7: Refactor `dispatch_sysml.rs` to use the shared emitter**

In `b00t-cli/src/dispatch_sysml.rs`, replace the body of `pub fn dispatch_chain_to_sysml_v2() -> String` with:

```rust
pub fn dispatch_chain_to_sysml_v2() -> String {
    use ufo_types::sysml_model::{emit_kerml, ElementId, ElementKind, Relation};
    let (nodes, edges) = dispatch_chain_iso_ir();
    let elements: Vec<(ElementId, ElementKind)> = nodes
        .iter()
        .map(|n| (ElementId::new(n.id.clone()), ElementKind::PartDefinition))
        .collect();
    let relations: Vec<Relation> = edges
        .iter()
        .map(|e| Relation::Dependency {
            client: ElementId::new(e.to.clone()),
            supplier: ElementId::new(e.from.clone()),
        })
        .collect();
    emit_kerml("b00t_dispatch_chain", &elements, &relations)
}
```

Keep the existing `#[cfg(test)]` round-trip test for this function unchanged — it must still pass (it validates against `sysml-v2-parser`). If the existing test asserts on specific `:>` specialization syntax the old hand-roll produced, relax it to assert `validate_sysml_v2(&out).is_ok()` + `out.contains("part def <mode-name>")` for each mode.

- [ ] **Step 8: Verify the b00t-cli default build + the dispatch test**

Run: `cargo test -p b00t-cli dispatch_sysml`
Expected: PASS. (`b00t-cli` default build — no `store-oxigraph`.)

- [ ] **Step 9: Commit**

```bash
git add b00t-c0re-lib/src/graph_kerml.rs b00t-c0re-lib/src/lib.rs b00t-c0re-lib/Cargo.toml b00t-cli/src/dispatch_sysml.rs
git commit -m "feat(sp5-05): graph_to_kerml view projection + dispatch_sysml uses shared ufo_types::emit_kerml"
```

---

### Task 6: `b00t-cli` — `compile_identity_triples`

**Files:**
- Create: `b00t-cli/src/identity_triples.rs`
- Modify: `b00t-cli/src/lib.rs` (`pub mod identity_triples;`)

**Interfaces:**
- Consumes: `crate::datum_utils::get_all_datums_for_tenant` (sig: `(b00t_path: &str, tenant: Option<&str>, max_depth: Option<usize>) -> Result<HashMap<String, (BootDatum, String)>>`); `crate::boot_datum::BootDatum`; `crate::datum_types::DatumType`; `crate::datum_agent_profile::AgentProfileSpec` (fields: `tool_allowlist: Vec<String>`, `skills: Vec<String>`, `soul_shard_grants: Vec<SoulShardGrant>`, `model_tier: ModelTier`, `budget_ceiling: u64`, `permissions: Vec<String>`, `signature: Option<DatumSignature>`); `crate::datum_agent_profile::{SoulShardGrant, ShardMode}`; `crate::soul_scope::ShardKind`.
- Produces: `pub fn compile_identity_triples(b00t_path: &str, tenant: Option<&str>) -> anyhow::Result<Vec<(String, String, String)>>`

- [ ] **Step 1: Confirm how an `AgentProfile` datum exposes its `AgentProfileSpec`** — before writing, check:

Run: `grep -n 'agent_profile\|AgentProfileSpec\|fn.*agent_profile' b00t-cli/src/boot_datum.rs b00t-cli/src/datum_agent_profile.rs`
You need the accessor that turns a `BootDatum` (with `datum_type == Some(DatumType::AgentProfile)`) into an `AgentProfileSpec` — likely `datum.agent_profile: Option<AgentProfileSpec>` on `BootDatum` (mirrors SP4's `datum.mcp_server`) or a `AgentProfileSpec::from_datum(&BootDatum)`. Use whichever exists. The steps below assume `datum.agent_profile: Option<AgentProfileSpec>` and `datum.mcp_server: Option<McpServerSpec>` (SP4) — adjust field/method names to match reality; the triple shapes are the contract.

- [ ] **Step 2: Write the failing test** — create `b00t-cli/src/identity_triples.rs` with the test only:

```rust
//! SP5-02 — compile the tenant/r0le/tool/grant/budget graph as SPO triples.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("worker.agentprofile.toml");
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(
            f,
            r#"
[b00t]
name = "worker"
type = "r0le"

[b00t.agent_profile]
tool_allowlist = ["cargo.*", "b00t_status"]
skills = ["rust"]
model_tier = "ch0nky"
budget_ceiling = 1000
permissions = []

[[b00t.agent_profile.soul_shard_grants]]
kind = "datum"
id = "*"
mode = "rw"

[[b00t.agent_profile.soul_shard_grants]]
kind = "project"
id = "*"
mode = "rw"
"#
        )
        .unwrap();
        dir
    }

    #[test]
    fn emits_tenant_role_tool_and_grant_triples() {
        let dir = fixture();
        let triples =
            compile_identity_triples(dir.path().to_str().unwrap(), None).unwrap();
        let has = |s: &str, p: &str, o: &str| {
            triples.iter().any(|(a, b, c)| a == s && b == p && c == o)
        };
        assert!(has("b00t:tenant/_base", "b00t:hasR0le", "b00t:r0le/_base/worker"));
        assert!(has("b00t:r0le/_base/worker", "b00t:allowsTool", "cargo.*"));
        assert!(has(
            "b00t:r0le/_base/worker",
            "b00t:grantsShard",
            "b00t:shard/datum/*"
        ));
        assert!(has("b00t:shard/datum/*", "b00t:shardMode", "rw"));
        assert!(has("b00t:r0le/_base/worker", "b00t:modelTier", "ch0nky"));
        assert!(has("b00t:r0le/_base/worker", "b00t:budgetCeiling", "1000"));
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p b00t-cli identity_triples`
Expected: FAIL — `compile_identity_triples` not found.

- [ ] **Step 4: Write the implementation** `[PI-CANDIDATE]` — prepend to `b00t-cli/src/identity_triples.rs`:

```rust
use crate::datum_types::DatumType;
use crate::datum_utils::get_all_datums_for_tenant;
use anyhow::Result;

/// Compile the identity/authz overlay as `b00t:`-namespace SPO triples:
/// tenant -> r0le -> {tool allowlist, soul-shard grants, budget, tier}, plus
/// MCP-service -> {budget, image}. `tenant == None` scans the base tree and
/// tags everything `_base`.
pub fn compile_identity_triples(
    b00t_path: &str,
    tenant: Option<&str>,
) -> Result<Vec<(String, String, String)>> {
    let t = tenant.unwrap_or("_base");
    let datums = get_all_datums_for_tenant(b00t_path, tenant, Some(6))?;
    let mut out: Vec<(String, String, String)> = Vec::new();

    // deterministic order
    let mut keys: Vec<&String> = datums.keys().collect();
    keys.sort();

    for key in keys {
        let (datum, _path) = &datums[key];

        // ── r0le / AgentProfile ────────────────────────────────────────────
        if datum.datum_type == Some(DatumType::AgentProfile) {
            if let Some(spec) = &datum.agent_profile {
                let role = key
                    .strip_suffix(".agentprofile")
                    .unwrap_or(key)
                    .to_string();
                let r0le_iri = format!("b00t:r0le/{t}/{role}");
                out.push((
                    format!("b00t:tenant/{t}"),
                    "b00t:hasR0le".into(),
                    r0le_iri.clone(),
                ));
                for tool in &spec.tool_allowlist {
                    out.push((r0le_iri.clone(), "b00t:allowsTool".into(), tool.clone()));
                }
                for g in &spec.soul_shard_grants {
                    let shard = format!("b00t:shard/{}/{}", g.kind.as_str(), g.id);
                    out.push((r0le_iri.clone(), "b00t:grantsShard".into(), shard.clone()));
                    let mode = match g.mode {
                        crate::datum_agent_profile::ShardMode::Rw => "rw",
                        crate::datum_agent_profile::ShardMode::R => "r",
                    };
                    out.push((shard, "b00t:shardMode".into(), mode.to_string()));
                }
                out.push((
                    r0le_iri.clone(),
                    "b00t:modelTier".into(),
                    format!("{:?}", spec.model_tier).to_lowercase(),
                ));
                out.push((
                    r0le_iri,
                    "b00t:budgetCeiling".into(),
                    spec.budget_ceiling.to_string(),
                ));
            }
        }

        // ── MCP service (SP4 [b00t.mcp_server]) ────────────────────────────
        if let Some(mcp) = &datum.mcp_server {
            let svc = key.strip_suffix(".mcp_server").unwrap_or(key);
            let svc_iri = format!("b00t:svc/{svc}");
            out.push((svc_iri.clone(), "b00t:image".into(), mcp.image.clone()));
            if let Some(ceiling) = mcp.budget_ceiling {
                out.push((
                    svc_iri,
                    "b00t:budgetCeiling".into(),
                    ceiling.to_string(),
                ));
            }
        }
    }

    out.sort();
    out.dedup();
    Ok(out)
}
```

> Adjust `datum.agent_profile`, `datum.mcp_server`, `spec.model_tier` / `ShardMode` / `mcp.image` / `mcp.budget_ceiling` to the real field names found in Step 1. If `ShardMode` has more than two variants, match them all.

- [ ] **Step 5: Register the module** — `b00t-cli/src/lib.rs`: `pub mod identity_triples;` (near `pub mod datum_triples;`).

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo test -p b00t-cli identity_triples`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add b00t-cli/src/identity_triples.rs b00t-cli/src/lib.rs
git commit -m "feat(sp5-02): compile_identity_triples — tenant/r0le/tool/grant/budget SPO edges"
```

---

### Task 7: `b00t-cli` — `r0le build` grants rw to soulscopes + datums

**Files:**
- Modify: `b00t-cli/src/commands/r0le.rs` (the `build` handler that assembles `soul_shard_grants`)

**Interfaces:**
- Consumes: `crate::soul_scope::ShardKind` (variants: `Project, System, Agent, Skill, Tool, Datum`); `crate::datum_agent_profile::{SoulShardGrant, ShardMode}`.
- Produces: no new public API — a behavior change in the emitted `AgentProfileSpec.soul_shard_grants`.

- [ ] **Step 1: Find the current grant assembly** —

Run: `grep -n 'soul_shard_grants\|SoulShardGrant\|ShardKind::' b00t-cli/src/commands/r0le.rs`
Identify the vec that becomes `spec.soul_shard_grants` (spec SP2-03: currently `{Skill:<s>:r}*` per discovered skill + `{Agent:<role>:rw}`).

- [ ] **Step 2: Write / update the failing test** — in `b00t-cli/src/commands/r0le.rs`'s test module, add:

```rust
#[test]
fn r0le_build_grants_rw_to_every_shard_kind() {
    // <build a minimal AgentProfileSpec via the same path `r0le build` uses>
    let spec = build_agent_profile_spec_for_test("worker", &["rust"]);
    use crate::soul_scope::ShardKind;
    for kind in [
        ShardKind::Project,
        ShardKind::System,
        ShardKind::Agent,
        ShardKind::Skill,
        ShardKind::Tool,
        ShardKind::Datum,
    ] {
        assert!(
            spec.soul_shard_grants.iter().any(|g| {
                g.kind == kind
                    && g.id == "*"
                    && matches!(g.mode, crate::datum_agent_profile::ShardMode::Rw)
            }),
            "missing rw:* grant for {kind:?}"
        );
    }
}
```

(If there is no test-friendly `build_agent_profile_spec_for_test` helper, extract the grant-assembly into a `pub(crate) fn default_shard_grants(role: &str, skills: &[String]) -> Vec<SoulShardGrant>` and test that directly.)

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p b00t-cli r0le_build_grants_rw`
Expected: FAIL — grants for `Project/System/Tool/Datum` at `rw:*` are absent.

- [ ] **Step 4: Implement** — in the grant-assembly code, prepend the six blanket grants:

```rust
use crate::soul_scope::ShardKind;
let mut grants: Vec<SoulShardGrant> = [
    ShardKind::Datum,
    ShardKind::Project,
    ShardKind::System,
    ShardKind::Agent,
    ShardKind::Skill,
    ShardKind::Tool,
]
.into_iter()
.map(|kind| SoulShardGrant {
    kind,
    id: "*".to_string(),
    mode: ShardMode::Rw,
})
.collect();
// existing per-skill / per-role grants still appended below:
grants.extend(existing_grants);
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p b00t-cli r0le::`
Expected: PASS (the new test + the existing `r0le::build` tests — update any that asserted an exact grant count).

- [ ] **Step 6: Commit**

```bash
git add b00t-cli/src/commands/r0le.rs
git commit -m "feat(sp5-02b): r0le build grants rw:* on all six soulscope+datum ShardKinds"
```

---

### Task 8: `b00t-cli` — `b00t graph emit-triples` subcommand

**Files:**
- Create: `b00t-cli/src/commands/graph.rs`
- Modify: `b00t-cli/src/commands/mod.rs` (declare + wire the subcommand)
- Modify: `b00t-cli/src/main.rs` (add `Graph` to the top-level command enum + dispatch)

**Interfaces:**
- Consumes: `crate::datum_triples::compile_datum_triples(&str) -> Result<Vec<(String,String,String)>>`; `crate::identity_triples::compile_identity_triples(&str, Option<&str>) -> Result<Vec<(String,String,String)>>` (Task 6).
- Produces: `b00t graph emit-triples [--tenant <T>] [--out <PATH>]` — JSONL, one `[s,p,o]` array per line.

- [ ] **Step 1: Look at an existing subcommand module for the pattern** —

Run: `sed -n '1,60p' b00t-cli/src/commands/mcp.rs` (or any `commands/*.rs` with a `clap` `Subcommand` enum + `execute`/`execute_async`). Mirror its structure — the enum, the arg structs, how `path` (the `_b00t_` dir) is resolved, how it's registered in `commands/mod.rs` and `main.rs`.

- [ ] **Step 2: Write the failing test** — create `b00t-cli/src/commands/graph.rs`:

```rust
//! `b00t graph` — emit the datum + identity graph as SPO triples (SP5-06).

use anyhow::Result;
use clap::Subcommand;
use std::io::Write;

#[derive(Subcommand, Debug)]
pub enum GraphCommands {
    /// Emit the composed datum + identity/authz graph as JSONL `[s,p,o]`.
    EmitTriples {
        /// Tenant overlay to include (default: base tree only).
        #[arg(long)]
        tenant: Option<String>,
        /// Output file (default: stdout).
        #[arg(long)]
        out: Option<String>,
    },
}

pub fn execute(cmd: &GraphCommands, b00t_path: &str) -> Result<()> {
    match cmd {
        GraphCommands::EmitTriples { tenant, out } => {
            let mut triples = crate::datum_triples::compile_datum_triples(b00t_path)?;
            triples.extend(crate::identity_triples::compile_identity_triples(
                b00t_path,
                tenant.as_deref(),
            )?);
            triples.sort();
            triples.dedup();

            let mut sink: Box<dyn Write> = match out {
                Some(p) => Box::new(std::fs::File::create(p)?),
                None => Box::new(std::io::stdout()),
            };
            for (s, p, o) in &triples {
                writeln!(sink, "{}", serde_json::to_string(&[s, p, o])?)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_triples_writes_jsonl_arrays() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("rust.cli.toml"),
            "[b00t]\nname = \"rust.cli\"\ntype = \"cli\"\ndepends_on = [\"cargo\"]\n",
        )
        .unwrap();
        let out = dir.path().join("g.jsonl");
        execute(
            &GraphCommands::EmitTriples {
                tenant: None,
                out: Some(out.to_string_lossy().into()),
            },
            dir.path().to_str().unwrap(),
        )
        .unwrap();
        let body = std::fs::read_to_string(&out).unwrap();
        assert!(body.lines().all(|l| {
            let v: Vec<String> = serde_json::from_str(l).unwrap();
            v.len() == 3
        }));
        assert!(body.contains("b00t:dependsOn"));
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p b00t-cli commands::graph`
Expected: FAIL — module not declared / not compiled.

- [ ] **Step 4: Wire it in** —
- `b00t-cli/src/commands/mod.rs`: add `pub mod graph;`
- `b00t-cli/src/main.rs`: add a `Graph { #[command(subcommand)] cmd: commands::graph::GraphCommands }` variant to the top-level `Commands` enum, and in the dispatch `match`: `Commands::Graph { cmd } => commands::graph::execute(cmd, &path)?,` (use the same `path` resolution the sibling commands use).

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p b00t-cli commands::graph`
Expected: PASS.

- [ ] **Step 6: Smoke-test against the real repo** (delegate to pi on sm3lly or CI):

Run: `cargo run -p b00t-cli -- graph emit-triples --out /tmp/g.jsonl && head -3 /tmp/g.jsonl && wc -l /tmp/g.jsonl`
Expected: non-empty JSONL, every line a 3-element array.

- [ ] **Step 7: Commit**

```bash
git add b00t-cli/src/commands/graph.rs b00t-cli/src/commands/mod.rs b00t-cli/src/main.rs
git commit -m "feat(sp5-06): b00t graph emit-triples — datum+identity graph as JSONL"
```

---

### Task 9: `b00t-c0re-lib` — `examples/b00t-graph.rs`

**Files:**
- Create: `b00t-c0re-lib/examples/b00t-graph.rs`
- Modify: `b00t-c0re-lib/Cargo.toml` (`[[example]]` with `required-features`)

**Interfaces:**
- Consumes: `b00t_c0re_lib::graph_load::load_graph`; `b00t_c0re_lib::graph_shapes::{validate_graph, Severity}`; `b00t_c0re_lib::graph_kerml::{graph_to_kerml, GraphView}`; `b00t_c0re_lib::irontology_bridge::{OxigraphStore, KnowledgeStoreBackend, StoreConfig}`.
- Produces: a binary `b00t-graph` with subcommands `load|query|validate|kerml`, each taking a triples-JSONL path.

- [ ] **Step 1: Declare the example** — append to `b00t-c0re-lib/Cargo.toml`:

```toml
[[example]]
name = "b00t-graph"
path = "examples/b00t-graph.rs"
required-features = ["store-oxigraph"]
```

Check `b00t-c0re-lib`'s dev-dependencies include `clap` (with `derive`) and `serde_json`. If `clap` is not a dep/dev-dep, add `clap = { version = "4", features = ["derive"] }` under `[dev-dependencies]`.

- [ ] **Step 2: Write the example** `[PI-CANDIDATE]` — create `b00t-c0re-lib/examples/b00t-graph.rs`:

```rust
//! SP5-07 — offline driver for the store-oxigraph graph substrate.
//!
//! Build: `cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph -- <cmd>`
//! Input: a JSONL file of `[subject, predicate, object]` arrays
//!        (produced by `b00t graph emit-triples`).

use anyhow::{Context, Result};
use b00t_c0re_lib::graph_kerml::{graph_to_kerml, GraphView};
use b00t_c0re_lib::graph_load::load_graph;
use b00t_c0re_lib::graph_shapes::{validate_graph, Severity};
use b00t_c0re_lib::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "b00t-graph")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Load a triples JSONL into a fresh store; print the quad count.
    Load { triples: String },
    /// Run a raw SPARQL SELECT/ASK against the loaded triples.
    Query {
        triples: String,
        sparql: String,
        #[arg(long)]
        json: bool,
    },
    /// Run the SHACL-lite shape suite. Exit 1 on any Violation with --strict.
    Validate {
        triples: String,
        #[arg(long)]
        strict: bool,
    },
    /// Emit a KerML view of the graph.
    Kerml {
        triples: String,
        #[arg(long, value_parser = ["datum-deps", "r0le-grants", "service-budgets"])]
        view: String,
    },
}

fn read_triples(path: &str) -> Result<Vec<(String, String, String)>> {
    let body = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
    let mut out = Vec::new();
    for (i, line) in body.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let v: Vec<String> = serde_json::from_str(line)
            .with_context(|| format!("{path}:{} not a JSON [s,p,o] array", i + 1))?;
        anyhow::ensure!(v.len() == 3, "{path}:{} expected 3 elements", i + 1);
        out.push((v[0].clone(), v[1].clone(), v[2].clone()));
    }
    Ok(out)
}

fn fresh_store() -> Result<OxigraphStore> {
    let dir = std::env::temp_dir().join(format!("b00t-graph-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    OxigraphStore::try_new(StoreConfig {
        endpoint: String::new(),
        namespace: "cli".into(),
        data_path: Some(dir),
    })
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Load { triples } => {
            let store = fresh_store()?;
            let n = load_graph(&store, &read_triples(&triples)?)?;
            println!("loaded {n} triples ({} quads)", store.store().len().unwrap_or(0));
        }
        Cmd::Query { triples, sparql, json } => {
            let store = fresh_store()?;
            load_graph(&store, &read_triples(&triples)?)?;
            let results = store.store().query(&sparql)?;
            match results {
                oxigraph::sparql::QueryResults::Boolean(b) => println!("{b}"),
                oxigraph::sparql::QueryResults::Solutions(sols) => {
                    for sol in sols {
                        let sol = sol?;
                        if json {
                            let map: std::collections::BTreeMap<String, String> = sol
                                .iter()
                                .map(|(v, t)| (v.as_str().to_string(), t.to_string()))
                                .collect();
                            println!("{}", serde_json::to_string(&map)?);
                        } else {
                            let row: Vec<String> =
                                sol.iter().map(|(v, t)| format!("{}={}", v.as_str(), t)).collect();
                            println!("{}", row.join("\t"));
                        }
                    }
                }
                _ => {}
            }
        }
        Cmd::Validate { triples, strict } => {
            let store = fresh_store()?;
            load_graph(&store, &read_triples(&triples)?)?;
            let violations = validate_graph(&store)?;
            let mut hard = 0;
            for v in &violations {
                let tag = match v.severity {
                    Severity::Violation => {
                        hard += 1;
                        "VIOLATION"
                    }
                    Severity::Warning => "warning",
                };
                println!("[{tag}] {} {} — {}", v.shape, v.focus_node, v.message);
            }
            if violations.is_empty() {
                println!("all shapes pass");
            }
            if strict && hard > 0 {
                std::process::exit(1);
            }
        }
        Cmd::Kerml { triples, view } => {
            let store = fresh_store()?;
            load_graph(&store, &read_triples(&triples)?)?;
            let gv = match view.as_str() {
                "datum-deps" => GraphView::DatumDeps,
                "r0le-grants" => GraphView::R0leGrants,
                "service-budgets" => GraphView::ServiceBudgets,
                _ => unreachable!(),
            };
            print!("{}", graph_to_kerml(&store, gv)?);
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Build the example** (delegate to pi on sm3lly, or wait for CI Task 10):

Run: `cargo build -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph`
Expected: compiles.

- [ ] **Step 4: End-to-end smoke** (pi / CI):

```bash
cargo run -p b00t-cli -- graph emit-triples --out /tmp/g.jsonl
cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph -- load /tmp/g.jsonl
cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph -- validate /tmp/g.jsonl
cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph -- kerml /tmp/g.jsonl --view datum-deps
```
Expected: quad count > 0; validate prints shapes (all pass, or real datum bugs to file as `b00t task`); kerml prints a `package b00t_datum_deps { ... }`.

- [ ] **Step 5: Commit**

```bash
git add b00t-c0re-lib/examples/b00t-graph.rs b00t-c0re-lib/Cargo.toml
git commit -m "feat(sp5-07): b00t-graph example bin — load/query/validate/kerml over a triples JSONL"
```

---

### Task 10: CI job + runbook

**Files:**
- Create: `.github/workflows/graph-oxigraph.yml`
- Create: `docs/runbooks/graph-substrate.md`

- [ ] **Step 1: Look at an existing workflow for the runner + toolchain setup pattern** —

Run: `ls .github/workflows/ && sed -n '1,40p' .github/workflows/<the rust test workflow>.yml`
Match its `runs-on`, Rust toolchain action, and cache setup.

- [ ] **Step 2: Write the workflow** `[PI-CANDIDATE]` — create `.github/workflows/graph-oxigraph.yml`:

```yaml
name: graph-oxigraph

on:
  pull_request:
    paths:
      - "b00t-c0re-lib/src/graph_*.rs"
      - "b00t-c0re-lib/src/query_bus.rs"
      - "b00t-c0re-lib/examples/b00t-graph.rs"
      - "b00t-cli/src/identity_triples.rs"
      - "b00t-cli/src/datum_triples.rs"
      - "b00t-cli/src/commands/graph.rs"
      - ".github/workflows/graph-oxigraph.yml"
  push:
    branches: [main, next]

jobs:
  graph-oxigraph:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          key: graph-oxigraph
      - name: cargo-nextest
        uses: taiki-e/install-action@nextest

      # 1. emit the graph from the repo's own _b00t_/ (default b00t-cli build)
      - name: emit-triples
        run: cargo run -p b00t-cli --locked -- graph emit-triples --out "$RUNNER_TEMP/graph.jsonl"

      - name: triple sanity
        run: |
          test -s "$RUNNER_TEMP/graph.jsonl"
          python3 -c "import json,sys; [json.loads(l) for l in open('$RUNNER_TEMP/graph.jsonl')]"

      # 2. b00t-c0re-lib graph unit tests (store-oxigraph only)
      - name: nextest (store-oxigraph)
        run: cargo nextest run -p b00t-c0re-lib --no-default-features --features store-oxigraph --locked

      # 3. SHACL-lite gate on the real graph
      - name: validate --strict
        run: cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --locked --example b00t-graph -- validate "$RUNNER_TEMP/graph.jsonl" --strict

      # 4. KerML view must parse
      - name: kerml view round-trips
        run: |
          cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --locked --example b00t-graph -- kerml "$RUNNER_TEMP/graph.jsonl" --view datum-deps > "$RUNNER_TEMP/view.kerml"
          test -s "$RUNNER_TEMP/view.kerml"
          grep -q "^package b00t_datum_deps {" "$RUNNER_TEMP/view.kerml"
```

> If step 3 (`validate --strict`) fails on real current datum data, that is a **real datum defect** — file it as `b00t task add "graph shape <name> violated by <focus>"` and either fix the datum in this PR or downgrade that one shape to `Severity::Warning` with a comment linking the task. Do not silently loosen a shape.

- [ ] **Step 3: Write the runbook** — create `docs/runbooks/graph-substrate.md`:

```markdown
# Runbook — SysML-v2 coherence substrate (SP5)

## The pieces

| Piece | Where | Feature |
|---|---|---|
| TRIPLES | `b00t-cli` `datum_triples.rs` + `identity_triples.rs` → `b00t graph emit-triples` (JSONL) | default |
| STORE | `b00t-c0re-lib` `irontology_bridge::OxigraphStore` (`~/.b00t/oxigraph/<ns>/`) | `store-oxigraph` |
| LOAD | `b00t-c0re-lib` `graph_load::load_graph(&store, &triples)` | `store-oxigraph` |
| SPARQL SOURCE | `b00t-c0re-lib` `query_bus::OxigraphSparqlSource` (property-path adjacency) | `store-oxigraph` |
| SHACL | `b00t-c0re-lib` `graph_shapes::validate_graph` (6 SPARQL shapes) | `store-oxigraph` |
| KERML VIEW | `b00t-c0re-lib` `graph_kerml::graph_to_kerml` → `ufo_types::sysml_model::emit_kerml` | `store-oxigraph` + `ufo-types/sysml` |
| DRIVER | `b00t-c0re-lib/examples/b00t-graph.rs` | `store-oxigraph` |

## Run it locally

```
cargo run -p b00t-cli -- graph emit-triples --out /tmp/g.jsonl
cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph -- validate /tmp/g.jsonl --strict
cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph -- kerml /tmp/g.jsonl --view r0le-grants
```

## Adding a SHACL shape

Append a `Shape { name, kind, severity, sparql, message }` to `SHAPES` in
`b00t-c0re-lib/src/graph_shapes.rs`. `AskFalse` fails if the `ASK` returns
true; `SelectAny` emits one violation per row (`?focus` = the offending node).
Add a fixture case to the `graph_shapes` test module.

## store-helixdb vs store-oxigraph

They are mutually exclusive (`compile_error!` in `irontology_bridge.rs`).
`b00t-cli` and the default `b00t-c0re-lib` build use `store-helixdb` (grok /
`ask`). The SP5 substrate only compiles under `store-oxigraph`, so it lives
in an example bin + a dedicated CI job, never in `b00t` proper. A first-class
`b00t graph query/validate` subcommand waits on the backends becoming
additive rather than exclusive (separate task).

## SP6 hand-off

`graph_to_kerml` output (KerML v2 text) is the artifact kr0ki consumes to
*render* the view (via holon-viz internally). That rendering is SP6. b00t
never calls kr0ki here and kr0ki never reads datums (kr0ki PRD §1.1).
```

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/graph-oxigraph.yml docs/runbooks/graph-substrate.md
git commit -m "ci(sp5-08): graph-oxigraph workflow + graph-substrate runbook"
```

- [ ] **Step 5: Push the branch and open/refresh the PR**

```bash
git push --no-verify origin spec/sp5-sysml-coherence
```

PR #1298 already exists (draft). Flip it to ready and let `graph-oxigraph` + the normal suite run:
```bash
gh pr ready 1298 --repo elasticdotventures/_b00t_
gh pr checks 1298 --repo elasticdotventures/_b00t_ --watch
```

- [ ] **Step 6: Merge once green**

```bash
gh pr merge 1298 --repo elasticdotventures/_b00t_ --squash --admin --delete-branch
```

---

## Integration Checkpoints

Run these after the tasks they name are all done (they live as tests / CI steps, not separate work):

1. **CP-1 triples → store** (Tasks 2, 6, 8): `b00t graph emit-triples` on the repo `_b00t_/` yields > 0 lines; `b00t-graph load` reports a matching quad count. *(CI Task 10 steps 1-2.)*
2. **CP-2 grant graph queryable** (Tasks 2, 6): `b00t-graph query /tmp/g.jsonl 'PREFIX b00t: <...#> SELECT ?t WHERE { <...#r0le/_base/worker> b00t:allowsTool ?t }'` returns the same tools `b00t r0le show worker` lists. *(Manual / add to the runbook.)*
3. **CP-3 shapes catch a plant** (Task 4): the `graph_shapes` unit tests inject a cycle + a budget-less service + a read-only r0le and assert exactly those violations.
4. **CP-4 KerML round-trips** (Tasks 1, 5): `graph_kerml` unit test + CI Task 10 step 4 — `graph_to_kerml` output parses through `validate_sysml_v2`.
5. **CP-5 default build unchanged** (Tasks 3, 5): `cargo test -p b00t-cli` and `cargo check -p b00t-c0re-lib` (default features) pass — the `store-oxigraph` code is `#[cfg]`-excluded and `dispatch_sysml.rs` still round-trips.

## Self-Review

- **Spec coverage:** SP5-01→Task 2; SP5-02→Task 6; SP5-02b→Task 7; SP5-03→Task 3; SP5-04→Task 4; SP5-05→Tasks 1+5; SP5-06→Task 8; SP5-07→Task 9; SP5-08→Task 10. The spec's "contribute the emitter to ufo-types" → Task 1. The spec's `RemoteMcpProxy`/kr0ki-render items are explicitly SP6 (spec "Deferred" section) — no task, correct.
- **Type consistency:** `load_graph(&OxigraphStore, &[(String,String,String)]) -> Result<usize>` used identically in Tasks 3, 4, 5, 9. `ShapeViolation`/`Severity` defined Task 4, consumed Task 9. `GraphView` defined Task 5, consumed Task 9. `emit_kerml(&str, &[(ElementId,ElementKind)], &[Relation]) -> String` defined Task 1, consumed Tasks 5 (both `graph_kerml` and `dispatch_sysml`). `compile_identity_triples(&str, Option<&str>) -> Result<Vec<(String,String,String)>>` defined Task 6, consumed Task 8.
- **Placeholder scan:** every code step carries the full body. The two "adjust field names to reality" notes (Task 6 Step 1, Task 5 Step 4 dependency syntax) are bounded verification steps with a stated fallback, not open-ended TODOs.
- **Open risk carried from the spec:** `validate --strict` (CI Task 10 step 3) may fail on real current `_b00t_/` data (a real `dependsOn` cycle, a real budget-less `McpServer` datum, r0les built before Task 7's rw default). Task 10 Step 2's note says: fix the datum or file a `b00t task` + downgrade that one shape to `Warning` with a linked comment — never silently loosen.

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-09-11-sp5-sysml-coherence-substrate.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — dispatch a fresh subagent per task, review between tasks. For this plan: Task 1 (ufo-types PR) and Tasks 2-4 are independent and can run in parallel; Task 5 waits on Task 1; Tasks 6-9 chain; Task 10 last. The `[PI-CANDIDATE]` steps (implementation transcription) go to `b00t-cli agent invoke pi` on sm3lly's qwen3.8-27b; frontier does every review + the field-name reconciliation notes.

**2. Inline Execution** — execute tasks in this session using executing-plans, batch with checkpoints.

**Which approach?**
