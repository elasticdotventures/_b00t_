# Roadmap: v0.11.0 — next epoch

## Theme: Deepen pipeline → harden release → reduce friction

### P0: Pipeline gap-fill (carried from 0.10.x)
After the mega-merge, ~15 unused-import warnings. Quick cleanup:
```bash
# Remove unused imports flagged by compiler
cargo fix --lib -p b00t-cli --allow-dirty
cargo fix --lib -p b00t-mcp --allow-dirty
```

### P1: State machine visualization (#774)
**Why**: The pipeline state machine (#743) is implemented in `pipeline_statemachine.rs` but has no visual output. Existing `pipeline_viz.rs` renders PipelineDag as Mermaid/SVG — extend it to also render StateMachine.

**How**:
- Add `impl VizFormat for StateMachine` — emit stage state as Mermaid state diagram
- `StateMachine::to_mermaid()`: render states as `stateIdle --> Validating` transitions with event labels
- Wire into `b00t viz state --pipeline <name> [--format mermaid|svg]`
- Uses existing `MermaidViz`, `SvgViz` structs in `pipeline_viz.rs`
- File: `b00t-cli/src/pipeline_viz.rs` (+ ~80 lines)

### P2: NATS transport K8s integration (#731 partially)
**Why**: The K8s CRD (#731) defines `CapsuleDefinition` but no operator exists. The NATS transport (#729) routes stage data but isn't wired into the K8s deployment.

**How**:
- `capsule_to_deployment()` emits K8s Deployment with NATS env vars (`NATS_URL`)
- Operator container sidecar for pod-level NATS proxy
- File: `b00t-cli/src/pipeline_k8s.rs` (+ ~60 lines)

### P3: Grok KB indexing for pipeline telemetry
**Why**: Pipeline logs (#735) and cost data (#745) are in-memory only. Grok KB can persist and query them.

**How**:
- `impl LogStore for GrokBackend` — store pipeline log entries as grok datums
- `impl DataFrameStore for GrokBackend` — store stage outputs for cost analysis
- `b00t grok digest -t pipeline-cost --run <id>` — index a run's cost data
- File: `b00t-cli/src/pipeline_grok.rs` (new, ~150 lines)

### P4: Fix `#[ignore]` tests
Re-enable ignored tests after verifying infra:
- `b00t-cli/src/datum_k8s.rs` — k8s sync test (needs cluster)
- `b00t-cli/src/guard.rs` — guard coverage (needs datums with guards)
- Fix guard configs for sccache, z3, helix-db datums (from handoff-2026-07-12.md)

### GitHub issues to track
| Issue | Priority | Effort |
|-------|----------|--------|
| #774 State machine viz | P1 | 1 session |
| Extend K8s CRD with NATS operator | P2 | 1 session |
| Grok KB log backend | P3 | 1 session |
| Unused import cleanup | P0 | 15 min |
| Re-enable ignored tests | P4 | 30 min |

### Key constraint
v0.11.0 is a hardening release — no mega-branches. Each issue gets its own branch, its own PR, merged within 24h. Kaizen over revolution.
