---
process control system: Ledgrrr is deterministic governance + audit. Use TOML → Rhai FSM + Mermaid + Rust enum. Subprocess via MCP, not library. Code is source of truth for types.

---
cost attribution: FOCUS v1.3 (FinOps) tracks compute spend. MCP tools: append_focus_record, compute_focus_delta. Custom x_ columns for variants (x_ExperimentId, x_ExperimentScore). Build cost attribution into workflows.

---
introspection: ontology-extractor parses Rust AST via syn. Generates OntologyGraph JSON of code structure. Extend to capture function signatures, generate TOML bindings automatically from code (avoid manual config).

---
rhai runtime: Engine::new() compiles .rhai scripts. call_fn(&scope, &ast, name, args) invokes functions. Register custom functions pre-compilation. Scope manages variable context. Isolate via MCP tool interface.

---
visualization: mdbook-rhai-mermaid bridges Rhai↔Mermaid. NodeVisualState (Idle/Active/Success/Warning/Error/Review) with animations (pulse/check/shake/blink/bounce). Annotate transitions with function type signatures.
