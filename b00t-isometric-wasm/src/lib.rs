//! WASM isometric renderer — client-side SVG generation from graph JSON.
//!
//! ```js
//! import init, { render_isometric } from './b00t_isometric_wasm.js';
//! await init();
//! const resp = await fetch('/api/admin/viz/entangle/json?hide_orphans=true');
//! const data = await resp.json();
//! const svg = render_isometric(JSON.stringify(data));
//! ```

use wasm_bindgen::prelude::*;

use b00t_l3dg3rr_viz::isometric::graph_to_isometric_svg;
use b00t_l3dg3rr_viz::{
    InvariantEdge, InvariantGraph, InvariantNode, VisualizationRole,
};

#[wasm_bindgen]
pub fn render_isometric(json_data: &str) -> String {
    match render_inner(json_data) {
        Ok(svg) => svg,
        Err(e) => format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 400 60'><rect width='100%' height='100%' fill='#1e293b'/><text x='200' y='35' text-anchor='middle' fill='#ef4444' font-family='monospace' font-size='12'>isometric: {}</text></svg>",
            e.replace('<', "&lt;").replace('>', "&gt;")
        ),
    }
}

fn render_inner(json_data: &str) -> Result<String, String> {
    let data: serde_json::Value = serde_json::from_str(json_data).map_err(|e| e.to_string())?;
    let nodes = data.get("nodes").and_then(|v| v.as_array()).ok_or("missing nodes")?;
    let edges = data.get("edges").and_then(|v| v.as_array()).ok_or("missing edges")?;

    let mut graph = InvariantGraph::new("wasm_graph");
    for n in nodes {
        let role_str = n.get("role").and_then(|v| v.as_str()).unwrap_or("step");
        let role = role_from_str(role_str);
        graph = graph.with_node(InvariantNode::new(
            n.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            n.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            role,
        ));
    }
    for e in edges {
        graph = graph.with_edge(InvariantEdge::new(
            e.get("from").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            e.get("to").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        ).with_label(
            e.get("label").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        ));
    }

    graph_to_isometric_svg(&graph)
}

fn role_from_str(s: &str) -> VisualizationRole {
    match s {
        "data" => VisualizationRole::Data,
        "intelligence" => VisualizationRole::Intelligence,
        "rule" => VisualizationRole::Rule,
        "security" => VisualizationRole::Security,
        "human" => VisualizationRole::Human,
        "logic" => VisualizationRole::Logic,
        "storage" => VisualizationRole::Storage,
        "report" => VisualizationRole::Report,
        "task" => VisualizationRole::Task,
        "event" => VisualizationRole::Event,
        "ingest" => VisualizationRole::Ingest,
        "validate" => VisualizationRole::Validate,
        "classify" => VisualizationRole::Classify,
        "review" => VisualizationRole::Review,
        "reconcile" => VisualizationRole::Reconcile,
        "commit" => VisualizationRole::Commit,
        "decision" => VisualizationRole::Decision,
        _ => VisualizationRole::Step,
    }
}
