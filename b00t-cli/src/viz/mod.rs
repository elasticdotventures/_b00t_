//! Visualization primitives and domain graph adapters.
//!
//! This mirrors l3dg3rr's `b00t-iface::viz` surface so b00t can emit the
//! same scene shape without vendoring the whole l3dg3rr workspace.

use crate::blessing::BlessingGraph;
use crate::commands::task::{Task, TaskStatus};
use crate::datum_utils::{DatumGraph, DatumGraphEdge, DatumGraphNode};
use crate::DatumType;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub const ISO_SCALE_X: f32 = 0.8660254;
pub const ISO_SCALE_Y: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3D {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
}

pub fn iso_project(p: Point3D, scale: f32, origin: Point2D) -> Point2D {
    Point2D {
        x: origin.x + (p.x - p.z) * scale * ISO_SCALE_X,
        y: origin.y + (p.x + p.z) * scale * ISO_SCALE_Y - p.y * scale,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRole {
    Ingest,
    Validate,
    Classify,
    Review,
    Reconcile,
    Commit,
    Decision,
    Step,
}

impl SemanticRole {
    pub fn color(self) -> &'static str {
        match self {
            Self::Ingest => "#4fc3f7",
            Self::Validate => "#66bb6a",
            Self::Classify => "#ffa726",
            Self::Review => "#ab47bc",
            Self::Reconcile => "#26c6da",
            Self::Commit => "#42a5f5",
            Self::Decision => "#ef5350",
            Self::Step => "#78909c",
        }
    }
}

impl std::fmt::Display for SemanticRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let role = match self {
            Self::Ingest => "ingest",
            Self::Validate => "validate",
            Self::Classify => "classify",
            Self::Review => "review",
            Self::Reconcile => "reconcile",
            Self::Commit => "commit",
            Self::Decision => "decision",
            Self::Step => "step",
        };
        write!(f, "{role}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneNode {
    pub id: String,
    pub label: String,
    pub position: Point3D,
    pub role: SemanticRole,
    pub arm_index: Option<u32>,
    pub is_default: bool,
}

impl SceneNode {
    pub fn project(&self, scale: f32, origin: Point2D) -> Point2D {
        iso_project(self.position, scale, origin)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub is_bezier: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneGraph {
    pub nodes: Vec<SceneNode>,
    pub edges: Vec<SceneEdge>,
    pub scale: f32,
    pub origin: Point2D,
}

impl SceneGraph {
    pub fn new(scale: f32) -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            scale,
            origin: Point2D { x: 400.0, y: 300.0 },
        }
    }

    pub fn add_node(&mut self, node: SceneNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: SceneEdge) {
        self.edges.push(edge);
    }

    pub fn bounding_box(&self) -> Option<(Point2D, Point2D)> {
        let mut iter = self.nodes.iter();
        let first = iter.next()?;
        let first = first.project(self.scale, self.origin);
        let mut min_x = first.x;
        let mut min_y = first.y;
        let mut max_x = first.x;
        let mut max_y = first.y;

        for node in iter {
            let p = node.project(self.scale, self.origin);
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x);
            max_y = max_y.max(p.y);
        }

        Some((
            Point2D { x: min_x, y: min_y },
            Point2D { x: max_x, y: max_y },
        ))
    }
}

#[derive(Debug, Clone)]
pub struct SceneTheme {
    pub node_width: f32,
    pub node_height: f32,
    pub font_size: u32,
    pub edge_color: String,
    pub grid_color: String,
}

impl Default for SceneTheme {
    fn default() -> Self {
        Self {
            node_width: 168.0,
            node_height: 72.0,
            font_size: 13,
            edge_color: "#64748b".into(),
            grid_color: "#e2e8f0".into(),
        }
    }
}

pub fn scene_to_svg(scene: &SceneGraph, theme: &SceneTheme) -> String {
    let (width, height) = match scene.bounding_box() {
        Some((min, max)) => {
            let padding = 80.0;
            (
                (max.x - min.x + padding * 2.0).max(800.0),
                (max.y - min.y + padding * 2.0).max(600.0),
            )
        }
        None => (800.0, 600.0),
    };

    let mut svg = String::new();
    svg.push_str(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0} {height:.0}" width="{width:.0}" height="{height:.0}">"#
    ));
    svg.push_str(&format!(
        r##"<defs><marker id="arrow" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="6" markerHeight="6" orient="auto"><path d="M 0 0 L 10 5 L 0 10 Z" fill="{}"/></marker><pattern id="grid" width="40" height="40" patternUnits="userSpaceOnUse"><path d="M 40 0 L 0 0 0 40" fill="none" stroke="{}" stroke-width="0.5"/></pattern></defs>"##,
        theme.edge_color,
        theme.grid_color
    ));
    svg.push_str(&format!(
        r#"<rect width="{width:.0}" height="{height:.0}" fill="url(#grid)"/>"#
    ));

    for edge in &scene.edges {
        let from_node = scene.nodes.iter().find(|n| n.id == edge.from);
        let to_node = scene.nodes.iter().find(|n| n.id == edge.to);
        if let (Some(from_node), Some(to_node)) = (from_node, to_node) {
            let from = from_node.project(scene.scale, scene.origin);
            let to = to_node.project(scene.scale, scene.origin);
            let path = if edge.is_bezier {
                let cpx = (from.x + to.x) / 2.0;
                let cpy = (from.y + to.y) / 2.0 - 32.0;
                format!(
                    "M {:.1} {:.1} Q {:.1} {:.1} {:.1} {:.1}",
                    from.x, from.y, cpx, cpy, to.x, to.y
                )
            } else {
                format!("M {:.1} {:.1} L {:.1} {:.1}", from.x, from.y, to.x, to.y)
            };
            svg.push_str(&format!(
                r#"<path d="{}" stroke="{}" stroke-width="2" fill="none" marker-end="url(#arrow)"/>"#,
                escape_xml(&path),
                theme.edge_color
            ));
        }
    }

    for node in &scene.nodes {
        let p = node.project(scene.scale, scene.origin);
        let x = p.x - theme.node_width / 2.0;
        let y = p.y - theme.node_height / 2.0;
        let color = node.role.color();
        svg.push_str(&format!(r#"<g transform="translate({x:.1},{y:.1})">"#));
        svg.push_str(&format!(
            r##"<rect x="0" y="0" width="{:.1}" height="{:.1}" rx="6" fill="{}" stroke="#ffffff" stroke-width="1"/>"##,
            theme.node_width, theme.node_height, color
        ));
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="30" text-anchor="middle" font-size="{}" fill="#ffffff" font-family="system-ui, sans-serif">{}</text>"##,
            theme.node_width / 2.0,
            theme.font_size,
            escape_xml(&node.label)
        ));
        svg.push_str(&format!(
            r##"<text x="{:.1}" y="52" text-anchor="middle" font-size="10" fill="#f8fafc" font-family="system-ui, sans-serif">{}</text>"##,
            theme.node_width / 2.0,
            node.role
        ));
        svg.push_str("</g>");
    }

    svg.push_str("</svg>");
    svg
}

pub fn blessing_to_scene(graph: &BlessingGraph) -> SceneGraph {
    let levels = graph_levels(
        graph.nodes.iter().map(|node| node.id.clone()).collect(),
        graph
            .edges
            .iter()
            .map(|edge| (edge.from.clone(), edge.to.clone()))
            .collect(),
    );
    let mut scene = SceneGraph::new(48.0);
    for (idx, node) in graph.nodes.iter().enumerate() {
        let depth = *levels.get(&node.id).unwrap_or(&0) as f32;
        scene.add_node(SceneNode {
            id: node.id.clone(),
            label: format!("{} cost={}", node.id, node.cost_tokens),
            position: Point3D::new(idx as f32 * 2.0, depth, depth),
            role: SemanticRole::Decision,
            arm_index: Some(idx as u32),
            is_default: idx == 0,
        });
    }
    for edge in &graph.edges {
        scene.add_edge(SceneEdge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label: Some(edge.relationship.clone()),
            is_bezier: true,
        });
    }
    scene
}

pub fn blessing_to_mermaid(graph: &BlessingGraph) -> String {
    let mut out = String::from("graph TD\n");
    for node in &graph.nodes {
        out.push_str(&format!(
            "    {}[\"{}\"]\n",
            mermaid_id(&node.id),
            mermaid_label(&format!(
                "{}\\ncost={}\\nroles={}",
                node.id,
                node.cost_tokens,
                node.role_access.join(",")
            ))
        ));
    }
    for edge in &graph.edges {
        out.push_str(&format!(
            "    {} -->|{}| {}\n",
            mermaid_id(&edge.from),
            mermaid_label(&edge.relationship),
            mermaid_id(&edge.to)
        ));
    }
    out
}

pub fn blessing_to_rhai_dsl(graph: &BlessingGraph) -> String {
    let mut out = String::new();
    for node in &graph.nodes {
        let children: Vec<String> = graph
            .edges
            .iter()
            .filter(|edge| edge.from == node.id)
            .map(|edge| format!(r#""{}""#, escape_rhai(&edge.to)))
            .collect();
        out.push_str(&format!(
            "role(\"{}\", {{ cost: {}, roles: [{}], children: [{}] }})\n",
            escape_rhai(&node.id),
            node.cost_tokens,
            node.role_access
                .iter()
                .map(|role| format!(r#""{}""#, escape_rhai(role)))
                .collect::<Vec<_>>()
                .join(", "),
            children.join(", ")
        ));
    }
    out
}

pub fn tasks_to_scene(tasks: &[Task]) -> SceneGraph {
    let ids: Vec<String> = tasks.iter().map(|task| task.id.to_string()).collect();
    let edges: Vec<(String, String)> = tasks
        .iter()
        .flat_map(|task| {
            task.dependencies
                .iter()
                .map(move |dep| (dep.to_string(), task.id.to_string()))
        })
        .collect();
    let levels = graph_levels(ids, edges);
    let mut scene = SceneGraph::new(52.0);
    for (idx, task) in tasks.iter().enumerate() {
        let id = task.id.to_string();
        let depth = *levels.get(&id).unwrap_or(&0) as f32;
        scene.add_node(SceneNode {
            id: id.clone(),
            label: format!("#{} {} P{}", task.id, task.title, task.priority),
            position: Point3D::new(idx as f32 * 2.0, depth, depth),
            role: task_role(&task.status),
            arm_index: Some(idx as u32),
            is_default: idx == 0,
        });
        for dep in &task.dependencies {
            scene.add_edge(SceneEdge {
                from: dep.to_string(),
                to: id.clone(),
                label: Some("depends_on".into()),
                is_bezier: true,
            });
        }
    }
    scene
}

pub fn tasks_to_mermaid(tasks: &[Task]) -> String {
    let mut out = String::from("graph LR\n");
    for task in tasks {
        out.push_str(&format!(
            "    T{}[\"{}\"]\n",
            task.id,
            mermaid_label(&format!(
                "#{} {}\\nstatus={}\\npriority={}",
                task.id, task.title, task.status, task.priority
            ))
        ));
    }
    for task in tasks {
        for dep in &task.dependencies {
            out.push_str(&format!("    T{} --> T{}\n", dep, task.id));
        }
    }
    out
}

pub fn tasks_to_rhai_dsl(tasks: &[Task]) -> String {
    let mut out = String::new();
    for task in tasks {
        out.push_str(&format!(
            "task(\"{}\", {{ status: \"{}\", priority: {}, blocked_by: [{}] }})\n",
            task.id,
            task.status,
            task.priority,
            task.dependencies
                .iter()
                .map(|dep| format!(r#""{}""#, dep))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    out
}

pub fn datum_graph_to_scene(graph: &DatumGraph) -> SceneGraph {
    let levels = graph_levels(
        graph.nodes.iter().map(|node| node.key.clone()).collect(),
        graph
            .edges
            .iter()
            .map(|edge| (edge.from.clone(), edge.to.clone()))
            .collect(),
    );
    let mut scene = SceneGraph::new(34.0);
    for (idx, node) in graph.nodes.iter().enumerate() {
        let depth = *levels.get(&node.key).unwrap_or(&0) as f32;
        scene.add_node(SceneNode {
            id: node.key.clone(),
            label: node.key.clone(),
            position: Point3D::new(idx as f32, depth, depth),
            role: SemanticRole::Step,
            arm_index: Some(idx as u32),
            is_default: idx == 0,
        });
    }
    for edge in &graph.edges {
        scene.add_edge(SceneEdge {
            from: edge.from.clone(),
            to: edge.to.clone(),
            label: Some(edge.edge_type.clone()),
            is_bezier: true,
        });
    }
    scene
}

pub fn datum_graph_to_mermaid(graph: &DatumGraph) -> String {
    let mut out = String::from("graph LR\n");
    for node in &graph.nodes {
        let datum_type = node
            .datum_type
            .as_ref()
            .map(|dtype| format!("{dtype:?}"))
            .unwrap_or_else(|| "?".into());
        out.push_str(&format!(
            "    {}[\"{}\"]\n",
            mermaid_id(&node.key),
            mermaid_label(&format!("{}\\n{}", node.key, datum_type))
        ));
    }
    for edge in &graph.edges {
        out.push_str(&format!(
            "    {} -->|{}| {}\n",
            mermaid_id(&edge.from),
            mermaid_label(&edge.edge_type),
            mermaid_id(&edge.to)
        ));
    }
    out
}

pub fn datum_graph_to_rhai_dsl(graph: &DatumGraph) -> String {
    let mut out = String::new();
    for node in &graph.nodes {
        out.push_str(&format!(
            "datum(\"{}\", {{ type: \"{}\" }})\n",
            escape_rhai(&node.key),
            node.datum_type
                .as_ref()
                .map(|dtype| format!("{dtype:?}"))
                .unwrap_or_else(|| "unknown".into())
        ));
    }
    for edge in &graph.edges {
        out.push_str(&format!(
            "entangles(\"{}\", \"{}\", \"{}\")\n",
            escape_rhai(&edge.from),
            escape_rhai(&edge.to),
            escape_rhai(&edge.edge_type)
        ));
    }
    out
}

pub fn scene_to_ascii(scene: &SceneGraph) -> String {
    let mut out = String::new();
    out.push_str("nodes:\n");
    for node in &scene.nodes {
        out.push_str(&format!("  - {} [{}]\n", node.label, node.role));
    }
    out.push_str("edges:\n");
    for edge in &scene.edges {
        let label = edge.label.as_deref().unwrap_or("edge");
        out.push_str(&format!("  - {} -> {} ({})\n", edge.from, edge.to, label));
    }
    out
}

fn task_role(status: &TaskStatus) -> SemanticRole {
    match status {
        TaskStatus::Pending => SemanticRole::Step,
        TaskStatus::InProgress => SemanticRole::Review,
        TaskStatus::Done => SemanticRole::Commit,
        TaskStatus::Blocked => SemanticRole::Decision,
        TaskStatus::Deferred => SemanticRole::Reconcile,
    }
}

fn graph_levels(nodes: Vec<String>, edges: Vec<(String, String)>) -> HashMap<String, usize> {
    let mut incoming: HashMap<String, Vec<String>> = HashMap::new();
    for node in &nodes {
        incoming.entry(node.clone()).or_default();
    }
    for (from, to) in edges {
        incoming.entry(to).or_default().push(from);
    }

    fn depth(
        node: &str,
        incoming: &HashMap<String, Vec<String>>,
        memo: &mut HashMap<String, usize>,
        stack: &mut Vec<String>,
    ) -> usize {
        if let Some(value) = memo.get(node) {
            return *value;
        }
        if stack.iter().any(|item| item == node) {
            return 0;
        }
        stack.push(node.to_string());
        let value = incoming
            .get(node)
            .map(|parents| {
                parents
                    .iter()
                    .map(|parent| depth(parent, incoming, memo, stack) + 1)
                    .max()
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        stack.pop();
        memo.insert(node.to_string(), value);
        value
    }

    let mut memo = HashMap::new();
    for node in nodes {
        let _ = depth(&node, &incoming, &mut memo, &mut Vec::new());
    }
    BTreeMap::from_iter(memo).into_iter().collect()
}

fn mermaid_id(raw: &str) -> String {
    let mut out = String::from("N");
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn mermaid_label(raw: &str) -> String {
    raw.replace('"', "'")
}

fn escape_rhai(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

fn escape_xml(raw: &str) -> String {
    raw.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blessing::{BlessingEdge, BlessingNode};

    #[test]
    fn iso_project_matches_l3dg3rr_contract() {
        let p = iso_project(
            Point3D::new(1.0, 0.0, 0.0),
            1.0,
            Point2D { x: 400.0, y: 300.0 },
        );
        assert!((p.x - 400.866).abs() < 0.001);
        assert!((p.y - 300.5).abs() < 0.001);
    }

    #[test]
    fn blessing_graph_emits_mermaid_and_scene() {
        let graph = BlessingGraph {
            nodes: vec![
                BlessingNode {
                    id: "executive".into(),
                    type_: "role".into(),
                    cost_tokens: 10,
                    role_access: vec!["executive".into()],
                    ..Default::default()
                },
                BlessingNode {
                    id: "operator".into(),
                    type_: "role".into(),
                    cost_tokens: 5,
                    role_access: vec!["operator".into()],
                    ..Default::default()
                },
            ],
            edges: vec![BlessingEdge {
                from: "executive".into(),
                to: "operator".into(),
                relationship: "delegates".into(),
            }],
        };
        let mermaid = blessing_to_mermaid(&graph);
        assert!(mermaid.contains("graph TD"));
        assert!(mermaid.contains("delegates"));
        let scene = blessing_to_scene(&graph);
        assert_eq!(scene.nodes.len(), 2);
        assert_eq!(scene.edges.len(), 1);
        assert!(blessing_to_rhai_dsl(&graph).contains("role(\"executive\""));
    }

    #[test]
    fn tasks_emit_dependency_edges() {
        let tasks = vec![
            Task {
                id: 1,
                title: "first".into(),
                description: None,
                status: TaskStatus::Done,
                priority: 1,
                tags: vec![],
                dependencies: vec![],
                notes: None,
                acceptance_criteria: vec![],
                created_at: "2026-05-03T00:00:00Z".into(),
                updated_at: None,
            },
            Task {
                id: 2,
                title: "second".into(),
                description: None,
                status: TaskStatus::Pending,
                priority: 2,
                tags: vec![],
                dependencies: vec![1],
                notes: None,
                acceptance_criteria: vec![],
                created_at: "2026-05-03T00:00:00Z".into(),
                updated_at: None,
            },
        ];
        let mermaid = tasks_to_mermaid(&tasks);
        assert!(mermaid.contains("T1 --> T2"));
        let scene = tasks_to_scene(&tasks);
        assert_eq!(scene.edges.len(), 1);
        assert!(tasks_to_rhai_dsl(&tasks).contains("blocked_by: [\"1\"]"));
        assert!(scene_to_ascii(&scene).contains("depends_on"));
    }

    #[test]
    fn datum_graph_emits_mermaid_and_scene() {
        let graph = DatumGraph {
            nodes: vec![
                DatumGraphNode {
                    key: "b00t.cli".into(),
                    name: "b00t".into(),
                    datum_type: Some(DatumType::Cli),
                    hint: "b00t CLI tool".into(),
                },
                DatumGraphNode {
                    key: "rust.c".into(),
                    name: "rustc".into(),
                    datum_type: Some(DatumType::Cli),
                    hint: "Rust compiler".into(),
                },
            ],
            edges: vec![DatumGraphEdge {
                from: "b00t.cli".into(),
                to: "rust.c".into(),
                edge_type: "depends_on".into(),
            }],
        };
        let mermaid = datum_graph_to_mermaid(&graph);
        assert!(mermaid.contains("b00t.cli"));
        assert!(mermaid.contains("rust.c"));
        assert!(mermaid.contains("-->"));
        let scene = datum_graph_to_scene(&graph);
        assert_eq!(scene.nodes.len(), 2);
        assert_eq!(scene.edges.len(), 1);
    }

    #[test]
    fn datum_graph_rhai_dsl_emits_entanglement() {
        let graph = DatumGraph {
            nodes: vec![DatumGraphNode {
                key: "git.cli".into(),
                name: "git".into(),
                datum_type: Some(DatumType::Cli),
                hint: "Git VCS".into(),
            }],
            edges: vec![],
        };
        let rhai = datum_graph_to_rhai_dsl(&graph);
        assert!(rhai.contains("datum(\"git.cli\""));
        assert!(rhai.contains("type: \"Cli\""));
    }

    #[test]
    fn empty_datum_graph_produces_empty_scene() {
        let graph = DatumGraph { nodes: vec![], edges: vec![] };
        let scene = datum_graph_to_scene(&graph);
        assert_eq!(scene.nodes.len(), 0);
        assert_eq!(scene.edges.len(), 0);
    }
}
