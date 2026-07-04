//! JSON graph data endpoint for client-side WASM rendering.
//! Returns { nodes: [...], edges: [...], depth: {...} } directly,
//! letting the WASM layout engine do SVG rendering client-side.

use b00t_l3dg3rr_viz::{InvariantEdge, InvariantGraph, InvariantNode, VisualizationRole};
use b00t_l3dg3rr_viz::isometric::{parse_mermaid, ContextDepth, filter_orphans};
use axum::extract::Query;
use std::collections::HashMap;

pub async fn entangle_json_handler(
    Query(params): Query<HashMap<String, String>>,
) -> impl axum::response::IntoResponse {
    let hide_orphans = params.get("hide_orphans").map(|v| v == "true").unwrap_or(false);
    let output = std::process::Command::new("b00t-cli")
        .args(["viz", "entangle", "--format", "mermaid"])
        .output();
    let raw = output.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .replace("```mermaid\n", "").replace("\n```", "")
        .replace("graph LR", "flowchart LR").replace("graph TD", "flowchart TD");

    let mut graph = parse_mermaid(&raw).unwrap_or_else(|_| InvariantGraph::new("empty"));
    if hide_orphans {
        graph = filter_orphans(&graph);
    }

    let depth = ContextDepth::classify(&graph);
    let nodes: Vec<serde_json::Value> = graph.nodes.iter().map(|n| {
        let d = depth.get(&n.id).copied().unwrap_or(ContextDepth::Surface);
        let d_str = match d { ContextDepth::Surface => "surface", ContextDepth::Extended => "extended", ContextDepth::Historical => "historical" };
        serde_json::json!({
            "id": n.id,
            "label": n.label,
            "role": n.role.as_str(),
            "invariant": n.invariant,
            "depth": d_str,
        })
    }).collect();

    let edges: Vec<serde_json::Value> = graph.edges.iter().map(|e| {
        serde_json::json!({
            "from": e.from,
            "to": e.to,
            "label": e.label,
        })
    }).collect();

    let (surface, extended, historical) = (
        depth.values().filter(|d| matches!(d, ContextDepth::Surface)).count(),
        depth.values().filter(|d| matches!(d, ContextDepth::Extended)).count(),
        depth.values().filter(|d| matches!(d, ContextDepth::Historical)).count(),
    );

    axum::Json(serde_json::json!({
        "nodes": nodes,
        "edges": edges,
        "depth": { "surface": surface, "extended": extended, "historical": historical },
    }))
}
