# b00t-l3dg3rr-viz

Generic visualization utilities for systems that implement the l3dg3rr invariant graph contract.

The crate deliberately avoids l3dg3rr domain dependencies. A host system provides typed invariant nodes and edges, validates the resulting graph, then gets deterministic Mermaid and SVG renderers.

```rust
use b00t_l3dg3rr_viz::{InvariantEdge, InvariantGraph, InvariantNode, VisualizationRole};

let graph = InvariantGraph::new("b00t")
    .with_node(InvariantNode::new("datum", "Datum", VisualizationRole::Ingest))
    .with_node(InvariantNode::new("verify", "Verify", VisualizationRole::Validate))
    .with_edge(InvariantEdge::new("datum", "verify"));

graph.validate().expect("graph is internally consistent");
let mermaid = graph.to_mermaid();
let svg = graph.to_svg();
```
