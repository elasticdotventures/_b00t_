# l3dg3rr Visualization Utility

`PromptExecution/l3dg3rr` exposes reusable skills around typed financial workflows, invariant checks, and visual audit graphs. b00t packages the host-neutral part as `b00t-l3dg3rr-viz`.

Upstream reference inspected: `PromptExecution/l3dg3rr@1dd4a1c`.

## Contract

Any system gets visualization when it can produce an `InvariantGraph` and pass validation:

- The graph name MUST be non-empty.
- Node IDs MUST be non-empty and unique.
- Edge endpoints MUST reference existing node IDs.
- Self-edges MUST carry a label that explains the invariant loop.

Systems can either build `InvariantGraph` directly or implement `L3dg3rrVisualizable`.

## Outputs

- `to_mermaid()` emits a Mermaid `flowchart LR` for mdBook, GitHub Markdown, and issue/PR summaries.
- `to_svg()` emits standalone SVG for CI artifacts and static documentation.
- Rustdoc is published by `.github/workflows/l3dg3rr-docs.yml` as an artifact on PRs and to GitHub Pages on `main`.

## Mission Fit

The utility separates l3dg3rr's abstract invariant graph from its financial domain. b00t can expose datum, hive, ACP, and task capability maps with the same visual contract that l3dg3rr uses for audit workflows. A remote or local node only needs to emit the generic graph shape to get comparable docs and diagrams.

<!-- b00t:map v1
summary: Host-neutral l3dg3rr invariant graph visualization utility packaged for b00t CI docs
tags: l3dg3rr, visualization, docs, ci, invariants, rust
tier: ch0nky
cmds: just l3dg3rr-docs-check, cargo test -p b00t-l3dg3rr-viz
complexity: 5
-->
