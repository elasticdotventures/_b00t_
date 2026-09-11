//! SP5-04 — SHACL-lite: a fixed set of SPARQL constraint queries over the
//! loaded b00t graph, run as a CI gate. No external SHACL engine — every
//! shape is a SPARQL `ASK` (fails if true) or `SELECT` (one violation per row).

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

enum ShapeKind {
    /// `ASK` — a violation if the query returns `true`.
    AskFalse,
    /// `SELECT ?focus` — one violation per returned row.
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
        sparql: "SELECT ?focus WHERE { ?t b00t:hasR0le ?focus . FILTER NOT EXISTS { ?focus b00t:grantsShard ?sh . ?sh b00t:shardMode \"rw\" . FILTER(CONTAINS(STR(?sh), \"shard/datum/\")) } }",
        message: "r0le has no rw grant on a datum shard",
    },
    Shape {
        name: "r0le-rw-soulscope",
        kind: ShapeKind::SelectAny,
        severity: Severity::Violation,
        sparql: "SELECT ?focus WHERE { ?t b00t:hasR0le ?focus . FILTER NOT EXISTS { ?focus b00t:grantsShard ?sh . ?sh b00t:shardMode \"rw\" . FILTER(REGEX(STR(?sh), \"shard/(project|system|agent|skill|tool)/\")) } }",
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
/// (Violation + Warning); callers decide the exit policy.
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
    out.dedup_by(|a, b| a.shape == b.shape && a.focus_node == b.focus_node);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_load::load_graph;
    use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};

    fn store_with(tag: &str, triples: &[(&str, &str, &str)]) -> OxigraphStore {
        let dir = std::env::temp_dir().join(format!(
            "sp5-shape-{}-{}-{}",
            std::process::id(),
            tag,
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
        s
    }

    #[test]
    fn clean_graph_has_no_hard_violations() {
        let s = store_with(
            "clean",
            &[
                ("b00t:tenant/pe", "b00t:hasR0le", "b00t:r0le/pe/worker"),
                ("b00t:r0le/pe/worker", "b00t:grantsShard", "b00t:shard/datum/all"),
                ("b00t:shard/datum/all", "b00t:shardMode", "rw"),
                ("b00t:r0le/pe/worker", "b00t:grantsShard", "b00t:shard/project/all"),
                ("b00t:shard/project/all", "b00t:shardMode", "rw"),
            ],
        );
        let v = validate_graph(&s).unwrap();
        assert!(
            v.iter().all(|x| x.severity == Severity::Warning),
            "unexpected hard violations: {v:?}"
        );
    }

    #[test]
    fn detects_a_dependson_cycle() {
        let s = store_with(
            "cycle",
            &[
                ("b00t:datum/a", "b00t:dependsOn", "b00t:datum/b"),
                ("b00t:datum/b", "b00t:dependsOn", "b00t:datum/a"),
            ],
        );
        let v = validate_graph(&s).unwrap();
        assert!(v
            .iter()
            .any(|x| x.shape == "dependsOn-acyclic" && x.severity == Severity::Violation));
    }

    #[test]
    fn detects_a_budgetless_service_and_a_role_missing_rw() {
        let s = store_with(
            "budget",
            &[
                ("b00t:svc/gh", "b00t:image", "ghcr.io/x/gh@sha256:deadbeef"),
                ("b00t:tenant/pe", "b00t:hasR0le", "b00t:r0le/pe/thin"),
                ("b00t:r0le/pe/thin", "b00t:grantsShard", "b00t:shard/tool/all"),
                ("b00t:shard/tool/all", "b00t:shardMode", "r"),
            ],
        );
        let v = validate_graph(&s).unwrap();
        assert!(v
            .iter()
            .any(|x| x.shape == "every-svc-has-a-budget" && x.severity == Severity::Violation));
        assert!(v
            .iter()
            .any(|x| x.shape == "r0le-rw-datum" && x.severity == Severity::Violation));
    }
}
