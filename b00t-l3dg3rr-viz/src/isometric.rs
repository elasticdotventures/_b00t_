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
    let lower = label.to_lowercase();
    if lower.contains("ingest") || lower.contains("document") {
        VisualizationRole::Ingest
    } else if lower.contains("valid") || lower.contains("verify") || lower.contains("check") {
        VisualizationRole::Validate
    } else if lower.contains("classif") || lower.contains("categor") || lower.contains("tag") {
        VisualizationRole::Classify
    } else if lower.contains("review") || lower.contains("inspect") || lower.contains("audit") {
        VisualizationRole::Review
    } else if lower.contains("reconcil") || lower.contains("match") || lower.contains("balance") {
        VisualizationRole::Reconcile
    } else if lower.contains("commit") || lower.contains("save") || lower.contains("write") {
        VisualizationRole::Commit
    } else if lower.contains("decis") || lower.contains("if") || lower.contains("branch") {
        VisualizationRole::Decision
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

pub fn mermaid_to_isometric_svg(mermaid_text: &str) -> Result<String, String> {
    let graph = parse_mermaid(mermaid_text).map_err(|e| e.to_string())?;
    if let Err(e) = graph.validate() {
        return Err(format!("graph validation failed: {e}"));
    }
    graph_to_isometric_svg(&graph)
}

#[derive(Debug, Clone, Copy)]
struct NodeVars {
    x: Variable,
    y: Variable,
    z: Variable,
}

fn kasuari_layout(graph: &InvariantGraph) -> Result<HashMap<String, (f64, f64, f64)>, String> {
    const MAX_NODES: usize = 40;
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
        for (i, id) in ids.iter().enumerate() {
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
                    .add_constraint((b_vars.x - a_vars.x) | GE(Strength::STRONG) | MIN_X_DIST)
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

    let role_colors: HashMap<VisualizationRole, &str> = [
        (VisualizationRole::Ingest, "#4fc3f7"),
        (VisualizationRole::Validate, "#66bb6a"),
        (VisualizationRole::Classify, "#ffa726"),
        (VisualizationRole::Review, "#ab47bc"),
        (VisualizationRole::Reconcile, "#26c6da"),
        (VisualizationRole::Commit, "#42a5f5"),
        (VisualizationRole::Decision, "#ef5350"),
        (VisualizationRole::Step, "#78909c"),
    ]
    .into_iter()
    .collect();

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
            let color = if let Some(from_node) =
                graph.nodes.iter().find(|n| n.id == edge.from)
            {
                role_colors
                    .get(&from_node.role)
                    .copied()
                    .unwrap_or("#90a4ae")
            } else {
                "#90a4ae"
            };
            let d = format!(
                "M {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}",
                sx1, sy1, mid, midy - 15.0, sx2, sy2
            );
            svg.push_str(&format!(
                r##"<path d="{}" stroke="{}" stroke-width="1" fill="none" opacity="0.4"/>"##,
                d, color
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

    for node in &graph.nodes {
        if let Some(&(x, z, y)) = positions.get(&node.id) {
            let (sx, sy) = iso_project(x, z, y, scale, offset_x, offset_y);
            let color = role_colors
                .get(&node.role)
                .copied()
                .unwrap_or("#78909c");
            let hw = 60.0;
            let hh = 18.0;
            let depth = 6.0;
            let short_label = if node.label.len() > 20 {
                format!("{}…", &node.label[..19])
            } else {
                node.label.clone()
            };

            svg.push_str(&format!(
                r##"<g transform="translate({:.1},{:.1})" class="iso-node">"##,
                sx - hw,
                sy - hh
            ));

            let w = hw * 2.0;
            let h = hh * 2.0;
            let wd = w + depth;
            let hd = h + depth;

            svg.push_str(&format!(
                r##"<polygon points="{w:.1},{hsub:.1} {wd:.1},{h:.1} {wd:.1},{hd:.1} {w:.1},{hd:.1}" fill="#000" opacity="0.15"/>"##,
                w = w,
                hsub = h - depth,
                wd = wd,
                h = h,
                hd = hd,
            ));
            svg.push_str(&format!(
                r##"<polygon points="0,{h:.1} {w:.1},{h:.1} {wd:.1},{hd:.1} {d:.1},{hd:.1}" fill="#000" opacity="0.1"/>"##,
                h = h,
                w = w,
                wd = wd,
                hd = hd,
                d = depth,
            ));
            svg.push_str(&format!(
                r##"<rect x="0" y="0" width="{w:.1}" height="{h:.1}" rx="4" fill="{color}" opacity="0.9" stroke="#e2e8f0" stroke-width="0.5"/>"##,
                w = w,
                h = h,
                color = color,
            ));
            svg.push_str(&format!(
                r##"<text x="{x:.1}" y="{y:.1}" text-anchor="middle" fill="#fff" font-size="9" font-weight="bold">{label}</text>"##,
                x = hw,
                y = hh + 3.0,
                label = escape_xml(&short_label),
            ));
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
}
