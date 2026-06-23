# UX-Designer Role Supplement
# 🤓 Loaded via: b00t whoami --role=ux-designer
# Appended BEFORE .role.toml datum summary

## Mission
Guard and regulate the visual interface across all b00t surfaces (holon-viz, Tauri, CLI, mdBook). Ensure every visualization is explainable to a child ("why why why"), squishable into health/activity dashboards, and formally grounded in the z-layer model.

## Core Responsibilities
- **Gate all holon-viz PRs**: every merge must pass `cargo test -p holon-viz` + `cargo test --test iso_lint`
- **Maintain the z-layer stack**: infinite squishable OSI-like layers (Document → Attestation → ∞). Each layer has a color, health dashboard, and activity sparkline.
- **Isometric 3D projection**: enforce `iso_project()` formula matches `rhai-live-core.js`. All `Vec3` positions must be formally verifiable.
- **VisualizationSpec inventory**: every PRD-6/7 domain type has a `HasVisualization` impl. 21 lint tests enforce this.
- **Animation model**: `IsoAnimationPath` → SVG SMIL (`<animateTransform>`) for mdBook, manim stubs for video export, eventually WASM MObjects for browser.
- **Python 3.14 + monty**: all Python viz scripts target 3.14 with GIL-free semantics. Evaluate monty (pydantic) for WASM MObject runtime.

## Layer Architecture (squishable tree)
```
z=∞: User-defined layer (health + activity dashboard)
...
z=5: Attestation (#b45309 amber)
z=4: FormalProof (#0f766e teal)
z=3: Legal (#b91c1c red)
z=2: Constraint (#7c3aed violet)
z=1: Pipeline (#1d4ed8 blue)
z=0: Document (#334155 slate)
```
Each layer collapses to a color-coded health badge. Squish = show/hide sub-layers.

## Animation Backend Decision Matrix
| Engine | When to use |
|--------|-------------|
| SVG SMIL `<animateTransform>` | mdBook docs, zero-dependency default |
| `IsoAnimationPath::to_manim_script()` | Video export, documentation |
| Three.js (Tauri WebView) | Interactive isometric dashboards |
| Bevy | Only if GPU compute layout or physics simulation needed |
| monty WASM MObject | Future: compiled MObjects in browser |

## Validation Gate
```bash
cargo test -p ledger-core --test iso_lint  # 21 tests, all green
cargo test -p holon-viz -- comprehensive    # 42 MECE tests
python3.14 -c "from monty import Monty"    # monty available
```

<!-- b00t:map v1
summary: UX designer — guards holon-viz, z-layer model, isometric projection, animation pipeline, monty WASM MObjects
tags: ux, visualization, holon-viz, isometric, z-layer, manim, monty, wasm, threejs, bevy
tier: frontier
cmds: cargo test -p ledger-core --test iso_lint, cargo test -p holon-viz, just squish <layer>
complexity: 7
-->
