//! SP5-05 — project a SPARQL view of the b00t graph into native KerML text.
//!
//! Native procedural generation via `ufo_types::sysml_model::emit_kerml` — the
//! same emitter `b00t-cli/src/dispatch_sysml.rs` uses. KerML is the first-class
//! view type (not Mermaid, which breaks on complex diagrams). The render side
//! (kr0ki, via holon-viz) consumes this text — that is SP6, out of scope here.

use crate::irontology_bridge::OxigraphStore;
use anyhow::Result;
use oxigraph::sparql::QueryResults;
use ufo_types::sysml_model::{emit_kerml, ElementId, ElementKind, Relation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphView {
    /// datum → datum `b00t:dependsOn` edges.
    DatumDeps,
    /// r0le → shard-grant / tool-allowlist edges.
    R0leGrants,
    /// MCP service → budget / image facts.
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
                "SELECT ?s ?o WHERE { { ?s b00t:grantsShard ?o } UNION { ?s b00t:allowsTool ?o } }"
            }
            GraphView::ServiceBudgets => {
                "SELECT ?s ?o WHERE { { ?s b00t:budgetCeiling ?o } UNION { ?s b00t:image ?o } }"
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

fn local_name(s: &str) -> String {
    s.rsplit(['#', '/']).next().unwrap_or(s).to_string()
}

/// Sanitize a graph local-name into an unquoted KerML identifier: KerML
/// unquoted names can't carry `.` / `-` / `:` / space — replace with `_`.
fn ident(name: &str) -> String {
    let n = name.replace(['.', '-', ' ', ':', '@', '*'], "_");
    if n.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(true) {
        format!("n_{n}")
    } else {
        n
    }
}

/// Run `view`'s SELECT, build a typed element/relation graph, and render it
/// as KerML via `ufo_types::sysml_model::emit_kerml`. Round-trip-validated in
/// debug builds.
pub fn graph_to_kerml(store: &OxigraphStore, view: GraphView) -> Result<String> {
    let q = format!("{PREFIXES}\n{}", view.sparql());
    let results = store.store().query(&q)?;

    let mut node_names: std::collections::BTreeSet<String> = Default::default();
    let mut relations: Vec<Relation> = Vec::new();

    if let QueryResults::Solutions(sols) = results {
        for sol in sols {
            let sol = sol?;
            let s = match sol.get("s") {
                Some(oxigraph::model::Term::NamedNode(n)) => local_name(n.as_str()),
                Some(t) => local_name(&t.to_string()),
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
        ufo_types::sysml::validate_sysml_v2(&text)
            .disposition
            .is_satisfied(),
        "graph_to_kerml produced invalid KerML:\n{text}"
    );
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_load::load_graph;
    use crate::irontology_bridge::{KnowledgeStoreBackend, OxigraphStore, StoreConfig};

    fn store_with(triples: &[(&str, &str, &str)]) -> OxigraphStore {
        let dir = std::env::temp_dir().join(format!(
            "sp5-kerml-{}-{}",
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
        s
    }

    #[test]
    fn datum_deps_view_emits_parts_and_round_trips() {
        let s = store_with(&[
            ("b00t:datum/rust.cli", "b00t:dependsOn", "b00t:datum/cargo"),
            ("b00t:datum/cargo", "b00t:dependsOn", "b00t:datum/rustup"),
        ]);
        let kerml = graph_to_kerml(&s, GraphView::DatumDeps).unwrap();
        assert!(kerml.contains("part def cargo;"), "{kerml}");
        assert!(kerml.contains("part def rustup;"), "{kerml}");
        assert!(kerml.contains("part def rust_cli;"), "{kerml}");
        assert_eq!(kerml, graph_to_kerml(&s, GraphView::DatumDeps).unwrap());
        assert!(ufo_types::sysml::validate_sysml_v2(&kerml)
            .disposition
            .is_satisfied());
    }
}
