use b00t_l3dg3rr_viz::{
    InvariantEdge, InvariantGraph, InvariantNode, L3dg3rrVisualizable, VisualizationRole,
};

struct B00tDatumFlow;

impl L3dg3rrVisualizable for B00tDatumFlow {
    fn invariant_graph(&self) -> InvariantGraph {
        InvariantGraph::new("b00t datum flow")
            .with_node(InvariantNode::new("load", "Load", VisualizationRole::Ingest))
            .with_node(InvariantNode::new(
                "classify",
                "Classify",
                VisualizationRole::Classify,
            ))
            .with_node(InvariantNode::new(
                "visualize",
                "Visualize",
                VisualizationRole::Commit,
            ))
            .with_edge(InvariantEdge::new("load", "classify"))
            .with_edge(InvariantEdge::new("classify", "visualize").with_label("valid"))
    }
}

#[test]
fn implementors_get_visualization_after_invariant_validation() {
    let graph = B00tDatumFlow.invariant_graph();

    graph.validate().expect("sample graph satisfies l3dg3rr invariants");

    let mermaid = graph.to_mermaid();
    let svg = graph.to_svg();

    assert!(mermaid.contains("n_classify -->|valid| n_visualize"));
    assert!(svg.contains("Visualize"));
}
