//! Generic l3dg3rr invariant graph utilities for b00t and peer systems.
//!
//! The upstream `PromptExecution/l3dg3rr` skills encode operational workflows
//! around typed invariants, visual audit graphs, and supervised agent surfaces.
//! This crate repackages the reusable part as a small host-neutral contract:
//! implement or build an [`InvariantGraph`], validate its type invariants, then
//! render Mermaid or deterministic SVG documentation.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Stable visual category shared by l3dg3rr docs and b00t capability graphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualizationRole {
    Ingest,
    Validate,
    Classify,
    Review,
    Reconcile,
    Commit,
    Decision,
    Step,
}

impl VisualizationRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ingest => "ingest",
            Self::Validate => "validate",
            Self::Classify => "classify",
            Self::Review => "review",
            Self::Reconcile => "reconcile",
            Self::Commit => "commit",
            Self::Decision => "decision",
            Self::Step => "step",
        }
    }

    #[must_use]
    pub const fn color(self) -> &'static str {
        match self {
            Self::Ingest => "#4fc3f7",
            Self::Validate => "#66bb6a",
            Self::Classify => "#ffa726",
            Self::Review => "#8d6e63",
            Self::Reconcile => "#26c6da",
            Self::Commit => "#42a5f5",
            Self::Decision => "#ef5350",
            Self::Step => "#78909c",
        }
    }
}

impl std::fmt::Display for VisualizationRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A typed invariant vertex. IDs are stable machine identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantNode {
    pub id: String,
    pub label: String,
    pub role: VisualizationRole,
    pub invariant: Option<String>,
}

impl InvariantNode {
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>, role: VisualizationRole) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            role,
            invariant: None,
        }
    }

    #[must_use]
    pub fn with_invariant(mut self, invariant: impl Into<String>) -> Self {
        self.invariant = Some(invariant.into());
        self
    }
}

/// A typed invariant edge. Endpoints MUST reference existing node IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

impl InvariantEdge {
    #[must_use]
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            label: None,
        }
    }

    #[must_use]
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

/// Host-neutral graph for l3dg3rr-style invariant visualization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantGraph {
    pub name: String,
    pub nodes: Vec<InvariantNode>,
    pub edges: Vec<InvariantEdge>,
}

impl InvariantGraph {
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_node(mut self, node: InvariantNode) -> Self {
        self.nodes.push(node);
        self
    }

    #[must_use]
    pub fn with_edge(mut self, edge: InvariantEdge) -> Self {
        self.edges.push(edge);
        self
    }

    /// Verify graph invariants before rendering.
    ///
    /// Enforced invariants:
    /// - graph name is non-empty
    /// - node IDs are non-empty and unique
    /// - every edge endpoint references a known node
    /// - self-edges are rejected unless labelled, so docs expose the reason
    pub fn validate(&self) -> Result<(), GraphValidationError> {
        if self.name.trim().is_empty() {
            return Err(GraphValidationError::EmptyGraphName);
        }

        let mut ids = HashSet::new();
        for node in &self.nodes {
            if node.id.trim().is_empty() {
                return Err(GraphValidationError::EmptyNodeId);
            }
            if !ids.insert(node.id.as_str()) {
                return Err(GraphValidationError::DuplicateNodeId(node.id.clone()));
            }
        }

        for edge in &self.edges {
            if !ids.contains(edge.from.as_str()) {
                return Err(GraphValidationError::MissingEdgeEndpoint(edge.from.clone()));
            }
            if !ids.contains(edge.to.as_str()) {
                return Err(GraphValidationError::MissingEdgeEndpoint(edge.to.clone()));
            }
            if edge.from == edge.to
                && edge
                    .label
                    .as_deref()
                    .is_none_or(|label| label.trim().is_empty())
            {
                return Err(GraphValidationError::UnlabelledSelfEdge(edge.from.clone()));
            }
        }

        Ok(())
    }

    /// Render a Mermaid flowchart. Call [`Self::validate`] first for strict CI.
    #[must_use]
    pub fn to_mermaid(&self) -> String {
        let mut out = String::from("flowchart LR\n");
        for node in &self.nodes {
            out.push_str(&format!(
                "  {}[\"{}\"]:::{}\n",
                mermaid_id(&node.id),
                escape_mermaid(&node.label),
                node.role
            ));
        }
        for edge in &self.edges {
            let label = edge
                .label
                .as_ref()
                .map_or(String::new(), |label| format!("|{}|", escape_mermaid(label)));
            out.push_str(&format!(
                "  {} -->{} {}\n",
                mermaid_id(&edge.from),
                label,
                mermaid_id(&edge.to)
            ));
        }
        for role in [
            VisualizationRole::Ingest,
            VisualizationRole::Validate,
            VisualizationRole::Classify,
            VisualizationRole::Review,
            VisualizationRole::Reconcile,
            VisualizationRole::Commit,
            VisualizationRole::Decision,
            VisualizationRole::Step,
        ] {
            out.push_str(&format!(
                "  classDef {} fill:{},stroke:#263238,color:#111;\n",
                role,
                role.color()
            ));
        }
        out
    }

    /// Render a standalone SVG with deterministic horizontal layout.
    #[must_use]
    pub fn to_svg(&self) -> String {
        let width = (self.nodes.len().max(1) as i32 * 180 + 80).max(640);
        let height = 220;
        let mut out = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}" role="img" aria-label="{} invariant graph">"##,
            escape_xml(&self.name)
        );
        out.push_str(r##"<rect width="100%" height="100%" fill="#f8fafc"/>"##);
        out.push_str(r##"<defs><marker id="arrow" viewBox="0 0 10 10" refX="10" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M 0 0 L 10 5 L 0 10 Z" fill="#455a64"/></marker></defs>"##);

        // Build a single O(N) lookup table so edge rendering is O(E) not O(E·N).
        let node_index_map: HashMap<&str, usize> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();

        for edge in &self.edges {
            // Fallback to index 0 is unreachable for validated graphs (validate()
            // guarantees all edge endpoints reference known nodes).
            let from = node_index_map.get(edge.from.as_str()).copied().unwrap_or(0);
            let to = node_index_map.get(edge.to.as_str()).copied().unwrap_or(0);
            let x1 = node_x(from) + 140;
            let x2 = node_x(to);
            let y = 110;
            out.push_str(&format!(
                r##"<path d="M {x1} {y} L {x2} {y}" stroke="#455a64" stroke-width="2" fill="none" marker-end="url(#arrow)"/>"##
            ));
            if let Some(label) = &edge.label {
                let mid = (x1 + x2) / 2;
                out.push_str(&format!(
                    r##"<text x="{mid}" y="96" text-anchor="middle" font-family="monospace" font-size="11" fill="#37474f">{}</text>"##,
                    escape_xml(label)
                ));
            }
        }

        for (index, node) in self.nodes.iter().enumerate() {
            let x = node_x(index);
            let y = 70;
            out.push_str(&format!(
                r##"<g><rect x="{x}" y="{y}" width="140" height="80" rx="10" fill="{}" stroke="#263238" stroke-width="1.5"/>"##,
                node.role.color()
            ));
            out.push_str(&format!(
                r##"<text x="{}" y="105" text-anchor="middle" font-family="monospace" font-size="14" font-weight="700" fill="#111">{}</text>"##,
                x + 70,
                escape_xml(&node.label)
            ));
            out.push_str(&format!(
                r##"<text x="{}" y="126" text-anchor="middle" font-family="monospace" font-size="11" fill="#263238">{}</text></g>"##,
                x + 70,
                node.role
            ));
        }

        out.push_str("</svg>");
        out
    }

}

/// A host system can implement this trait to provide visualization without
/// depending on l3dg3rr domain crates.
pub trait L3dg3rrVisualizable {
    fn invariant_graph(&self) -> InvariantGraph;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphValidationError {
    EmptyGraphName,
    EmptyNodeId,
    DuplicateNodeId(String),
    MissingEdgeEndpoint(String),
    UnlabelledSelfEdge(String),
}

impl std::fmt::Display for GraphValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyGraphName => f.write_str("graph name is empty"),
            Self::EmptyNodeId => f.write_str("node id is empty"),
            Self::DuplicateNodeId(id) => write!(f, "duplicate node id: {id}"),
            Self::MissingEdgeEndpoint(id) => write!(f, "edge endpoint does not exist: {id}"),
            Self::UnlabelledSelfEdge(id) => write!(f, "unlabelled self-edge: {id}"),
        }
    }
}

impl std::error::Error for GraphValidationError {}

fn node_x(index: usize) -> i32 {
    40 + i32::try_from(index).unwrap_or(0) * 180
}

fn mermaid_id(id: &str) -> String {
    let mut value = String::from("n_");
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            value.push(ch);
        } else {
            value.push('_');
        }
    }
    value
}

fn escape_mermaid(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
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
    fn valid_graph_renders_mermaid_and_svg() {
        let graph = InvariantGraph::new("b00t")
            .with_node(InvariantNode::new("datum", "Datum", VisualizationRole::Ingest))
            .with_node(InvariantNode::new(
                "verify",
                "Verify",
                VisualizationRole::Validate,
            ))
            .with_edge(InvariantEdge::new("datum", "verify").with_label("checked"));

        assert_eq!(graph.validate(), Ok(()));
        assert!(graph.to_mermaid().contains("flowchart LR"));
        assert!(graph.to_mermaid().contains("n_datum -->|checked| n_verify"));
        assert!(graph.to_svg().contains("b00t invariant graph"));
    }

    #[test]
    fn duplicate_nodes_are_rejected() {
        let graph = InvariantGraph::new("bad")
            .with_node(InvariantNode::new("same", "A", VisualizationRole::Step))
            .with_node(InvariantNode::new("same", "B", VisualizationRole::Step));

        assert_eq!(
            graph.validate(),
            Err(GraphValidationError::DuplicateNodeId("same".into()))
        );
    }

    #[test]
    fn missing_edge_endpoint_is_rejected() {
        let graph = InvariantGraph::new("bad")
            .with_node(InvariantNode::new("a", "A", VisualizationRole::Step))
            .with_edge(InvariantEdge::new("a", "missing"));

        assert_eq!(
            graph.validate(),
            Err(GraphValidationError::MissingEdgeEndpoint("missing".into()))
        );
    }

    #[test]
    fn labelled_self_edge_is_allowed_for_explicit_invariants() {
        let graph = InvariantGraph::new("loop")
            .with_node(InvariantNode::new("a", "A", VisualizationRole::Decision))
            .with_edge(InvariantEdge::new("a", "a").with_label("retry budget"));

        assert_eq!(graph.validate(), Ok(()));
    }
}
