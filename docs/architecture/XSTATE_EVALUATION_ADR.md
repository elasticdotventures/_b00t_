# ADR: XState patterns for b00t state machines — actor model, visual editor, model-based testing

**Status**: Accepted (evaluation only — no code change; recommendation is "adopt patterns, not the library")
**Related**: _b00t_ issue #773 (this ADR's origin), #774 (canonical state-machine
visualization — overlaps significantly, see below), `b00t-c0re-lib/src/ooda.rs`,
`b00t-c0re-lib/src/pipeline_nodes.rs`, `b00t-c0re-lib/src/state_introspection.rs`

## Context

Issue #773 asks b00t to evaluate XState (stately.ai) — a JS/TS actor-model
statechart library — against b00t's current Rust-native state machine
approach, across three axes: actor model, visual editor, and model-based
testing. b00t has two live state-machine implementations today:

1. **OODA loop** (`b00t-c0re-lib/src/ooda.rs`) — `statig` 0.4.1, a
   proc-macro-driven hierarchical state machine (HSM) crate. States
   (`Idle → Observing → Orienting → Deciding → Acting → Reviewing →
   Complete/Failed`) are `#[state]`-annotated methods on `OodaCtx`, dispatched
   via a private `OodaDispatch` enum. A parallel `OodaPhase` enum plus
   `can_transition_to()` provides a *second*, hand-rolled validation layer
   that mirrors the statig machine — `OodaLoop::transition_to()` checks
   `OodaPhase::can_transition_to` first, then dispatches into the statig
   `sm.handle(&dispatch)` for the "real" HSM transition. The two are kept in
   sync by hand; nothing enforces they can't drift.
2. **Pipeline nodes** (`b00t-c0re-lib/src/pipeline_nodes.rs`) — a
   `PipelineNode` trait ("PolyConnector") with compile-time-checked
   `Compose<A, B>` composition (`A::Output = B::Input` enforced by the type
   system), where each node also carries a *data-only* `StateMachine` struct
   (`StateMachine::idle_run_cycle()`) used purely for visualization/FOL
   annotation — it does not drive `execute()`, which always just calls the
   node's function directly. This is not a runtime state machine; it is a
   declarative shape attached to a node for diagram/JSON export.
3. **State introspection** (`b00t-c0re-lib/src/state_introspection.rs`) — a
   `StateMachineIntrospection` trait plus `impl_state_machine_introspection!`
   macro that, given a state enum and `can_transition`/`event_for_target`
   functions, auto-derives `render_mermaid_state_diagram()` and a custom
   compact `render_s5()` text format. `OodaPhase` already uses this
   (`ooda_phase_mermaid_state_diagram()`, `ooda_phase_s5()`), with tests
   (`ooda_phase_renders_mermaid_and_s5_from_introspection`) asserting the
   rendered diagram's transitions match `transition_descriptors()` exactly —
   i.e. the diagram is derived from the same transition table the runtime
   validates against, not hand-drawn separately. Doc comments reference
   SCXML and CLIF as future renderer targets; neither exists yet.

This introspection layer is functionally b00t's answer to two of #773's
three XState axes already, and is the direct subject of issue #774
("canonical state machine visualization — emit diagrams from statig Rust
definitions"), which is still open and unimplemented for the harder case
(deriving diagrams from `#[state_machine]`-macro definitions directly,
rather than from a hand-maintained parallel enum as `OodaPhase` currently
is).

## XState's three capabilities, evaluated against what exists

### 1. Actor model (`send()`/`subscribe()` between machines)

XState's actor model lets independently-running statecharts communicate via
typed messages, each actor owning its own state and mailbox. b00t has no
equivalent today — `OodaLoop` and `PipelineNode` machines are both
single-machine, single-thread, driven by direct method calls
(`transition_to()`, `execute()`), not message-passed. There is no
supervision tree, no mailbox, no cross-machine event bus for state machines
specifically (b00t does have `agent_coordination.rs` and MCP-level
agent-to-agent messaging for *agents*, but that is a different layer, not
wired to `OodaPhase`/`PipelineNode` state transitions).

Rust already has actor-model crates that are a closer match than
reimplementing XState's protocol: `actix`, `ractor`, or `statig`'s own
async mode. Given b00t is single-process CLI/MCP-tool-oriented today (not a
distributed multi-agent runtime with independent long-lived actors), there
is no concrete driving use case for this yet. **Verdict: not applicable
now; if b00t's hive/multi-agent coordination (`agent_coordination.rs`,
`b00t_agent_*` MCP tools) grows a need for typed supervised actors, evaluate
`ractor` at that time — it is a native Rust actor framework with a real
supervision tree, not a JS interop shim.**

### 2. Visual editor (Stately Registry)

XState's visual editor is a hosted, interactive canvas for designing and
debugging JS/TS statecharts, round-tripping to/from the machine definition.
b00t's introspection layer produces static Mermaid `stateDiagram-v2` text
(renders natively in GitHub, VS Code, and this repo's own Artifact tooling)
and a custom compact "S5" text format — both derived, not hand-drawn, and
both covered by tests that assert they match the actual transition table.
This is strictly a **generator**, not an editor: there is no round-trip
(edit the diagram, regenerate Rust), and no interactive canvas.

Given b00t's state machines are Rust-native (statig proc-macros, not a
portable JSON schema), a hosted JS visual editor cannot edit them
directly — the realistic integration is one-way (Rust → diagram), which is
exactly what `state_introspection.rs` already does, and what #774 exists to
extend to the harder case (deriving directly from `#[state_machine]` macro
attributes instead of a parallel hand-maintained enum). Building or adopting
a bidirectional visual editor for two current, low-transition-count state
machines (6 states + terminal, 2-state cycle) is effort disproportionate to
payoff. **Verdict: don't adopt Stately's editor. The gap that matters
(automated diagram generation from the actual `statig` macro definitions,
not a hand-synced shadow enum) is #774's problem, already scoped, still
open.**

### 3. Model-based testing (generate test paths from machine definition)

XState's `@xstate/test`/`@xstate/graph` walks a machine definition to
generate all reachable state paths as test cases. b00t's
`transition_descriptors()` already exposes the full transition table as
structured Rust data (`Vec<StateTransitionDescriptor>`) — everything needed
to *generate* exhaustive path tests already exists as introspectable data;
what's missing is a generator that walks it and emits `#[test]` functions
or property-based assertions (e.g. via `proptest`), instead of the current
approach of hand-written transition tests
(`ooda_phase_transitions_valid`, `any_phase_can_fail`, etc. in `ooda.rs`).
This is a small, concrete, Rust-native gap — not a reason to bring in
XState. **Verdict: adopt the pattern, not the library. A follow-up (not
scoped by this ADR) could add a `state_introspection.rs` helper that walks
`transition_descriptors()` and asserts every non-terminal state has at
least one outgoing transition and every state is reachable from
`initial_state()` — closing the actual gap with ~50 lines of Rust, no JS
runtime, no schema translation layer.**

## Recommendation

**Don't adopt XState.** None of its three headline capabilities clears the
bar for a JS/TS dependency in a Rust-native, single-process tool:

- **Actor model**: no current use case; `ractor` is the right Rust-native
  fallback if/when hive coordination needs it.
- **Visual editor**: b00t's existing Mermaid/S5 generator
  (`state_introspection.rs`) already covers the "see the machine" need
  one-way from Rust; closing the remaining gap (macro-derived diagrams) is
  issue #774's job, not a new dependency.
- **Model-based testing**: the underlying data (`transition_descriptors()`)
  already exists; only a small generator is missing, doable in Rust with no
  interop cost.

**Do not create `_b00t_/xstate.repo.toml`** — the literal deliverable
originally suggested in #773's body — since that would register XState as
a tracked external reference for a library this evaluation is declining to
adopt; a `references` entry belongs on #774's eventual datum instead
(mermaid/state-visualization), where it is relevant as a *prior-art* link,
not an adoption target.

**Concrete follow-ups this ADR surfaces (not executed here, out of scope):**
1. #774 remains the right vehicle for macro-derived (not hand-synced)
   diagram generation from `#[state_machine]` definitions.
2. `OodaPhase::can_transition_to()` and the statig `OodaCtx` machine encode
   the same transition table twice by hand — a small refactor to derive one
   from the other (or from `transition_descriptors()`) would remove a
   drift risk, independent of XState.
3. A `proptest`/exhaustive-path generator over `transition_descriptors()`
   would deliver XState's model-based-testing value natively, cheaply, and
   is a reasonable small follow-up task if test coverage gaps are found in
   OODA or pipeline-node transitions.
