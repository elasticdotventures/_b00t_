//! SP5-01 — assert a plain SPO triple set into an `OxigraphStore`.
//!
//! The graph substrate operates on `Vec<(String, String, String)>` triples —
//! no `BootDatum` dependency. The compilers that produce those triples live in
//! `b00t-cli` (`datum_triples`, `identity_triples`); the composed set reaches
//! here via `b00t graph emit-triples` → JSONL → the `b00t-graph` example bin.

use crate::irontology_bridge::OxigraphStore;
use anyhow::{Context, Result};
use oxigraph::model::{GraphName, Literal, NamedNode, Quad, Term};

pub const B00T_NS: &str = "http://b00t.promptexecution.com/ontology#";
pub const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";

/// Expand a CURIE-ish term to a full IRI. `b00t:foo` and `rdfs:foo` map to
/// the b00t / RDFS namespaces; anything already `http(s)://…` is returned
/// unchanged; a bare token is treated as a b00t-namespace local name.
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

/// Is this object-position term an IRI (a node reference) or a plain literal?
/// It is an IRI iff it carries a known prefix or an http scheme.
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
/// number of triples processed (oxigraph dedups on insert, so re-running the
/// same set is a no-op).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};

    fn temp_store() -> OxigraphStore {
        let dir = std::env::temp_dir().join(format!(
            "sp5-load-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
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
        load_graph(&store, &triples).unwrap();
        assert_eq!(store.store().len().unwrap(), 2);
    }
}
