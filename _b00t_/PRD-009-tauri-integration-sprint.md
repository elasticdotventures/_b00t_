# PRD-009: Tauri Desktop Integration Sprint

**Status:** Draft
**Date:** 2026-05-15
**Priority:** P1
**Risk:** High (cross-platform Tauri + specta compatibility)

## 1. Problem Statement

The ledgrrr visualization ecosystem currently has two disconnected layers:

| Layer | Location | State |
|-------|----------|-------|
| **Python simulator** | `~/.b00t/vendor/ledgrrr/scripts/ledgrrr-viz-serve.py` | Works, Flask+waitress, 3 pipelines, auth, JSONL log |
| **HTML dashboard** | `crates/holon-viz/target/live-dashboard.html` | Works, Cytoscape.js dagre layout, sidebar process selector |
| **Tauri host** | `crates/ledgerr-host/src/bin/tauri/` | Windows-only, build FAILS (specta nightly feature), no process sim |
| **Rust type graph** | `crates/holon-viz/src/gen.rs` | 21 typed nodes, 14 relationship kinds, Rhai DSL |

The gap: the Tauri host has a proper desktop shell (tray, notifications, credential manager, Cytoscape VZ panel) but it's Windows-only and doesn't compile. The Python simulator has the process model but no desktop integration. The isometric SVG view specified in PRD-3/PRD-9 exists only as documentation — no implementation.

## 2. Objectives

1. **Fix Tauri build** — specta 2.0.0-rc.25 `debug_closure_helpers` → upgrade or pin nightly
2. **Cross-compile for Linux** — remove `#[cfg(target_os = "windows")]` guard, fix platform-specific code
3. **Wire process simulation** — port the Python `LiveProcess` to Rust as a Tauri-managed state machine, OR bridge to the existing Python server via HTTP IPC
4. **Add isometric SVG view** — implement the PRD-3 spec: pure SVG (no WebGL), CSS transition animations, role-keyed node icons
5. **Unified dashboard** — combine the Python type-graph API with the Tauri desktop shell (notifications, tray, credential store)
6. **Test coverage** — integration tests for the Tauri IPC layer

## 3. Tauri Host Survey Summary

From `crates/ledgerr-host/`:

| Aspect | Detail |
|--------|--------|
| Entry point | `src/bin/tauri/main.rs` → binary `host-tauri` |
| App identifier | `ventures.elastic.ledgrrr` |
| Window | 1400×900, min 1100×760, centered, resizable |
| UI | Vanilla JS + esbuild + TypeScript + Cytoscape.js |
| IPC commands | 15 (specta-typed, tauri-specta wired, TS bindings auto-exported) |
| VZ panel | Cytoscape.js dagre layout, "Type Graph" ↔ "Pipeline" toggle |
| Type graph source | `gen.rs` → `TypeRelationshipGraph::seed()` |
| Build failure | specta 2.0.0-rc.25 uses `feature(debug_closure_helpers)` — needs nightly or specta upgrade |
| Process sim | None — Tauri is a viewer; simulation is in `ledgerr-mcp` HSM |
| Platform guard | `#[cfg(target_os = "windows")]` on main — exits cleanly on non-Windows |

specta commands (relevant to viz):
- `get_type_graph` → `CytoscapeGraph` — the full type relationship graph
- `get_holon_viz_graph` → `CytoscapeGraph` — 11-node pipeline holon hierarchy
- `get_evidence_dashboard` → `EvidenceDashboardPayload` — evidence queue status

## 4. Required Work

### Sprint-1: Fix Build + Linux Port (2-3 days)

1. **Specta bump or nightly** — specta 2.0.0-rc.25 → specta 2.0.0 final (check crates.io). If no stable release, switch `rust-toolchain.toml` to `nightly-2026-05-01` for host crate only.
2. **Remove platform guard** — delete or conditionalize the `#[cfg(target_os = "windows")]` in `main.rs`. Handle:
   - `tray-icon` — use `tauri-plugin-tray` or `libappindicator` on Linux
   - `windows-sys` — gate behind `#[cfg(windows)]` imports
   - Notification backend — `notify-rust` for Linux
3. **Vendor system dependencies** — add `tauri.conf.json` Linux bundle targets (deb, AppImage)
4. **Build CI** — `cargo check -p ledgerr-host` in the justfile

### Sprint-2: Process Simulation Bridge (3-4 days)

Option A (recommended): **Rust-native LiveProcess** — port 200 lines of Python to Rust using the existing `CytoscapeGraph` types:
```rust
// In crates/ledgerr-host/src/process.rs
pub struct LiveProcess {
    catalog: HashMap<String, PipelineCatalog>,
    active: String,
    log: Vec<AuthorizationReceipt>,
    lock: Mutex<()>,
}
impl LiveProcess {
    pub fn load_process(&mut self, key: &str) -> ProcessState;
    pub fn tick(&mut self, step_id: &str, by: &str) -> TickResult;
    pub fn state(&self) -> ProcessState;
    pub fn graph(&self) -> CytoscapeGraph;
}
```
Add 2 new Tauri commands: `load_process` and `tick_process`. Wire into VZ panel.

Option B: **HTTP bridge** — Tauri spawns the Python server as a child process and proxies `/api/*` calls via Tauri commands. Simpler but introduces a process dependency.

### Sprint-3: Isometric SVG View (3-4 days)

From PRD-3 acceptance criteria:
> **AC-10.3** — The isometric-3d view must use pure SVG (no WebGL). Inserting or deleting a node must animate position changes via CSS transitions (not snap). Minimum 60 fps on a 20-node graph.
> **AC-10.4** — The isometric SVG canvas must implement pan (click-drag) and zoom (scroll wheel). Pan/zoom must use CSS `transform: matrix()` — not SVG viewBox — for GPU-accelerated composition.
> **AC-10.5** — Nodes rendered as `<g>` groups with `<use>` references to a centralized `<defs>` icon library. Icons must be inline SVG (no external fetches).
> **AC-10.8** — The isometric-3d node icon library must use role keys inferred from node labels: `ingest`, `validate`, `classify`, `review`, `reconcile`, `commit`, `decision`. When no role matches, an auto-generated glTF data URI is used as fallback.

Implementation:
1. New panel "Isometric" (or toggle in VZ panel)
2. SVG coordinate transform: (x, y) → isometric projection: `isoX = (x - y) * cos(30°)`, `isoY = (x + y) * sin(30°)`
3. Node library from `crates/holon-viz/src/icons.rs` or inline SVG defs
4. z-index sorting by back-to-front depth (higher Y = in front)
5. CSS transitions on node position changes

### Sprint-4: Unified Dashboard + Tests (3-4 days)

1. **Catalog sidebar** — port the Python dashboard's process catalog UI (3 pipelines, step selector, Rhai DSL display)
2. **Auth bridge** — if using Option B (HTTP bridge), pipe the Bearer token from Tauri's credential store
3. **Audit log panel** — display `AuthorizationReceipt` entries with filter/search
4. **Integration tests** — `crates/ledgerr-host/tests/`:
   - `specta_types.rs` — verify all 15+ commands serialize/deserialize correctly
   - `live_process.rs` — test `load_process` + `tick` + `state` + `graph` as unit tests
   - `tauri_e2e.rs` — Tauri test harness (`tauri::test::mock_app`) invoking IPC commands
   - `isometric_render_test.js` — headless Cypress/Playwright test for SVG view

## 5. Sub-agent Dispatch

```
PRD-009
├── Sprint-1: Build + Linux port
│   ├── SA-A: Fix specta build (bump to 2.0.0 final or nightly toolchain)
│   └── SA-B: Remove #[cfg(windows)], port tray/notify to Linux
├── Sprint-2: Process simulation bridge
│   ├── SA-C: Rust LiveProcess (port from Python 200 lines)
│   └── SA-D: Add load_process + tick_process Tauri commands
├── Sprint-3: Isometric SVG view
│   ├── SA-E: SVG coordinate transform + defs icon library
│   └── SA-F: CSS transition animations + pan/zoom
├── Sprint-4: Unified dashboard + tests
│   ├── SA-G: Port catalog/step-detail/audit panels to Tauri UI
│   ├── SA-H: specta integration tests + LiveProcess unit tests
│   └── SA-I: Headless viz render tests (Cypress/Playwright)
└── Verification agent: full Tauri binary smoke test + all tests pass on Linux
```

## 6. Success Criteria

- `cargo check -p ledgerr-host` succeeds on Linux
- `host-tauri` binary launches and displays window with VZ panel
- Process catalog sidebar shows 3 pipelines (pipeline, governance, arc_kit_au)
- Clicking a pipeline loads its steps and renders graph
- "Next" button advances through pipeline, graph nodes highlight
- Isometric SVG view renders 20+ nodes at 60fps
- All tests pass: `cargo test -p ledgerr-host` + `cargo test -p holon-viz`
- Python simulator continues to work independently (no regression)
- Auth works: remote requests to Python server still require Bearer token
