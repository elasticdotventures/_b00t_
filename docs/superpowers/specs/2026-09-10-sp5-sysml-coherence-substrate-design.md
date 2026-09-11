# SP5 — SysML-v2 coherence substrate (design)

Sub-project 5 of the `b00t.promptexecution.com` platform program (parent spec:
`2026-09-10-b00t-platform-sp123-design.md`; siblings: SP4 =
`2026-09-10-sp4-serverless-mcp-hosting-design.md`). SP1–SP4 are merged to `main`
(PRs #1296, #1297).

**SP5 is `elasticdotventures/_b00t_#1177` milestone P4** — "Oxigraph-backed
queryable process graph" — scoped up to the identity/authz graph. P1/P2/P3 of
#1177 are done (`b00t-cli/src/dispatch_sysml.rs` + `ufo-types` `sysml`/`mbse`/
`sysml_model`/`ontology`/`view`); P4 was the one unbuilt stretch goal.

## Scope (locked with operator, 2026-09-10)

SP5 delivers the **substrate only**:

1. **Enable `store-oxigraph`** as a real query path (today `OxigraphSparqlSource`
   is a stub returning `Ok(vec![])`).
2. **Load the datum + identity/authz graph** into the embedded Oxigraph store:
   all `BootDatum` structural triples **plus** the SP1/SP2 tenant → r0le →
   tool-allowlist → soul-shard-grant → budget edges.
3. **SHACL-lite validation** — a fixed set of SPARQL `ASK`/`SELECT` constraint
   shapes run as a CI gate (referential integrity + tenant-key isolation +
   dependency-cycle detection).
4. **KerML view projection** — project a graph query into KerML text
   (`ufo_types::sysml_model` constructs), round-trip-validated through
   `ufo_types::sysml::validate_sysml_v2`. **KerML is the first-class output, not
   Mermaid** — Mermaid breaks on complex diagrams; the render side (SP6) consumes
   KerML/`iso_ir`, never Mermaid, as its primary view type.

**Explicitly NOT in SP5 — SP6 end state:** functional kr0ki *rendering* of a
KerML query view (kr0ki using `holon-viz` internally to draw it). SP5 stops at
emitting the KerML view text + `iso_ir` JSON. kr0ki deploy (DNS/CDN) is also SP6.

## Locked decisions

| # | Decision | Consequence for SP5 |
|---|---|---|
| **D-SP5-1** | **kr0ki must never read datums** (kr0ki PRD-KR0KI-001 §1.1 — separation of concerns; b00t owns the model side, kr0ki owns the render side; the cut is at `iso_ir`). | SP5's graph load, SPARQL, SHACL, and KerML projection **all live in b00t** (`b00t-c0re-lib`). SP5 emits KerML text + `iso_ir` JSON as the hand-off artifact. Nothing in kr0ki changes in SP5. |
| **D-SP5-2** | **Refactor, do not duplicate — corrected 2026-09-11.** `BootDatum`, `datum_utils`, `soul_scope::ShardKind`, `datum_agent_profile::AgentProfileSpec` all live in **`b00t-cli`**; `b00t-c0re-lib` cannot depend on `b00t-cli`. So `compile_datum_triples` **stays in `b00t-cli`** (moving it drags the whole datum layer across — out of scope). | SP5's substrate (`b00t-c0re-lib`, `store-oxigraph`) operates on **plain `Vec<(String,String,String)>` triples** — zero `BootDatum` dependency. The triple *compilers* (`compile_datum_triples` + the new `compile_identity_triples`) stay in `b00t-cli`. A new **`b00t graph emit-triples`** subcommand (default `b00t-cli` build) writes the composed triple set as JSONL; the `b00t-c0re-lib/examples/b00t-graph` bin (built `--no-default-features --features store-oxigraph`) reads that JSONL. Two binaries, no cross-crate feature conflict, nothing moved or duplicated. The generic graph→KerML text emitter is **contributed to `ufo-types`** (`sysml_model::emit_kerml`), consumed by SP5 *and* by `dispatch_sysml.rs` (which drops its private `dispatch_chain_to_sysml_v2` hand-roll). |
| **D-SP5-2b** | **r0les get read/write to both soulscopes AND datums** (operator, 2026-09-11). | `b00t r0le build`'s default `soul_shard_grants` becomes `{Datum,"*",Rw}` + `{Project,"*",Rw}` + `{System,"*",Rw}` + `{Agent,"*",Rw}` + `{Skill,"*",Rw}` + `{Tool,"*",Rw}` (was `{Skill:<s>:r}* + {Agent:<role>:rw}`). SP5-02 emits these faithfully as `grantsShard`/`shardMode` edges; SP5-04 adds a shape that flags any r0le lacking `rw` on a `datum` shard or on a soulscope shard. |
| **D-SP5-3** | **KerML first-class, Mermaid rejected** as the view type. | SP5-05 projects to `ufo_types::sysml_model::{ElementKind, Relation}` → KerML text, round-trip-validated. No Mermaid/D2 emitter in SP5. |
| **D-SP5-4** | **Full grant graph + SHACL-lite gate** (not datum-graph-only). | SP5-02 emits identity/authz edges; SP5-04 gates them. |
| **D-SP5-5** | `store-oxigraph` **stays opt-in** — non-default, mutually exclusive with the default `store-helixdb` (`irontology_bridge.rs` `compile_error!` on both). | All SP5 code + tests compile/run only under `--no-default-features --features store-oxigraph`. Shipped as `b00t-c0re-lib/examples/b00t-graph.rs`, **not** a first-class `b00t` subcommand (that needs the backend features to become additive — a separate task). Default b00t is unchanged. |
| **D-SP5-6** | No local `cargo build`/`test` — CI is the verification gate. | A dedicated `graph-oxigraph` CI job; pushes use `--no-verify`. |

## Reusable substrate (call these, don't rebuild)

- **`b00t-c0re-lib/src/irontology_bridge.rs::OxigraphStore`** — embedded SPARQL
  1.1 RDF store at `~/.b00t/oxigraph/<namespace>/`. Real, working: `Store::open`,
  `fact_to_quad`, `upsert_facts`, `quads_for_pattern`. `#[cfg(feature =
  "store-oxigraph")]`.
- **`b00t-cli/src/datum_triples.rs::compile_datum_triples(b00t_path) ->
  Vec<(String,String,String)>`** — already emits `b00t:` namespace SPO triples
  from every datum: `b00t:datum/<key>`, `b00t:dependsOn`, `b00t:requires`,
  `b00t:hasPart`, `b00t:hasKeyword`, `b00t:hasSkill`, `b00t:hasType`,
  `rdfs:label`. **Stays in `b00t-cli`; SP5-06 adds a `b00t graph emit-triples` bridge.**
- **`b00t-c0re-lib/src/query_bus.rs`** — `QueryContext.triples:
  Vec<(String,String,String)>` is the slot ("empty if graph not loaded yet");
  `OxigraphSparqlSource { query_template, depth }` is the stub to finish
  (SP5-03); `QueryBus::fanout` runs all `QuerySource`s concurrently.
- **`b00t-cli/src/dispatch_sysml.rs`** — #1177 P1/P2: `dispatch_chain_iso_ir() ->
  (Vec<Node>, Vec<Edge>)`, round-trip-validated SysML v2. The pattern SP5-05
  follows; its private `dispatch_chain_to_sysml_v2` is replaced by the shared
  `ufo-types` emitter.
- **`ufo-types` (v0.14.0)** — `iso_ir::{Node, Edge}`; `sysml::validate_sysml_v2`
  (wraps `sysml-v2-parser`, grammar-checked); `mbse::MbseExport` (per-`Stereotyped`
  value → SysML v2 `part`); `sysml_model::{ElementKind, Relation, ElementId}`
  (KerML abstract syntax, pure data, `#[non_exhaustive]`); `ontology::{UfoRelation,
  OntologicalEdge}`; `stereotype::UfoStereotype`; `view::SysmlViewKind`.
- **SP2 `AgentProfileSpec`** — `{ tool_allowlist: Vec<String>, skills, soul_shard_grants:
  Vec<SoulShardGrant{kind: ShardKind, id, mode}>, model_tier, budget_ceiling, permissions,
  signature }`, resolved tenant-namespaced via
  `datum_utils::get_all_datums_for_tenant(b00t_path, Option<&str>, depth)`
  (overlay `tenants/<t>/_b00t_/**`).
- **SP4 `[b00t.mcp_server]` / `McpServerSpec`** — `image`, `budget_ceiling`, etc.
  on the same `BootDatum`.

## Upstream — kr0ki blockers (resolved as part of this work, 2026-09-10)

| # | Where | Resolution |
|---|---|---|
| D1 | ledgrrr#202 | **Closed.** `sysml-derive` stays Part/containment-agnostic; `UfoStereotype`→SysML metadata emission lives in `ufo-types` (per D3); systhread/kr0ki compose the two. `sysml-derive` extraction to a standalone crate approved, non-blocking. |
| D2 | ledgrrr#203 | **Closed.** Real (non-dev) dependency on `holon-viz` accepted, precondition: semver discipline on `cytoscape.rs` public types; pin by `rev` until a tagged `0.x`. Struct-shape copying rejected (PRD §2.2). |
| D3, D6 | npl#53 f/u, kr0ki | Already resolved (typed layer → `ufo-types`; `VOCABULARY.md`). |
| D4 (DNS), D5 (CDN) | infrastructure | **Open — SP6 scope.** kr0ki service deploy is not SP5. |

ufo-types#3 / ledgrrr#201 (the stale-`rev` consolidation) already merged.

## Sub-tasks

| id | title | key files | contract (essentials) | dep | size |
|---|---|---|---|---|---|
| **SP5-06** | `b00t graph emit-triples` subcommand (bridge — replaces the "move into b00t-c0re-lib" idea, see D-SP5-2) | `b00t-cli/src/commands/graph.rs` (new), `commands/mod.rs`, `main.rs` | `b00t graph emit-triples [--tenant T] [--out PATH]` composes `crate::datum_triples::compile_datum_triples(path)` + `crate::identity_triples::compile_identity_triples(path, tenant)`, dedups, writes one JSON `[s,p,o]` array per line to `--out` (default stdout). `compile_datum_triples` is **not moved**. | SP5-02 | S |
| **SP5-01** | Graph load path | `b00t-c0re-lib/src/graph_load.rs` (new), `lib.rs` | `pub fn load_graph(store: &OxigraphStore, triples: &[(String, String, String)]) -> Result<usize>` — expand `b00t:` → `http://b00t.promptexecution.com/ontology#`, `rdfs:` → the RDFS namespace; object is an IRI if it matches `\w+:\S+` and a known prefix, else a plain literal; insert as quads via `Store::insert`; return quad count. Idempotent (store dedups). | SP5-06 | M |
| **SP5-02** | Identity/authz edge compiler | `b00t-c0re-lib/src/identity_triples.rs` (new), `lib.rs` | `pub fn compile_identity_triples(b00t_path: &str, tenant: Option<&str>) -> Result<Vec<(String, String, String)>>`. Over `get_all_datums_for_tenant` datums with `datum_type == AgentProfile`: `b00t:tenant/<t> b00t:hasR0le b00t:r0le/<t>/<role>`, `b00t:r0le/<t>/<role> b00t:allowsTool "<glob>"`, `… b00t:grantsShard b00t:shard/<kind>/<id>`, `b00t:shard/<kind>/<id> b00t:shardMode "<r|rw>"`, `… b00t:budgetCeiling <n>`, `… b00t:modelTier "<tier>"`. Over `McpServer` datums: `b00t:svc/<key> b00t:budgetCeiling <n>`, `b00t:svc/<key> b00t:image "<ref>"`. `<role>` = datum key minus `.agentprofile` suffix; `<t>` = the tenant overlay the datum resolved from (`_base` when unshadowed). | SP2, SP4 datum shapes | M |
| **SP5-03** | Finish `OxigraphSparqlSource::query` | `b00t-c0re-lib/src/query_bus.rs` | Add `store: Arc<OxigraphStore>` (or lazy `OnceCell` from `$B00T_OXIGRAPH_NS`) + `from_env()`. `query()`: derive the subject IRI as `b00t:datum/<slug(ctx.text)>` (the topic under lookup), substitute it into `self.query_template` (a property-path SPARQL `SELECT ?adj ?label` over `(b00t:dependsOn|b00t:hasPart|b00t:relatedTo)+` bounded by `self.depth`), run `store.query()`, map each solution → `QueryResult { key: local-name(?adj), summary: ?label or "", source: "oxigraph:sparql", trust: TrustGrade::DatumCompiled, score: depth-decay, match_reason: Some("graph path from <subject>") }`. Register in the `QueryBus` default set behind `#[cfg(feature = "store-oxigraph")]`. Empty store → `Ok(vec![])` (unchanged graceful behaviour). | SP5-01 | M |
| **SP5-04** | SHACL-lite shape suite | `b00t-c0re-lib/src/graph_shapes.rs` (new), `lib.rs` | `pub struct ShapeViolation { shape: &'static str, focus_node: String, message: String, severity: Severity }` (`Severity::{Violation, Warning}`); `pub fn validate_graph(store: &OxigraphStore) -> Result<Vec<ShapeViolation>>` runs a fixed `&[(name, sparql, severity)]`: **`dependsOn-acyclic`** — `ASK { ?x b00t:dependsOn+ ?x }` must be `false` (Violation); **`every-svc-has-a-budget`** — every `?s` with `b00t:image` must have `b00t:budgetCeiling` (Violation); **`every-r0le-has-a-tenant`** — every `b00t:r0le/*` must be the object of some `b00t:hasR0le` (Violation); **`grantsShard-kind-is-declared`** — every `b00t:shard/<kind>/*` `<kind>` ∈ `ShardKind` variants (Violation); **`no-cross-tenant-datum-key-collision`** — no datum key appears under two distinct `b00t:tenant/*` overlays with divergent triples (Warning). | SP5-01 | M |
| **SP5-05** | Graph → KerML view projection (+ contribute the emitter to `ufo-types`) | `ufo-types/src/sysml_model.rs` (new `emit_kerml`), `b00t-c0re-lib/src/graph_kerml.rs` (new), `b00t-cli/src/dispatch_sysml.rs` (drop private emitter) | **ufo-types PR:** `pub fn emit_kerml(package: &str, elements: &[(ElementId, ElementKind)], relations: &[Relation]) -> String` — deterministic KerML text (`package … { part def …; … }`), no wall-clock/UUID, round-trip-checked in-crate against `validate_sysml_v2` (feature `sysml`). **b00t side:** `pub enum GraphView { DatumDeps, R0leGrants, ServiceBudgets }`; `pub fn graph_to_kerml(store: &OxigraphStore, view: GraphView) -> Result<String>` — SPARQL the store for the view's subgraph, map each node → `(ElementId, ElementKind::PartDefinition)`, each edge → a `Relation` (`Dependency` for `dependsOn`, `FeatureMembership` for `hasPart`, `Domain{kind}` for `allowsTool`/`grantsShard`), call `ufo_types::sysml_model::emit_kerml`, then assert `validate_sysml_v2(&out).is_ok()`. `dispatch_sysml.rs::dispatch_chain_to_sysml_v2` becomes a thin caller of `emit_kerml`; its round-trip tests move/stay green. **No Mermaid.** | SP5-01, SP5-02 | M |
| **SP5-07** | `b00t-graph` example bin | `b00t-c0re-lib/examples/b00t-graph.rs` (new), `b00t-c0re-lib/Cargo.toml` `[[example]]` (+ `required-features = ["store-oxigraph"]`) | subcommands: `load [--tenant T]` (compose SP5-06 + SP5-02 triples → `load_graph`, print quad count); `query '<sparql>' [--json]` (raw `store.query()`); `validate [--tenant T] [--strict]` (SP5-04; exit 1 on any `Severity::Violation` under `--strict`, else print + exit 0); `kerml --view {datum-deps|r0le-grants|service-budgets} [--tenant T]` (SP5-05, print KerML to stdout). | SP5-01…05 | M |
| **SP5-08** | CI job + runbook | `.github/workflows/…`, `docs/runbooks/graph-substrate.md` (new) | New `graph-oxigraph` job (own workflow or a matrix leg): `cargo nextest run -p b00t-c0re-lib --no-default-features --features store-oxigraph`; then `cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph -- validate --strict` against the repo's own `_b00t_/`; then `… --example b00t-graph -- kerml --view datum-deps` piped through a `validate_sysml_v2` assertion. Runbook: the pieces table (TRIPLES / STORE / SPARQL SOURCE / SHACL / KERML VIEW), how to add a shape, the `store-helixdb`↔`store-oxigraph` exclusivity note, SP6 hand-off (KerML text → kr0ki). | SP5-01…07 | S |

## Build order

```
SP5-06  (refactor first — unblocks clean triple compile in b00t-c0re-lib)
   │
SP5-01 ─┬─ SP5-03
        ├─ SP5-04
        └─ SP5-05  (also needs SP5-02)
SP5-02  (parallel with SP5-01)
   │
SP5-07  (joins 01..06)
   │
SP5-08
```

Critical path: `SP5-06 → SP5-01 → SP5-05 → SP5-07 → SP5-08`.

## Integration checkpoints (test the join)

1. **CP-1 triples → store** (SP5-01 + SP5-06): the repo's own `_b00t_/` asserts
   N > 0 quads; a `b00t:dependsOn+` property-path query returns the same
   transitive set `reasoning::adjacency::find_adjacent` produces from the same
   triples.
2. **CP-2 grant graph queryable** (SP5-02 + SP5-01): `SELECT ?t WHERE {
   b00t:r0le/promptexecution/worker b00t:allowsTool ?t }` equals `b00t r0le show
   worker`'s `tool_allowlist`.
3. **CP-3 shapes catch a plant** (SP5-04): a fixture `_b00t_` with one
   budget-less `McpServer` datum and one `dependsOn` cycle → `validate_graph`
   returns exactly those two violations, nothing else.
4. **CP-4 KerML round-trips** (SP5-05): `graph_to_kerml(_, DatumDeps)` output
   parses clean through `ufo_types::sysml::validate_sysml_v2`; every graph node
   appears as a `part def`, every edge as a KerML `Relation`.
5. **CP-5 fan-out parity** (SP5-03): under `store-oxigraph`, `QueryBus::fanout`
   merges `oxigraph:sparql` results with keyword-source results; under the
   default `store-helixdb` build the behaviour is byte-identical to today (the
   source isn't compiled in).

## Risks

- **Backend feature mutual-exclusion** (`store-helixdb` default, `compile_error!`
  on both) — every SP5 build path is `--no-default-features --features
  store-oxigraph`. Contained by shipping an example bin, not a `b00t` subcommand;
  first-class `b00t graph` is a follow-up once storage backends are additive.
- **`compile_datum_triples` move** (SP5-06) — `datum_utils` crate placement;
  fallback is a `&[(String, BootDatum)]` signature so the caller supplies datums.
- **No whole-graph KerML emitter exists** — `ufo_types::mbse` is per-`Stereotyped`
  value only. SP5-05 contributes `sysml_model::emit_kerml` to `ufo-types` (its own
  PR, coordinate the version bump). This is the "contribute, don't duplicate"
  path — `dispatch_sysml.rs` adopts it too.
- **AgentProfile tenant overlay** (SP2-06) — the cross-tenant key-collision shape
  (SP5-04) needs `get_all_datums_for_tenant` to expose which overlay a key came
  from; may need a small return-shape addition.
- **`_b00t_` graph is not a DAG guarantee** — `dependsOn-acyclic` may fire on
  real current data; if so, that's a real datum bug to fix, not a shape to relax
  (the shape is a Violation deliberately).

## Deferred (SP6 or later)

- **Functional kr0ki rendering** of the KerML query view — kr0ki using `holon-viz`
  internally to draw it. **SP6 end state.**
- kr0ki service deploy — `kr0ki.b00t.promptexecution.com` DNS (D4) + CDN (D5).
- First-class `b00t graph` subcommand (needs additive storage backends).
- OWL2 DL reasoning (pellet/konclude) over the loaded graph.
- `rudof` / full SHACL engine (v1 is ~5 SPARQL-expressed shapes).
- S3 / content-hash of the asserted graph.
- `iso_ir → Mermaid/D2` — explicitly not a b00t deliverable; the render side
  consumes KerML/`iso_ir`.
