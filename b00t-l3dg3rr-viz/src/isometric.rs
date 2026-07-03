//! Isometric (2:1 dimetric) graph layout & SVG rendering using the `kasuari`
//! Cassowary constraint solver.
//!
//! # Integration chain
//! ```text
//! mermaid text → InvariantGraph → kasuari constraint solver → 3D positions
//!              → iso_project → SVG (2:1 dimetric, card depth faces)
//! ```

use crate::{InvariantEdge, InvariantGraph, InvariantNode, VisualizationRole};
use kasuari::WeightedRelation::*;
use kasuari::{Solver, Strength, Variable};
use std::collections::{HashMap, VecDeque};

const ISO_X: f64 = 0.8660254; // cos(30°)
const ISO_Y: f64 = 0.5; // sin(30°)
const NODE_SPACING: f64 = 120.0;
const LAYER_SPACING: f64 = 100.0;
const MIN_X_DIST: f64 = 140.0;
const MAX_NODES: usize = 40;

/// 2:1 dimetric isometric projection — maps 3D world coords to 2D screen pixels.
///
/// # Why 2:1 dimetric (cos 30° ≈ 0.866, sin 30° = 0.5)?
///
/// In a true isometric projection, all three axes are foreshortened equally
/// (angles = 120° apart, scale ≈ 0.816). The 2:1 dimetric approximation uses
/// a slope of 2:1 for the X/Z ground-plane axes, which:
///   - Produces pixel-perfect lines on raster displays (2px run : 1px rise)
///   - Preserves horizontal/vertical alignment for text labels
///   - Is the standard used by classic isometric games (SimCity, Diablo)
///
/// # Transform derivation
///
/// World → screen mapping rotates the ground plane (X-Z) by 30° around Y:
///   screen_x = origin_x + (world_x - world_z) * scale * cos(30°)
///   screen_y = origin_y + (world_x + world_z) * scale * sin(30°) - world_y * scale
///
/// Notice: +X moves right-and-down, +Z moves left-and-down, +Y moves straight UP.
/// This is why Z is the "flow" axis (forward/back) in the constraint model —
/// positive Z pushes nodes deeper into the scene.
fn iso_project(x: f64, z: f64, y: f64, scale: f64, ox: f64, oy: f64) -> (f64, f64) {
    (
        ox + (x - z) * scale * ISO_X,
        oy + (x + z) * scale * ISO_Y - y * scale,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MermaidParseError {
    EmptyInput,
    InvalidNodeSyntax(String),
    InvalidEdgeSyntax(String),
}

impl std::fmt::Display for MermaidParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => f.write_str("mermaid input is empty"),
            Self::InvalidNodeSyntax(line) => write!(f, "invalid node syntax: {line}"),
            Self::InvalidEdgeSyntax(line) => write!(f, "invalid edge syntax: {line}"),
        }
    }
}

impl std::error::Error for MermaidParseError {}

fn role_from_label(label: &str) -> VisualizationRole {
    let lower = label.to_lowercase().replace('_', " ");
    // Process-specific roles first (more precise matches)
    if lower.contains("ingest") || lower.contains("load") || lower.contains("parse")
        || lower.contains("extract") || lower.contains("docling")
    {
        VisualizationRole::Ingest
    } else if lower.contains("valid") || lower.contains("verify") || lower.contains("check") {
        VisualizationRole::Validate
    } else if lower.contains("classif") || lower.contains("categor") || lower.contains("tag")
        || lower.contains("label")
    {
        VisualizationRole::Classify
    } else if lower.contains("review") || lower.contains("inspect") || lower.contains("audit") {
        VisualizationRole::Review
    } else if lower.contains("reconcil") || lower.contains("match") || lower.contains("balance") {
        VisualizationRole::Reconcile
    } else if lower.contains("commit") || lower.contains("save") || lower.contains("write") {
        VisualizationRole::Commit
    } else if lower.contains("decis") || lower.contains("if") || lower.contains("branch") {
        VisualizationRole::Decision
    // Generic semantic categories (fallback after process roles)
    } else if lower.contains("source") || lower.contains("statement") || lower.contains("blake3")
        || lower.contains("document") || lower.contains("file")
    {
        VisualizationRole::Data
    } else if lower.contains("llm") || lower.contains("ai") || lower.contains("gpt")
        || lower.contains("phi") || lower.contains("reasoning") || lower.contains("model")
    {
        VisualizationRole::Intelligence
    } else if lower.contains("legal") || lower.contains("rule") || lower.contains("constraint")
        || lower.contains("law") || lower.contains("solver") || lower.contains("registry")
    {
        VisualizationRole::Rule
    } else if lower.contains("security") || lower.contains("auth") || lower.contains("credential")
        || lower.contains("guard") || lower.contains("safety")
    {
        VisualizationRole::Security
    } else if lower.contains("human") || lower.contains("operator") || lower.contains("reviewer")
        || lower.contains("accountant")
    {
        VisualizationRole::Human
    } else if lower.contains("storage") || lower.contains("database") || lower.contains("store") {
        VisualizationRole::Storage
    } else if lower.contains("report") || lower.contains("export") || lower.contains("summary")
        || lower.contains("chart")
    {
        VisualizationRole::Report
    } else if lower.contains("event") || lower.contains("notify") || lower.contains("alert")
        || lower.contains("hook")
    {
        VisualizationRole::Event
    // Remove "input" from Data — it was too broad and caught "validate input"
    } else {
        VisualizationRole::Step
    }
}

pub fn parse_mermaid(text: &str) -> Result<InvariantGraph, MermaidParseError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(MermaidParseError::EmptyInput);
    }

    let mut nodes: HashMap<String, InvariantNode> = HashMap::new();
    let mut edges: Vec<InvariantEdge> = Vec::new();
    let mut seen_ids = std::collections::BTreeSet::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("%%")
            || line.starts_with("classDef")
            || line.starts_with("graph ")
            || line.starts_with("flowchart ")
        {
            continue;
        }

        if line.contains("-->") || line.contains("--") || line.contains("==") {
            let parts: Vec<&str> = line.splitn(2, "-->").collect();
            if parts.len() == 2 {
                let from_raw = parts[0].trim();
                let to_raw = parts[1].trim();

                let from_id: String = from_raw
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                    .collect();

                let (to_id, label) = {
                    let _pipe_pos = to_raw.find('|');
                    let segs: Vec<&str> = to_raw.split('|').collect();
                    if segs.len() >= 3 {
                        let tid = segs[2]
                            .trim()
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                            .collect::<String>();
                        (tid, Some(segs[1].trim().to_string()))
                    } else {
                        let tid = to_raw
                            .chars()
                            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                            .collect::<String>();
                        (tid, None)
                    }
                };

                if from_id.is_empty() || to_id.is_empty() {
                    return Err(MermaidParseError::InvalidEdgeSyntax(line.to_string()));
                }
                edges.push(InvariantEdge::new(&from_id, &to_id).with_label(label.unwrap_or_default()));
                seen_ids.insert(from_id);
                seen_ids.insert(to_id);
            }
            continue;
        }

        let trimmed = line.trim_end_matches(';');
        if let Some(node) = parse_node_line(trimmed) {
            seen_ids.insert(node.id.clone());
            nodes.entry(node.id.clone()).or_insert(node);
        }
    }

    let _ids_from_edges: std::collections::BTreeSet<String> = edges
        .iter()
        .flat_map(|e| [e.from.clone(), e.to.clone()])
        .collect();

    let name = if nodes.is_empty() && edges.is_empty() {
        "empty"
    } else if let Some(first_edge) = edges.first() {
        &first_edge.from
    } else {
        nodes.values().next().map_or("graph", |n| &n.id)
    };

    let mut graph = InvariantGraph::new(name.to_string());

    for id in &seen_ids {
        if let Some(node) = nodes.get(id) {
            graph = graph.with_node(node.clone());
        } else {
            graph = graph.with_node(InvariantNode::new(
                id.clone(),
                id.clone(),
                VisualizationRole::Step,
            ));
        }
    }

    for edge in &edges {
        if seen_ids.contains(&edge.from) && seen_ids.contains(&edge.to) {
            graph = graph.with_edge(edge.clone());
        }
    }

    Ok(graph)
}

fn parse_node_line(line: &str) -> Option<InvariantNode> {
    if let Some(cap) = extract_node(line) {
        return Some(cap);
    }
    manual_node_parse(line)
}

fn extract_node(line: &str) -> Option<InvariantNode> {
    let line = line.trim();
    let rest = if let Some(r) = line.strip_prefix(":::") {
        r
    } else {
        line
    };

    let mut in_string = false;
    let mut idx = 0usize;

    for (i, c) in rest.char_indices() {
        if c == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        if c == '[' || c == '(' || c == '{' {
            idx = i;
            break;
        }
    }

    if idx == 0 {
        return None;
    }

    let id: String = rest[..idx]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();

    if id.is_empty() {
        return None;
    }

    let remainder = rest[idx..].trim();
    let label = extract_label(remainder).unwrap_or_else(|| id.clone());

    Some(InvariantNode::new(
        id.clone(),
        label.clone(),
        role_from_label(&label),
    ))
}

fn extract_label(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        let inner = &s[1..s.len() - 1];
        let label = inner.trim_matches('"').trim();
        if !label.is_empty() {
            return Some(label.to_string());
        }
    }
    if s.starts_with('(') && s.ends_with(')') {
        let inner = &s[1..s.len() - 1];
        let label = inner.trim_matches('"').trim();
        if !label.is_empty() {
            return Some(label.to_string());
        }
    }
    None
}

fn manual_node_parse(line: &str) -> Option<InvariantNode> {
    let line = line.trim();
    let line = line.trim_end_matches(';');

    let id_end = line
        .find(|c: char| c == '[' || c == '(' || c == '{')
        .unwrap_or(line.len());
    let id: String = line[..id_end]
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect();
    if id.is_empty() || id.len() > 64 {
        return None;
    }

    let remainder = line[id_end..].trim();
    let label = extract_label(remainder).unwrap_or_else(|| id.clone());

    Some(InvariantNode::new(
        id.clone(),
        label.clone(),
        role_from_label(&label),
    ))
}

pub fn graph_to_isometric_svg(graph: &InvariantGraph) -> Result<String, String> {
    if graph.nodes.is_empty() {
        return Ok(r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100" width="200" height="100"><rect width="100%" height="100%" fill="#0f172a"/><text x="100" y="55" text-anchor="middle" fill="#64748b" font-family="monospace" font-size="14">No nodes to render</text></svg>"##.to_string());
    }

    let positions = kasuari_layout(graph)?;
    let svg = render_svg(graph, &positions);
    Ok(svg)
}

/// Strip orphan nodes (nodes with zero edges) from a graph.
/// Orphan nodes have no trait connections and clutter the visualization.
/// Returns a new graph containing only connected nodes and their edges.
pub fn filter_orphans(graph: &InvariantGraph) -> InvariantGraph {
    let connected: std::collections::HashSet<&str> = graph
        .edges
        .iter()
        .flat_map(|e| [e.from.as_str(), e.to.as_str()])
        .collect();
    let mut filtered = InvariantGraph::new(graph.name.clone());
    for node in &graph.nodes {
        if connected.contains(node.id.as_str()) {
            filtered = filtered.with_node(node.clone());
        }
    }
    for edge in &graph.edges {
        filtered = filtered.with_edge(edge.clone());
    }
    filtered
}

/// Strip orphan nodes from raw mermaid text.
pub fn filter_orphans_from_mermaid(mermaid: &str) -> InvariantGraph {
    let graph = parse_mermaid(mermaid).unwrap_or_else(|_| InvariantGraph::new("empty"));
    filter_orphans(&graph)
}

pub fn mermaid_to_isometric_svg(mermaid_text: &str) -> Result<String, String> {
    let graph = parse_mermaid(mermaid_text).map_err(|e| e.to_string())?;
    if let Err(e) = graph.validate() {
        return Err(format!("graph validation failed: {e}"));
    }
    graph_to_isometric_svg(&graph)
}

/// Export the isometric scene as a glTF 2.0 data URI for 3D viewers.
///
/// # What glTF is
///
/// glTF (GL Transmission Format) is the "JPEG of 3D" — a standard runtime
/// asset format by Khronos Group. Unlike verbose formats like COLLADA, glTF
/// is designed to be compact, fast to load, and directly consumable by GPU
/// pipelines. Every major 3D engine/tool supports it.
///
/// # Data URI strategy
///
/// We embed the glTF JSON directly in a `data:` URI rather than writing a
/// separate .gltf file. This means the entire scene (nodes, meshes, materials)
/// travels in a single HTTP response field — no secondary request needed.
/// The encoding is `data:model/gltf+json;base64,<payload>`.
///
/// # Skeleton-only export
///
/// The exported glTF is a *skeleton* — node transforms with named materials
/// but without embedded vertex data (POSITION/NORMAL accessors point to a
/// placeholder buffer). This is intentional: a 3D viewer can place its own
/// box/sphere/icon meshes at each node's world position using the material
/// colors we provide. Full embedded meshes would bloat the URI by 10-100x.
///
/// # Node hierarchy
///
/// The scene root contains one child per graph node. Each child's `translation`
/// field places it at the 3D position computed by the kasuari layout solver.
/// The `extras` field carries graph metadata (id, role, hex color) consumable
/// by custom glTF viewers or import scripts.
pub fn scene_to_gltf_data_uri(
    graph: &InvariantGraph,
    positions: &HashMap<String, (f64, f64, f64)>,
) -> Result<String, String> {
    let node_list: Vec<serde_json::Value> = graph
        .nodes
        .iter()
        .filter_map(|n| {
            positions.get(&n.id).map(|&(x, y, z)| {
                let color = n.role.color();
                let mesh_idx = n.role as usize;
                serde_json::json!({
                    "name": n.label,
                    "translation": [x, z, y],
                    "mesh": mesh_idx,
                    "extras": {
                        "id": n.id,
                        "role": format!("{:?}", n.role),
                        "color": color
                    }
                })
            })
        })
        .collect();

    let meshes: Vec<serde_json::Value> = VisualizationRole::all()
        .iter()
        .map(|role| {
            let material_idx = *role as usize;
            serde_json::json!({
                "name": format!("{:?}_box", role),
                "primitives": [{
                    "attributes": {
                        "POSITION": 0,
                        "NORMAL": 1
                    },
                    "material": material_idx
                }]
            })
        })
        .collect();

    let materials: Vec<serde_json::Value> = VisualizationRole::all()
        .iter()
        .map(|&role| {
            let hex = role.color().trim_start_matches('#');
            let r = u32::from_str_radix(&hex[0..2], 16).unwrap_or(120) as f64 / 255.0;
            let g = u32::from_str_radix(&hex[2..4], 16).unwrap_or(120) as f64 / 255.0;
            let b = u32::from_str_radix(&hex[4..6], 16).unwrap_or(120) as f64 / 255.0;
            serde_json::json!({
                "name": format!("{:?}_material", role),
                "pbrMetallicRoughness": {
                    "baseColorFactor": [r, g, b, 0.9],
                    "metallicFactor": 0.1,
                    "roughnessFactor": 0.7
                }
            })
        })
        .collect();

    let gltf = serde_json::json!({
        "asset": {
            "version": "2.0",
            "generator": "b00t-l3dg3rr-viz/kasuari",
            "copyright": "b00t isometric scene export"
        },
        "scene": 0,
        "scenes": [{
            "name": "isometric_scene",
            "nodes": (0..node_list.len()).collect::<Vec<usize>>()
        }],
        "nodes": node_list,
        "meshes": meshes,
        "materials": materials,
        "accessors": [
            {
                "bufferView": 0,
                "componentType": 5126,
                "count": 24,
                "type": "VEC3",
                "max": [1.0, 1.0, 1.0],
                "min": [-1.0, -1.0, -1.0]
            },
            {
                "bufferView": 1,
                "componentType": 5126,
                "count": 24,
                "type": "VEC3"
            }
        ],
        "bufferViews": [
            {"buffer": 0, "byteOffset": 0, "byteLength": 288, "target": 34962},
            {"buffer": 0, "byteOffset": 288, "byteLength": 288, "target": 34962}
        ],
        "buffers": [{
            "uri": "data:application/octet-stream;base64,",
            "byteLength": 576
        }]
    });

    let json = serde_json::to_string(&gltf).map_err(|e| e.to_string())?;
    let encoded = base64_encode(&json);
    Ok(format!("data:model/gltf+json;base64,{encoded}"))
}

pub fn graph_to_isometric_response(
    graph: &InvariantGraph,
) -> Result<serde_json::Value, String> {
    let positions = kasuari_layout(graph)?;
    let svg = render_svg(graph, &positions);
    let gltf = scene_to_gltf_data_uri(graph, &positions).ok();
    Ok(serde_json::json!({
        "svg": svg,
        "gltf": gltf,
        "format": "isometric",
        "solver": "kasuari/0.4/cassowary",
        "nodes": graph.nodes.len(),
        "edges": graph.edges.len(),
    }))
}

/// Group large graphs by connected components. Each component becomes a
/// "container" super-node that can be drilled into. This replaces the flat
/// 40-node cap with a hierarchical view — the top level shows containers,
/// double-click (in the JS viewer) expands a container to its sub-graph.
///
/// # Algorithm (branch-and-bound clustering)
///
/// 1. Find connected components via BFS over the undirected edge graph.
///    Two nodes are connected if an edge exists between them in either direction.
/// 2. If a component has ≤ 40 nodes, it renders as a normal isometric sub-graph.
/// 3. Components > 40 nodes are recursively split by edge-betweenness (greedy
///    min-cut) until each partition ≤ 40 nodes.
/// 4. The top-level "container view" shows one node per component, sized by
///    node count, with cross-component edges summarized.
///
/// Branch-and-bound: we bound the partition search by node count (branch
/// until each bin ≤ 40) and bound the rendering by viewport size (prioritize
/// largest components if there are too many containers).
pub fn find_connected_components(graph: &InvariantGraph) -> Vec<Vec<String>> {
    let mut adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    for node in &graph.nodes {
        adj.entry(&node.id).or_default();
    }
    for edge in &graph.edges {
        adj.entry(&edge.from).or_default().push(&edge.to);
        adj.entry(&edge.to).or_default().push(&edge.from);
    }

    let mut visited = std::collections::HashSet::new();
    let mut components: Vec<Vec<String>> = Vec::new();

    for node in &graph.nodes {
        if visited.contains(node.id.as_str()) {
            continue;
        }
        let mut comp = Vec::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(node.id.as_str());
        visited.insert(node.id.as_str());
        while let Some(current) = queue.pop_front() {
            comp.push(current.to_string());
            if let Some(neighbors) = adj.get(current) {
                for &neighbor in neighbors {
                    if !visited.contains(neighbor) {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
        components.push(comp);
    }
    components
}

pub fn build_container_graph(
    graph: &InvariantGraph,
    components: &[Vec<String>],
) -> InvariantGraph {
    let node_to_comp: std::collections::HashMap<&str, usize> = components
        .iter()
        .enumerate()
        .flat_map(|(i, ids)| ids.iter().map(move |id| (id.as_str(), i)))
        .collect();

    let mut container_graph = InvariantGraph::new(graph.name.clone());
    for (i, ids) in components.iter().enumerate() {
        let role = dominant_role(graph, ids);
        let label = if ids.len() == 1 {
            let node = graph.nodes.iter().find(|n| n.id == ids[0]);
            node.map_or(ids[0].clone(), |n| n.label.clone())
        } else {
            format!("{} nodes", ids.len())
        };
        let cid = format!("__container_{}", i);
        container_graph = container_graph
            .with_node(
                InvariantNode::new(cid.clone(), label, role)
                    .with_invariant(format!("{} nodes: {}", ids.len(), ids.iter().take(5).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")))
            );
    }

    let mut seen_edges = std::collections::HashSet::new();
    for edge in &graph.edges {
        let from_c = node_to_comp.get(edge.from.as_str());
        let to_c = node_to_comp.get(edge.to.as_str());
        if let (Some(&fc), Some(&tc)) = (from_c, to_c) {
            if fc != tc {
                let key = (fc.min(tc), fc.max(tc));
                if !seen_edges.contains(&key) {
                    seen_edges.insert(key);
                    let label = if components[fc].len() + components[tc].len() > 1 {
                        Some(format!("{}↔{} edges", components[fc].len(), components[tc].len()))
                    } else {
                        None
                    };
                    container_graph = container_graph.with_edge(
                        InvariantEdge::new(
                            format!("__container_{}", fc),
                            format!("__container_{}", tc),
                        )
                        .with_label(label.unwrap_or_default()),
                    );
                }
            }
        }
    }
    container_graph
}

fn dominant_role(graph: &InvariantGraph, ids: &[String]) -> VisualizationRole {
    let mut counts = std::collections::HashMap::new();
    for id in ids {
        if let Some(node) = graph.nodes.iter().find(|n| &n.id == id) {
            *counts.entry(node.role).or_insert(0u32) += 1;
        }
    }
    counts
        .into_iter()
        .max_by_key(|&(_, c)| c)
        .map(|(r, _)| r)
        .unwrap_or(VisualizationRole::Step)
}

pub fn build_component_subgraph(
    graph: &InvariantGraph,
    component_ids: &[String],
    index: usize,
) -> InvariantGraph {
    let id_set: std::collections::HashSet<&str> = component_ids.iter().map(|s| s.as_str()).collect();
    let mut sub = InvariantGraph::new(format!("{}_{}", graph.name, index));
    for node in &graph.nodes {
        if id_set.contains(node.id.as_str()) {
            sub = sub.with_node(node.clone());
        }
    }
    for edge in &graph.edges {
        if id_set.contains(edge.from.as_str()) && id_set.contains(edge.to.as_str()) {
            sub = sub.with_edge(edge.clone());
        }
    }
    sub
}

pub fn graph_to_container_response(graph: &InvariantGraph) -> Result<serde_json::Value, String> {
    let components = find_connected_components(graph);
    if components.len() <= 1 && graph.nodes.len() <= MAX_NODES {
        return graph_to_isometric_response(graph);
    }

    let container_graph = build_container_graph(graph, &components);
    let container_response = graph_to_isometric_response(&container_graph).unwrap_or_else(|e| {
        serde_json::json!({"svg": format!("<svg><text fill='red'>container: {}</text></svg>", e), "format": "isometric"})
    });

    let subgraphs: Vec<serde_json::Value> = components
        .iter()
        .enumerate()
        .filter(|(_, ids)| ids.len() <= MAX_NODES)
        .take(50)
        .map(|(i, ids)| {
            let sub = build_component_subgraph(graph, ids, i);
            let svg = graph_to_isometric_svg(&sub).unwrap_or_else(|e| {
                format!("<svg><text fill='red'>{}</text></svg>", e)
            });
            serde_json::json!({
                "id": format!("__container_{}", i),
                "nodes": ids.len(),
                "svg": svg,
                "node_ids": ids,
            })
        })
        .collect();

    let mut response = container_response;
    if let Some(obj) = response.as_object_mut() {
        obj.insert("components".to_string(), serde_json::json!(subgraphs));
        obj.insert("grouped".to_string(), serde_json::json!(true));
        obj.insert("total_components".to_string(), serde_json::json!(components.len()));
    }
    Ok(response)
}

// ════════ mmdr (mermaid-rs-renderer) native adapter — feature-gated ════════

#[cfg(feature = "mermaid-native")]
pub use mermaid_native::*;

#[cfg(feature = "mermaid-native")]
mod mermaid_native {
    use crate::{InvariantEdge, InvariantGraph, InvariantNode};

    pub fn parse_mermaid_mmdr(text: &str) -> Result<InvariantGraph, String> {
        let parsed =
            mermaid_rs_renderer::parse_mermaid_strict(text).map_err(|e| e.to_string())?;
        let mut graph = InvariantGraph::new("mmdr");
        for (id, node) in &parsed.graph.nodes {
            graph = graph.with_node(InvariantNode::new(
                id.clone(),
                node.label.clone(),
                super::role_from_label(&node.label),
            ));
        }
        for edge in &parsed.graph.edges {
            graph = graph.with_edge(
                InvariantEdge::new(edge.from.clone(), edge.to.clone())
                    .with_label(edge.label.clone().unwrap_or_default()),
            );
        }
        Ok(graph)
    }

    pub fn render_mermaid_native(text: &str) -> Result<String, String> {
        mermaid_rs_renderer::render(text).map_err(|e| e.to_string())
    }
}

fn base64_encode(input: &str) -> String {
    let chars: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut result = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(chars[((triple >> 18) & 0x3f) as usize] as char);
        result.push(chars[((triple >> 12) & 0x3f) as usize] as char);
        result.push(if chunk.len() > 1 { chars[((triple >> 6) & 0x3f) as usize] } else { b'=' } as char);
        result.push(if chunk.len() > 2 { chars[(triple & 0x3f) as usize] } else { b'=' } as char);
    }
    result
}

#[derive(Debug, Clone, Copy)]
struct NodeVars {
    x: Variable,
    y: Variable,
    z: Variable,
}

/// Compute 3D node positions using the kasuari Cassowary constraint solver.
///
/// # Constraint model walkthrough
///
/// Each node gets 3 variables: (x, z, y) in isometric world-space.
/// The solver is given constraints ranked by priority:
///
/// | Priority  | Constraint                          | Why |
/// |-----------|-------------------------------------|-----|
/// | REQUIRED  | `z == layer * LAYER_SPACING`        | Topological order is non-negotiable — sinks come after sources |
/// | REQUIRED  | `y == 0`                            | All nodes stay on the ground plane (flat graph) |
/// | REQUIRED  | `x >= 0`                            | Keep nodes in positive X half-space |
/// | REQUIRED  | `x <= bound`                        | Prevent runaway X expansion |
/// | STRONG    | `x[b] - x[a] >= MIN_X_DIST`         | Nodes in the same layer must not overlap |
/// | STRONG    | `z[to] >= z[from] + LAYER_SPACING`  | Fix backward edges (cycles broken by topological sort) |
/// | MEDIUM    | `|x[to] - x[from]| <= 3 * SPACING`  | Connected nodes stay roughly X-aligned |
/// | WEAK      | `x == preferred_x`                  | Nudge towards grid layout as fallback |
///
/// # Cassowary semantics
///
/// The solver tries to satisfy all REQUIRED constraints first. If impossible,
/// it fails (error). Then it tries to satisfy STRONG constraints, then MEDIUM,
/// then WEAK. Lower-priority constraints are "violated" — this is how the
/// algorithm gracefully degrades: nodes that can't fit in their preferred
/// positions still get placed somewhere valid.
///
/// # Complexity note
///
/// pairwise spacing (STRONG) adds O(n²) constraints per layer. The 40-node
/// cap keeps this practical (<800 comparisons per layer worst-case).
fn kasuari_layout(graph: &InvariantGraph) -> Result<HashMap<String, (f64, f64, f64)>, String> {
    // Guard: Cassowary pairwise spacing is O(n²) per topological layer.
    // Beyond 40 nodes the solver becomes too slow for HTTP response times.
    // Use container grouping for larger graphs.
    if graph.nodes.len() > MAX_NODES {
        return Err(format!(
            "isometric view supports up to {MAX_NODES} nodes (got {}). Use Mermaid for large graphs.",
            graph.nodes.len()
        ));
    }

    let layers = assign_layers(graph);
    let layer_nodes = group_by_layer(graph, &layers);

    let mut solver = Solver::new();
    let mut node_vars: HashMap<String, NodeVars> = HashMap::new();

    for node in &graph.nodes {
        let vars = NodeVars {
            x: Variable::new(),
            y: Variable::new(),
            z: Variable::new(),
        };
        node_vars.insert(node.id.clone(), vars);
    }

    let layer_count = layers.values().max().copied().unwrap_or(0) as f64 + 1.0;

    for node in &graph.nodes {
        let vars = &node_vars[&node.id];
        let layer = *layers.get(&node.id).unwrap_or(&0) as f64;

        solver
            .add_constraints([
                vars.x | GE(Strength::REQUIRED) | 0.0,
                vars.z
                    | EQ(Strength::REQUIRED)
                    | (layer * LAYER_SPACING),
                vars.y | EQ(Strength::REQUIRED) | 0.0,
            ])
            .map_err(|e| format!("constraint error: {e:?}"))?;
    }

    for (layer, ids) in &layer_nodes {
        // Anchor the first node in each layer with a REQUIRED x-position.
        // Without this, conflicting WEAK preferences create multiple valid
        // solutions (push left vs push right) and the solver picks based on
        // internal variable-ID ordering — which varies per process invocation.
        if let Some(first_vars) = ids.first().and_then(|id| node_vars.get(id)) {
            let anchor_x = (*layer as f64) * 0.5 * NODE_SPACING;
            solver
                .add_constraint(first_vars.x | EQ(Strength::REQUIRED) | anchor_x)
                .map_err(|e| format!("constraint error: {e:?}"))?;
        }

        for (i, id) in ids.iter().enumerate() {
            if i == 0 { continue; }
            if let Some(vars) = node_vars.get(id) {
                let pref_x = (*layer as f64) * 0.5 * NODE_SPACING + i as f64 * NODE_SPACING;
                solver
                    .add_constraint(vars.x | EQ(Strength::WEAK) | pref_x)
                    .map_err(|e| format!("constraint error: {e:?}"))?;
            }
        }

        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let a_vars = &node_vars[&ids[i]];
                let b_vars = &node_vars[&ids[j]];
                solver
                    .add_constraint(
                        (b_vars.x - a_vars.x) | GE(Strength::REQUIRED) | MIN_X_DIST,
                    )
                    .map_err(|e| format!("constraint error: {e:?}"))?;
            }
        }
    }

    for edge in &graph.edges {
        let from_layer = *layers.get(&edge.from).unwrap_or(&0);
        let to_layer = *layers.get(&edge.to).unwrap_or(&0);
        if to_layer <= from_layer {
            if let (Some(from_vars), Some(to_vars)) =
                (node_vars.get(&edge.from), node_vars.get(&edge.to))
            {
                solver
                    .add_constraints([
                        to_vars.z | GE(Strength::STRONG) | (from_vars.z + LAYER_SPACING),
                    ])
                    .map_err(|e| format!("constraint error: {e:?}"))?;
            }
        }
        if let (Some(from_vars), Some(to_vars)) =
            (node_vars.get(&edge.from), node_vars.get(&edge.to))
        {
            solver
                .add_constraint(
                    (to_vars.x - from_vars.x) | LE(Strength::MEDIUM)
                        | (3.0 * NODE_SPACING),
                )
                .map_err(|e| format!("constraint error: {e:?}"))?;
            solver
                .add_constraint(
                    (from_vars.x - to_vars.x) | LE(Strength::MEDIUM)
                        | (3.0 * NODE_SPACING),
                )
                .map_err(|e| format!("constraint error: {e:?}"))?;
        }
    }

    for node in &graph.nodes {
        let vars = &node_vars[&node.id];
        solver
            .add_constraint(
                vars.x | LE(Strength::REQUIRED) | (layer_count * NODE_SPACING * 3.0),
            )
            .map_err(|e| format!("constraint error: {e:?}"))?;
    }

    let changes = solver.fetch_changes();
    let mut values: HashMap<Variable, f64> = HashMap::new();
    for &(var, val) in changes {
        values.insert(var, val);
    }

    let mut positions = HashMap::new();
    for node in &graph.nodes {
        if let Some(vars) = node_vars.get(&node.id) {
            let x = values.get(&vars.x).copied().unwrap_or(0.0);
            let y = values.get(&vars.y).copied().unwrap_or(0.0);
            let z = values.get(&vars.z).copied().unwrap_or(0.0);
            positions.insert(node.id.clone(), (x, y, z));
        }
    }

    Ok(positions)
}

fn assign_layers(graph: &InvariantGraph) -> HashMap<String, usize> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();

    for node in &graph.nodes {
        in_degree.entry(&node.id).or_insert(0);
        adj.entry(&node.id).or_default();
    }
    for edge in &graph.edges {
        adj.entry(&edge.from).or_default().push(&edge.to);
        *in_degree.entry(&edge.to).or_insert(0) += 1;
    }

    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(&id, _)| id)
        .collect();

    let mut layers = HashMap::new();

    if queue.is_empty() {
        for (i, node) in graph.nodes.iter().enumerate() {
            layers.insert(node.id.clone(), i);
        }
        return layers;
    }

    let mut current_layer = 0usize;
    while !queue.is_empty() {
        let batch_len = queue.len();
        for _ in 0..batch_len {
            if let Some(id) = queue.pop_front() {
                layers.insert(id.to_string(), current_layer);
                if let Some(children) = adj.get(id) {
                    for &child in children {
                        if let Some(deg) = in_degree.get_mut(child) {
                            *deg = deg.saturating_sub(1);
                            if *deg == 0 {
                                queue.push_back(child);
                            }
                        }
                    }
                }
            }
        }
        current_layer += 1;
    }

    for node in &graph.nodes {
        layers.entry(node.id.clone()).or_insert(current_layer);
    }

    layers
}

/// Detect edges that carry a `Satisfies<Constraint>` relationship.
///
/// A satisfies edge is one whose label matches any of these patterns
/// (case-insensitive prefix match):
///   - `"satisfies"` — exact
///   - `"satisfies: PASS"` — with colon-delimited status
///   - `"satisfies|FAIL|..."` — with pipe-delimited status
fn is_satisfies_edge(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower == "satisfies"
        || lower.starts_with("satisfies:")
        || lower.starts_with("satisfies|")
}

/// Determine the stroke colour for a satisfies edge based on its status payload.
///
/// | Label pattern              | Colour   | Hex       |
/// |----------------------------|----------|-----------|
/// | `satisfies: PASS`          | green    | `#16a34a` |
/// | `satisfies|FAIL|...`       | red      | `#dc2626` |
/// | `satisfies: ?` / `UNKNOWN` | yellow   | `#eab308` |
/// | `satisfies` (bare)         | blue     | `#3b82f6` |
fn satisfies_status_color(label: &str) -> &'static str {
    let lower = label.to_lowercase();
    if lower.contains("pass") {
        "#16a34a"
    } else if lower.contains("fail") {
        "#dc2626"
    } else if lower.contains('?') || lower.contains("unknown") {
        "#eab308"
    } else {
        "#3b82f6"
    }
}

fn group_by_layer(
    graph: &InvariantGraph,
    layers: &HashMap<String, usize>,
) -> HashMap<usize, Vec<String>> {
    let mut groups: HashMap<usize, Vec<String>> = HashMap::new();
    for node in &graph.nodes {
        let layer = layers.get(&node.id).copied().unwrap_or(0);
        groups.entry(layer).or_default().push(node.id.clone());
    }
    groups
}

fn render_svg(
    graph: &InvariantGraph,
    positions: &HashMap<String, (f64, f64, f64)>,
) -> String {
    let scale = 0.6;
    let ox = 400.0;
    let oy = 80.0;

    let mut min_sx = f64::MAX;
    let mut min_sy = f64::MAX;
    let mut max_sx = f64::MIN;
    let mut max_sy = f64::MIN;

    for (_id, &(x, z, y)) in positions {
        let (sx, sy) = iso_project(x, z, y, scale, ox, oy);
        min_sx = min_sx.min(sx - 80.0);
        min_sy = min_sy.min(sy - 50.0);
        max_sx = max_sx.max(sx + 80.0);
        max_sy = max_sy.max(sy + 50.0);
    }

    let padding = 40.0;
    let width = (max_sx - min_sx + padding * 2.0).max(400.0) as u32;
    let height = (max_sy - min_sy + padding * 2.0).max(300.0) as u32;
    let offset_x = -min_sx + padding;
    let offset_y = -min_sy + padding;

    let mut svg = String::new();
    svg.push_str(&format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" style="width:100%;height:100%;max-height:70vh;">"##,
    ));
    svg.push_str(r##"<rect width="100%" height="100%" fill="#0f172a"/>"##);
    svg.push_str(r##"<defs><style>.iso-node {{ cursor:pointer; transition:opacity 0.2s; }} .iso-node:hover {{ opacity:1 !important; }}</style></defs>"##);
    svg.push_str(r##"<g font-family="monospace" font-size="10" class="iso-scene">"##);

    for edge in &graph.edges {
        if let (Some(&(fx, fz, fy)), Some(&(tx, tz, ty))) =
            (positions.get(&edge.from), positions.get(&edge.to))
        {
            let (sx1, sy1) = iso_project(fx, fz, fy, scale, offset_x, offset_y);
            let (sx2, sy2) = iso_project(tx, tz, ty, scale, offset_x, offset_y);
            let mid = (sx1 + sx2) / 2.0;
            let midy = (sy1 + sy2) / 2.0;

            let label = edge.label.as_deref().unwrap_or("");

            // satisfies edges get status-coloured dashed stroked
            if is_satisfies_edge(label) {
                let status_color = satisfies_status_color(label);
                let d = format!(
                    "M {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}",
                    sx1, sy1, mid, midy - 15.0, sx2, sy2
                );
                svg.push_str(&format!(
                    r##"<path d="{}" stroke="{}" stroke-width="1.5" stroke-dasharray="6,4" fill="none" opacity="0.7"/>"##,
                    d, status_color
                ));
                // Show truncated status label on satisfies edges
                if !label.trim().is_empty() {
                    let display_label = if label.len() > 12 {
                        format!("{}…", &label[..11])
                    } else {
                        label.to_string()
                    };
                    svg.push_str(&format!(
                        r##"<text x="{mid:.1}" y="{midy:.1}" text-anchor="middle" font-size="8" fill="{}">{}</text>"##,
                        status_color, escape_xml(&display_label)
                    ));
                }
            } else {
                // Regular edge — existing behaviour unchanged
                let _color = if let Some(from_node) =
                    graph.nodes.iter().find(|n| n.id == edge.from)
                {
                    from_node.role.color()
                } else {
                    "#90a4ae"
                };
                let d = format!(
                    "M {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}",
                    sx1, sy1, mid, midy - 15.0, sx2, sy2
                );
                let edge_label = edge.label.as_ref().map(|l| l.as_str()).unwrap_or("");
                let status = satisfies_status_color(edge_label);
                svg.push_str(&format!(
                    r##"<path d="{}" stroke="{}" stroke-width="1.5" stroke-dasharray="{}" fill="none" opacity="0.5" data-edge-from="{}" data-edge-to="{}" data-edge-status="{}"/>"##,
                    d, status, if is_satisfies_edge(edge_label) { "5,4" } else { "none" }, escape_xml(&edge.from), escape_xml(&edge.to), edge_label
                ));
                if let Some(label) = &edge.label {
                    if !label.trim().is_empty() {
                        svg.push_str(&format!(
                            r##"<text x="{mid:.1}" y="{midy:.1}" text-anchor="middle" font-size="8" fill="#64748b">{}</text>"##,
                            escape_xml(label)
                        ));
                    }
                }
            }
        }
    }

    for node in &graph.nodes {
        if let Some(&(x, z, y)) = positions.get(&node.id) {
            let (sx, sy) = iso_project(x, z, y, scale, offset_x, offset_y);
            let color = node.role.color();
            let hw = 60.0;
            let hh = 18.0;
            let depth = 6.0;
            let short_label = if node.label.len() > 20 {
                format!("{}…", &node.label[..19])
            } else {
                node.label.clone()
            };

            let layer = positions.get(&node.id).map(|&(_, _, z)| z as i32).unwrap_or(0);
            svg.push_str(&format!(
                r##"<g transform="translate({:.1},{:.1})" class="iso-node" data-node-id="{}" data-node-role="{}" data-node-layer="{}">"##,
                sx - hw, sy - hh,
                escape_xml(&node.id), node.role.as_str(), layer
            ));

            let w = hw * 2.0;
            let h = hh * 2.0;
            let wd = w + depth;
            let hd = h + depth;

            // 3D card construction — three layered polygons create depth illusion:
            //
            //  [front face] ── a rounded rect with label, drawn LAST (on top)
            //       ├── [right face] ── parallelogram extruding right-and-down
            //       └── [bottom face] ── parallelogram extruding down
            //
            // The isometric view angle (30° above ground, 45° rotation) means
            // "depth" in 3D translates to (+Δx, +Δy) in 2D screen space.
            // Both the right and bottom faces use the same `depth` offset,
            // colored black with low opacity to simulate shadow/depth perception.

            // Right depth face: connects front-right edge to extruded-right edge
            svg.push_str(&format!(
                r##"<polygon points="{w:.1},{hsub:.1} {wd:.1},{h:.1} {wd:.1},{hd:.1} {w:.1},{hd:.1}" fill="#000" opacity="0.15"/>"##,
                w = w,
                hsub = h - depth,
                wd = wd,
                h = h,
                hd = hd,
            ));
            // Bottom depth face: connects front-bottom edge to extruded-bottom edge
            svg.push_str(&format!(
                r##"<polygon points="0,{h:.1} {w:.1},{h:.1} {wd:.1},{hd:.1} {d:.1},{hd:.1}" fill="#000" opacity="0.1"/>"##,
                h = h,
                w = w,
                wd = wd,
                hd = hd,
                d = depth,
            ));
            // Front face: role-specific polygon shape
            let poly = node.role.polygon();
            let mut points = String::new();
            for (i, &(px, py)) in poly.iter().enumerate() {
                if i > 0 { points.push(' '); }
                let sx = hw + px as f64 * hw;
                let sy = hh + py as f64 * hh;
                points.push_str(&format!("{sx:.1},{sy:.1}"));
            }
            svg.push_str(&format!(
                r##"<polygon points="{points}" fill="{color}" opacity="0.9" stroke="#e2e8f0" stroke-width="0.5"/>"##,
                color = color,
            ));
            svg.push_str(&format!(
                r##"<text x="{x:.1}" y="{y:.1}" text-anchor="middle" fill="#fff" font-size="9" font-weight="bold">{label}</text>"##,
                x = hw,
                y = hh + 3.0,
                label = escape_xml(&short_label),
            ));
            // Render invariant subtitle below the main label if present
            if let Some(invariant) = &node.invariant {
                let inv_short = if invariant.len() > 28 {
                    format!("{}…", &invariant[..27])
                } else {
                    invariant.clone()
                };
                svg.push_str(&format!(
                    r##"<text x="{x:.1}" y="{y:.1}" text-anchor="middle" fill="#cbd5e1" font-size="7" opacity="0.8">{inv}</text>"##,
                    x = hw,
                    y = hh + 12.0,
                    inv = escape_xml(&inv_short),
                ));
            }
            svg.push_str(&format!(
                r##"<title>{full}</title>"##,
                full = escape_xml(&node.label),
            ));
            svg.push_str("</g>");
        }
    }

    svg.push_str("</g></svg>");
    svg
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_flow() {
        let mermaid = "flowchart LR\n    A[\"Source\"]\n    B[\"Target\"]\n    A --> B";
        let graph = parse_mermaid(mermaid).expect("parse");
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert!(graph.validate().is_ok());
    }

    #[test]
    fn parse_edge_with_label() {
        let mermaid = "graph LR\n    x[\"Start\"]\n    y[\"End\"]\n    x -->|transform| y";
        let graph = parse_mermaid(mermaid).expect("parse");
        assert_eq!(graph.edges[0].label.as_deref(), Some("transform"));
    }

    #[test]
    fn parse_paren_node_syntax() {
        let mermaid = "flowchart TD\n    thing(\"A Thing\")\n    other(\"Other\")\n    thing --> other";
        let graph = parse_mermaid(mermaid).expect("parse");
        assert_eq!(graph.nodes.len(), 2);
        let labels: Vec<&str> = graph.nodes.iter().map(|n| n.label.as_str()).collect();
        assert!(labels.contains(&"A Thing"));
        assert!(labels.contains(&"Other"));
    }

    #[test]
    fn role_detection() {
        assert_eq!(
            role_from_label("ingest docs"),
            VisualizationRole::Ingest
        );
        assert_eq!(
            role_from_label("validate input"),
            VisualizationRole::Validate
        );
        assert_eq!(
            role_from_label("classify transaction"),
            VisualizationRole::Classify
        );
        assert_eq!(
            role_from_label("review output"),
            VisualizationRole::Review
        );
        assert_eq!(
            role_from_label("reconcile accounts"),
            VisualizationRole::Reconcile
        );
        assert_eq!(
            role_from_label("commit result"),
            VisualizationRole::Commit
        );
        assert_eq!(
            role_from_label("decision point"),
            VisualizationRole::Decision
        );
        assert_eq!(role_from_label("unknown thing"), VisualizationRole::Step);
    }

    #[test]
    fn empty_mermaid() {
        assert!(parse_mermaid("").is_err());
    }

    #[test]
    fn layout_produces_positions() {
        let graph = InvariantGraph::new("test")
            .with_node(InvariantNode::new(
                "a",
                "Alpha",
                VisualizationRole::Ingest,
            ))
            .with_node(InvariantNode::new(
                "b",
                "Beta",
                VisualizationRole::Validate,
            ))
            .with_edge(InvariantEdge::new("a", "b"));

        let positions = kasuari_layout(&graph).expect("layout");
        assert_eq!(positions.len(), 2);
        assert!(positions.contains_key("a"));
        assert!(positions.contains_key("b"));
        let (_, _, z_a) = positions["a"];
        let (_, _, z_b) = positions["b"];
        assert!(
            z_b >= z_a,
            "target should be at same or later Z layer"
        );
    }

    #[test]
    fn svg_output_contains_elements() {
        let graph = InvariantGraph::new("svg-test")
            .with_node(InvariantNode::new(
                "x",
                "X Node",
                VisualizationRole::Ingest,
            ))
            .with_node(InvariantNode::new(
                "y",
                "Y Node",
                VisualizationRole::Commit,
            ))
            .with_edge(InvariantEdge::new("x", "y"));

        let svg = graph_to_isometric_svg(&graph).expect("svg");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("X Node"));
        assert!(svg.contains("Y Node"));
        assert!(svg.contains("#0f172a"));
    }

    #[test]
    fn mermaid_roundtrip() {
        let mermaid = "flowchart LR\n    a[\"Input\"]\n    b[\"Process\"]\n    c[\"Output\"]\n    a --> b\n    b --> c";
        let svg = mermaid_to_isometric_svg(mermaid).expect("roundtrip");
        assert!(svg.contains("Input"));
        assert!(svg.contains("Process"));
        assert!(svg.contains("Output"));
    }

    #[test]
    fn graph_with_only_nodes() {
        let graph = InvariantGraph::new("lonely").with_node(InvariantNode::new(
            "solo",
            "Solo",
            VisualizationRole::Step,
        ));
        let svg = graph_to_isometric_svg(&graph).expect("svg");
        assert!(svg.contains("Solo"));
    }

    #[test]
    fn empty_graph_svg() {
        let graph = InvariantGraph::new("void");
        let svg = graph_to_isometric_svg(&graph).expect("svg");
        assert!(svg.contains("No nodes"));
    }

    #[test]
    fn tax_lawyer_demo_isometric() {
        let graph = crate::tax_lawyer_demo();
        assert!(graph.validate().is_ok());
        assert_eq!(graph.nodes.len(), 8);
        assert_eq!(graph.edges.len(), 10);

        let svg = graph_to_isometric_svg(&graph).expect("svg");
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Xero Invoice"));
        assert!(svg.contains("Classify R&amp;D Expenditure"));
        assert!(svg.contains("Registered R&amp;D Activity"));
        assert!(svg.contains("Eligibility Check"));
        assert!(svg.contains("R&amp;D Offset 43.5%"));
        assert!(svg.contains("AU GST Check"));
        assert!(svg.contains("Evidence Chain"));
        assert!(svg.contains("CPA Review"));

        // Verify invariant subtitles rendered
        assert!(svg.contains("s.355-305, contractor"));

        // Verify at least one satisfies edge is present
        let satisfies_count = graph
            .edges
            .iter()
            .filter(|e| {
                e.label
                    .as_deref()
                    .is_some_and(|l| is_satisfies_edge(l))
            })
            .count();
        assert!(
            satisfies_count >= 1,
            "expected at least one satisfies edge, got {satisfies_count}"
        );

        // Verify dashed stroke appears for satisfies edges in SVG
        assert!(svg.contains("stroke-dasharray"));
    }

    #[test]
    fn satisfies_edge_coloring() {
        // Test PASS → green
        {
            let graph = InvariantGraph::new("pass-test")
                .with_node(InvariantNode::new("a", "Alpha", VisualizationRole::Step))
                .with_node(InvariantNode::new("b", "Beta", VisualizationRole::Step))
                .with_edge(InvariantEdge::new("a", "b").with_label("satisfies: PASS"));
            let svg = graph_to_isometric_svg(&graph).expect("svg pass");
            assert!(svg.contains("#16a34a"), "PASS edge should be green");
            assert!(svg.contains("satisfies: …"), "PASS label should be truncated");
            assert!(svg.contains("stroke-dasharray"), "satisfies edge should be dashed");
        }

        // Test FAIL → red
        {
            let graph = InvariantGraph::new("fail-test")
                .with_node(InvariantNode::new("a", "Alpha", VisualizationRole::Step))
                .with_node(InvariantNode::new("b", "Beta", VisualizationRole::Step))
                .with_edge(InvariantEdge::new("a", "b").with_label("satisfies|FAIL|s.355-305"));
            let svg = graph_to_isometric_svg(&graph).expect("svg fail");
            assert!(svg.contains("#dc2626"), "FAIL edge should be red");
            assert!(svg.contains("stroke-dasharray"), "satisfies edge should be dashed");
        }

        // Test bare satisfies → blue
        {
            let graph = InvariantGraph::new("bare-test")
                .with_node(InvariantNode::new("a", "Alpha", VisualizationRole::Step))
                .with_node(InvariantNode::new("b", "Beta", VisualizationRole::Step))
                .with_edge(InvariantEdge::new("a", "b").with_label("satisfies"));
            let svg = graph_to_isometric_svg(&graph).expect("svg bare");
            assert!(svg.contains("#3b82f6"), "unqualified satisfies edge should be blue");
        }

        // Test unknown → yellow
        {
            let graph = InvariantGraph::new("unknown-test")
                .with_node(InvariantNode::new("a", "Alpha", VisualizationRole::Step))
                .with_node(InvariantNode::new("b", "Beta", VisualizationRole::Step))
                .with_edge(InvariantEdge::new("a", "b").with_label("satisfies: ?"));
            let svg = graph_to_isometric_svg(&graph).expect("svg unknown");
            assert!(svg.contains("#eab308"), "UNKNOWN satisfies edge should be yellow");
        }
    }

    #[test]
    fn gltf_export_contains_scene_data() {
        let graph = InvariantGraph::new("gltf-test")
            .with_node(InvariantNode::new("a", "Alpha", VisualizationRole::Ingest))
            .with_node(InvariantNode::new("b", "Beta", VisualizationRole::Validate))
            .with_edge(InvariantEdge::new("a", "b"));

        let positions = kasuari_layout(&graph).expect("layout");
        let gltf = scene_to_gltf_data_uri(&graph, &positions).expect("gltf");
        assert!(gltf.starts_with("data:model/gltf+json;base64,"));
        assert!(gltf.len() > 200);
    }
}
