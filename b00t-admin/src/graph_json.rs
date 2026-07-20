//! JSON graph data endpoint for client-side WASM rendering.
//! Returns { nodes: [...], edges: [...], depth: {...} } directly,
//! letting the WASM layout engine do SVG rendering client-side.

use axum::extract::Query;
use b00t_l3dg3rr_viz::InvariantGraph;
use b00t_l3dg3rr_viz::artifact::curate_orphans;
use b00t_l3dg3rr_viz::isometric::{filter_orphans, parse_mermaid};
use std::collections::{HashMap, HashSet};

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
            serde_json::json!({
                "id": n.id,
                "label": n.label,
                "role": n.role.as_str(),
                "invariant": n.invariant,
                "depth": d_str,
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
    });

    if curate {
        let curation = curate_orphans(&nodes, &edges);
        if let Some(obj) = response.as_object_mut() {
            obj.insert("curation".to_string(), serde_json::json!(curation));
        }
    }

    axum::Json(response)
}
