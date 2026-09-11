# Runbook — SysML-v2 coherence substrate (SP5)

`_b00t_#1177` P4. The b00t datum + identity/authz graph as a queryable Oxigraph
SPARQL store, with a SHACL-lite validation gate and a native procedurally-
generated KerML view projection.

## The pieces

| Piece | Where | Cargo feature |
|---|---|---|
| TRIPLES | `b00t-cli` `datum_triples.rs` (structural) + `identity_triples.rs` (tenant/r0le/tool/grant/budget) → `b00t graph emit-triples` writes JSONL `[s,p,o]` | default |
| STORE | `b00t-c0re-lib` `irontology_bridge::OxigraphStore` (embedded SPARQL 1.1, `~/.b00t/oxigraph/<ns>/`); `.store()` borrows the raw `oxigraph::store::Store` | `store-oxigraph` |
| LOAD | `graph_load::load_graph(&store, &triples)` — `b00t:`/`rdfs:` CURIE expansion, IRI-vs-literal object heuristic | `store-oxigraph` |
| SPARQL SOURCE | `query_bus::OxigraphSparqlSource` — property-path adjacency `(dependsOn\|hasPart\|relatedTo)+` from the topic subject; `from_env()` opens `$B00T_OXIGRAPH_NS` (default `graph`) | `store-oxigraph` |
| SHACL | `graph_shapes::validate_graph` — 6 SPARQL shapes: `dependsOn-acyclic`, `every-svc-has-a-budget`, `every-r0le-has-a-tenant`, `r0le-rw-datum`, `r0le-rw-soulscope`, `no-cross-tenant-datum-key` (Warning) | `store-oxigraph` |
| KERML VIEW | `graph_kerml::graph_to_kerml(&store, GraphView)` → `ufo_types::sysml_model::emit_kerml` (native procedural, no templates), round-trip validated via `validate_sysml_v2` | `store-oxigraph` + `ufo-types/sysml` |
| DRIVER | `b00t-c0re-lib/examples/b00t-graph.rs` — `load` / `query` / `validate` / `kerml` over a triples JSONL | `store-oxigraph` |

## Run it locally

```sh
# emit (normal b00t-cli build)
cargo run -p b00t-cli --bin b00t-cli -- graph emit-triples --out /tmp/g.jsonl
# optionally overlay a tenant: --tenant app4dog

# everything below: the store-oxigraph example bin
CORE="cargo run -p b00t-c0re-lib --no-default-features --features store-oxigraph --example b00t-graph --"
$CORE load /tmp/g.jsonl
$CORE query /tmp/g.jsonl 'PREFIX b00t: <http://b00t.promptexecution.com/ontology#> SELECT ?t WHERE { <http://b00t.promptexecution.com/ontology#r0le/_base/worker> b00t:allowsTool ?t }'
$CORE validate /tmp/g.jsonl --strict
$CORE kerml /tmp/g.jsonl --view r0le-grants
```

## Adding a SHACL shape

Append a `Shape { name, kind, severity, sparql, message }` to `SHAPES` in
`b00t-c0re-lib/src/graph_shapes.rs`:
- `ShapeKind::AskFalse` — the shape fails if the `ASK` returns `true`.
- `ShapeKind::SelectAny` — one violation per row; bind `?focus` to the
  offending node.

Add a fixture case to the `graph_shapes` test module (a small triple set that
trips the new shape and asserts it fires).

## `store-helixdb` vs `store-oxigraph`

Mutually exclusive — `compile_error!` in `irontology_bridge.rs` if both or
neither. `b00t-cli` and the default `b00t-c0re-lib` build use `store-helixdb`
(grok / `ask`). The SP5 substrate only compiles under `store-oxigraph`, so it
lives in an example bin + the dedicated `graph-oxigraph` CI workflow, never in
`b00t` proper. A first-class `b00t graph query|validate|kerml` subcommand waits
on the storage backends becoming additive rather than exclusive (separate
task).

## CI

`.github/workflows/graph-oxigraph.yml` — emits the graph from the repo's own
`_b00t_/`, runs the `store-oxigraph` unit tests, gates on `validate --strict`,
and checks the `datum-deps` KerML view renders. A hard `VIOLATION` from
`validate --strict` is a **real datum defect** — fix the datum, or
`b00t task add "graph shape <name> violated by <focus>"` and downgrade that
one shape to `Severity::Warning` with a comment linking the task. Never
silently loosen a shape to get CI green.

## SP6 hand-off

`graph_to_kerml` output (KerML v2 text) is the artifact **kr0ki** consumes to
*render* the view (using `holon-viz` internally). That rendering is **SP6**.
b00t never calls kr0ki here, and **kr0ki never reads datums** (kr0ki
PRD-KR0KI-001 §1.1 — b00t owns the model side, kr0ki the render side, the cut
is at `iso_ir` / KerML).
