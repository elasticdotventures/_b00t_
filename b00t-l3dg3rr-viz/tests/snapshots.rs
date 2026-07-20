//! Deterministic rendering tests for the isometric engine.
//!
//! Every test verifies that `graph_to_isometric_svg()` is a pure function:
//! same input always produces identical SVG output. No snapshot hashes,
//! no magic values — the contract is determinism itself.

use std::time::Instant;

use b00t_l3dg3rr_viz::isometric::{graph_to_isometric_svg, mermaid_to_isometric_svg};
use b00t_l3dg3rr_viz::tax_lawyer_demo;
use b00t_l3dg3rr_viz::{InvariantEdge, InvariantGraph, InvariantNode, VisualizationRole};

fn hash_of(svg: &str) -> String {
    let h = blake3::hash(svg.as_bytes());
    h.to_hex()[..16].to_string()
}

/// Assert that `runs` consecutive renders of `graph` produce identical SVG.
/// Returns timing in milliseconds.
fn assert_pure(graph: &InvariantGraph, runs: usize) -> u128 {
    let t0 = Instant::now();
    let first = graph_to_isometric_svg(graph).expect("first render");
    let expected = hash_of(&first);
    for i in 1..runs {
        let svg = graph_to_isometric_svg(graph).expect("repeat render");
        let actual = hash_of(&svg);
        assert_eq!(actual, expected, "divergent SVG at run {i}");
    }
    t0.elapsed().as_millis()
}

/// Mermaid text is also deterministic through the public API.
fn assert_mermaid_pure(mermaid: &str, runs: usize) -> u128 {
    let t0 = Instant::now();
    let first = mermaid_to_isometric_svg(mermaid).expect("first render");
    let expected = hash_of(&first);
    for i in 1..runs {
        let svg = mermaid_to_isometric_svg(mermaid).expect("repeat render");
        assert_eq!(hash_of(&svg), expected, "divergent mermaid SVG at run {i}");
    }
    t0.elapsed().as_millis()
}

#[test]
fn tax_lawyer_demo_is_pure() {
    let ms = assert_pure(&tax_lawyer_demo(), 10);
    assert!(ms < 1000, "tax_lawyer_demo: {ms}ms exceeds 1s budget");
}

#[test]
fn chain_3_is_pure() {
    let graph = InvariantGraph::new("chain")
        .with_node(InvariantNode::new("a", "Source", VisualizationRole::Data))
        .with_node(InvariantNode::new("b", "Process", VisualizationRole::Task))
        .with_node(InvariantNode::new("c", "Output", VisualizationRole::Report))
        .with_edge(InvariantEdge::new("a", "b").with_label("transform"))
        .with_edge(InvariantEdge::new("b", "c").with_label("emit"));
    let ms = assert_pure(&graph, 10);
    assert!(ms < 500, "chain_3: {ms}ms exceeds 500ms budget");
}

#[test]
fn mermaid_roundtrip_is_pure() {
    let mermaid = "flowchart LR\n    a[\"Input\"]\n    b[\"Validate\"]\n    c[\"Commit\"]\n    a --> b\n    b --> c";
    let ms = assert_mermaid_pure(mermaid, 10);
    assert!(ms < 500, "mermaid_roundtrip: {ms}ms exceeds 500ms budget");
}

#[test]
fn satisfies_edges_are_pure() {
    let graph = InvariantGraph::new("constraints")
        .with_node(InvariantNode::new("r", "Rule", VisualizationRole::Rule))
        .with_node(InvariantNode::new(
            "t",
            "Transaction",
            VisualizationRole::Data,
        ))
        .with_edge(InvariantEdge::new("r", "t").with_label("satisfies: PASS"))
        .with_node(InvariantNode::new(
            "f",
            "Failed",
            VisualizationRole::Decision,
        ))
        .with_edge(InvariantEdge::new("r", "f").with_label("satisfies|FAIL|s38-190"));
    let ms = assert_pure(&graph, 10);
    assert!(ms < 500, "satisfies_edges: {ms}ms exceeds 500ms budget");
}

#[test]
fn different_graphs_produce_different_svg() {
    let a =
        InvariantGraph::new("a").with_node(InvariantNode::new("x", "X", VisualizationRole::Step));
    let b =
        InvariantGraph::new("b").with_node(InvariantNode::new("y", "Y", VisualizationRole::Ingest));
    assert_ne!(
        hash_of(&graph_to_isometric_svg(&a).unwrap()),
        hash_of(&graph_to_isometric_svg(&b).unwrap()),
        "different graphs must produce different SVG"
    );
}

#[test]
fn empty_and_single_node_render() {
    let empty = InvariantGraph::new("void");
    let svg = graph_to_isometric_svg(&empty).expect("empty render");
    assert!(svg.contains("<svg"));
    assert!(svg.contains("No nodes"));

    let single = InvariantGraph::new("one").with_node(InvariantNode::new(
        "solo",
        "Solo",
        VisualizationRole::Step,
    ));
    let svg = graph_to_isometric_svg(&single).expect("single render");
    assert!(svg.contains("<svg"));
}
