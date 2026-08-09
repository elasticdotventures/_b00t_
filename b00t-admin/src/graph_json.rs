//! JSON graph data endpoint for client-side WASM rendering.
//! Returns { nodes: [...], edges: [...], depth: {...} } directly,
//! letting the WASM layout engine do SVG rendering client-side.

use axum::extract::Query;
use b00t_cli::DatumType;
use b00t_l3dg3rr_viz::InvariantGraph;
use b00t_l3dg3rr_viz::artifact::curate_orphans;
use b00t_l3dg3rr_viz::isometric::{filter_orphans, parse_mermaid};
use std::collections::{HashMap, HashSet};

/// Resolve a node's [`b00t_cli::SemanticClass`] shape/colour from its label.
///
/// `datum_graph_to_mermaid` (b00t-cli/src/viz/mod.rs) embeds each node's
/// `DatumType::type_prefix()` token as the label's second line
/// (`"key\ntype_token"`, using a literal `\n` mermaid line-break, not a
/// newline byte). This round-trips that token back through
/// `DatumType::from_type_token()` to recover the real SemanticClass —
/// avoiding re-deriving shape/colour tables here (single source of truth
/// stays `b00t-cli/src/datum_types.rs`).
///
/// Returns `None` for labels with no recognizable trailing type token (e.g.
/// graphs not sourced from `viz entangle`, or nodes with `datum_type: None`
/// which embed the literal `"?"` placeholder).
fn semantic_style(label: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let token = label.rsplit("\\n").next()?.trim();
    let dtype = DatumType::from_type_token(token)?;
    let class = dtype.semantic_class();
    // SemanticClass has no public name accessor; css_class() ("sc-agent")
    // is the existing stable identifier — strip the "sc-" prefix rather
    // than adding a new method that duplicates the same information.
    let class_name = class.css_class().trim_start_matches("sc-");
    Some((class.shape(), class.color(), class_name))
}

#[cfg(test)]
mod semantic_style_tests {
    use super::*;

    #[test]
    fn resolves_known_type_token() {
        // Agent → SemanticClass::Agent → shape "circle"
        let style = semantic_style("my-agent\\nagent");
        assert_eq!(style, Some(("circle", "#059669", "agent")));
    }

    #[test]
    fn resolves_infra_type_token() {
        // k8s → SemanticClass::Infra → shape "hexagon"
        let style = semantic_style("my-cluster\\nk8s");
        assert_eq!(style, Some(("hexagon", "#326ce5", "infra")));
    }

    #[test]
    fn returns_none_for_missing_type_placeholder() {
        assert_eq!(semantic_style("orphan-node\\n?"), None);
    }

    #[test]
    fn returns_none_for_label_without_type_suffix() {
        assert_eq!(semantic_style("plain label, no type token"), None);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextDepth {
    Surface,
    Extended,
    Historical,
}

impl ContextDepth {
    fn classify(graph: &InvariantGraph) -> HashMap<String, ContextDepth> {
        let connected: HashSet<&str> = graph
            .edges
            .iter()
            .flat_map(|e| [e.from.as_str(), e.to.as_str()])
            .collect();
        let has_invariant: HashSet<&str> = graph
            .nodes
            .iter()
            .filter(|n| n.invariant.as_ref().map_or(false, |i| !i.is_empty()))
            .map(|n| n.id.as_str())
            .collect();
        graph
            .nodes
            .iter()
            .map(|n| {
                let depth = if !connected.contains(n.id.as_str()) {
                    ContextDepth::Surface
                } else if has_invariant.contains(n.id.as_str()) {
                    ContextDepth::Historical
                } else {
                    ContextDepth::Extended
                };
                (n.id.clone(), depth)
            })
            .collect()
    }
}

pub async fn entangle_json_handler(
    Query(params): Query<HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let hide_orphans = params
        .get("hide_orphans")
        .map(|v| v == "true")
        .unwrap_or(false);
    let curate = params.get("curate").map(|v| v == "true").unwrap_or(false);
    let output = std::process::Command::new("b00t-cli")
        .args(["viz", "entangle", "--format", "mermaid"])
        .output();
    let raw = output
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .replace("```mermaid\n", "")
        .replace("\n```", "")
        .replace("graph LR", "flowchart LR")
        .replace("graph TD", "flowchart TD");

    let mut graph = parse_mermaid(&raw).unwrap_or_else(|_| InvariantGraph::new("empty"));
    if hide_orphans {
        graph = filter_orphans(&graph);
    }

    let depth = ContextDepth::classify(&graph);
    // 🤓 #714: legend entries keyed by semantic_class name so duplicates
    // across nodes collapse to one legend row each (BTreeMap for stable,
    // alphabetical ordering in the response).
    let mut legend: std::collections::BTreeMap<&'static str, (&'static str, &'static str)> =
        std::collections::BTreeMap::new();
    let nodes: Vec<serde_json::Value> = graph
        .nodes
        .iter()
        .map(|n| {
            let d = depth.get(&n.id).copied().unwrap_or(ContextDepth::Surface);
            let d_str = match d {
                ContextDepth::Surface => "surface",
                ContextDepth::Extended => "extended",
                ContextDepth::Historical => "historical",
            };
            let style = semantic_style(&n.label);
            if let Some((shape, color, class_name)) = style {
                legend.entry(class_name).or_insert((shape, color));
            }
            serde_json::json!({
                "id": n.id,
                "label": n.label,
                "role": n.role.as_str(),
                "invariant": n.invariant,
                "depth": d_str,
                "shape": style.map(|s| s.0),
                "color": style.map(|s| s.1),
                "semantic_class": style.map(|s| s.2),
            })
        })
        .collect();

    let legend: Vec<serde_json::Value> = legend
        .into_iter()
        .map(|(class_name, (shape, color))| {
            serde_json::json!({
                "semantic_class": class_name,
                "shape": shape,
                "color": color,
            })
        })
        .collect();

    let edges: Vec<serde_json::Value> = graph
        .edges
        .iter()
        .map(|e| {
            serde_json::json!({
                "from": e.from,
                "to": e.to,
                "label": e.label,
            })
        })
        .collect();

    let (surface, extended, historical) = (
        depth
            .values()
            .filter(|d| matches!(d, ContextDepth::Surface))
            .count(),
        depth
            .values()
            .filter(|d| matches!(d, ContextDepth::Extended))
            .count(),
        depth
            .values()
            .filter(|d| matches!(d, ContextDepth::Historical))
            .count(),
    );

    let mut response = serde_json::json!({
        "nodes": nodes,
        "edges": edges,
        "depth": { "surface": surface, "extended": extended, "historical": historical },
        // 🤓 #714: distinct SemanticClass values actually present in this
        // graph, so the dashboard legend updates dynamically per-render
        // instead of hardcoding all 9 classes regardless of relevance.
        "legend": legend,
    });

    if curate {
        let curation = curate_orphans(&nodes, &edges);
        if let Some(obj) = response.as_object_mut() {
            obj.insert("curation".to_string(), serde_json::json!(curation));
        }
    }

    axum::Json(response)
}
