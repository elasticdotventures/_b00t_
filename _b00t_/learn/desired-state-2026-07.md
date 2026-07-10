# Desired State — b00t v0.9.x → v1.0

## Architecture
- **b00t-admin merges with ledgrrr**: ledgrrr becomes a shared library inside b00t-admin. No separate mermaid rendering — ledgrrr's `kasuari` constraint solver + glTF/isometric emitter replaces it.
- **Isometric first**: Cassowary-inspired constraint layouts for knowledge graph visualization. `mdbook-rhai-mermaid` emitter + `kasuari` crate provide the engine. 2:1 dimetric projection, SceneGraph → SVG/glTF.
- **Capability-based dispatch**: `provides` field on `BootDatum` replaces suffix-matching. Intent matches capability, not filename.

## Display System
- **SemanticClass**: 9 classes (Infra/Agent/Protocol/Skill/Tool/Repo/Data/Secret/Unknown), each with shape/color/icon/SVG defaults.
- **css_class**: The hook for subsystem animations. Each type gets a CSS class; subsystems provide CSS animations.
- **No TOML overrides**: Code is the single source of truth for display.

## Datum System
- **Stereotype hierarchy**: DatumType::implies() — Runtime→Cli, Agent→Runtime, McpServer→Mcp, Ai→Agent.
- **Symlink datums**: One canonical file per tool, symlinked into type namespaces (.cli.toml → .toml, .runtime.tomllmd → .toml).
- **Idempotent merge**: When multiple datums match, merge complementary fields; only prompt on conflicts.

## Ecosystem Search
- **blessed crate**: v0.1.0 on crates.io. Queries blessed.rs manifest (92 curated entries).
- **Trust tiers**: 🥾blessed > 📋awesome > 📦local. Ingested awesome-rust (770 entries).
- **Project scanner**: Cross-references Cargo.toml/package.json/go.mod against manifests.

## OODA / Governance
- **Typed workflow contracts**: Each phase has typed input/output. Proposals within phases with exit criteria.
- **Governance**: Auto-selection by confidence. Operator approval for mutations.
- **Ralph loop**: Agent REPL outer-loop for autonomous execution.
- **Ooda as Plan subtype**: Not a Skill. Teal rectangle (Tool class).

## Build / Deploy
- **b00t-buildd**: Background daemon watches git diff, maps files→crates, auto-builds. Auto-restarts b00t-admin.
- **just deploy-check**: test → build → restart → validate JS → check health. Catches merge conflicts + brace bugs before browser.
- **HTML sanity tests**: Compile-time checks for `<<<` merge markers, `{{}}` brace bugs, CDN presence, HTML structure.
- **bump-install**: Self-verifying — polls health API until version matches.

## UI/UX
- **localStorage persistence**: Panel state, viz selection, viewport position, orphan filter survive refresh.
- **Crash detection**: Heartbeat turns red after 3 failures, banner appears with reload link.
- **Cytoscape**: Degree-scaled repulsion (Cassowary-inspired), shift+scroll 10x zoom, orphan filter checkbox.
- **Isometric view**: Python SVG generator, 322-node 3D projection, responsive scaling, hover tooltips.

## OpenCode Integration
- **Plugin auto-install**: hook_pre checks + installs @prevalentware/opencode-goal-plugin before launch.
- **/goal + /b00t**: Goal pursuit with budget tracking, skill registry with session-scoped enable/disable.
- **Runtime dispatch**: `b00t opencode` → runtime (no disambiguation). No sandbox in WSL.

## MCP Health
- **0 parse errors**: All schema issues fixed (gate.check→gate, mcp→mcp.stdio, env→gate).
- **ledgrrr-mcp**: Working in opencode after tool schema fixes (description, type:object, strip $schema).
- **rust-doc**: Built, embeddings cache at ~/.local/share/rustdocs-mcp-server/, nomic-embed-text on k8s.

## Namespace
- **crates.io**: `blessed` v0.1.0, `b00t` v0.1.0 placeholder
- **npm**: `b00t-cli` available, needs NPM_TOKEN
- **PyPI**: `p00ty` as fallback (b00t rejected), needs PYPI_TOKEN

## CAD
- **b00t-cad**: cadrum (OpenCASCADE wrapper), 80ms flange demo, STEP/STL/glTF export. No C++ toolchain.
