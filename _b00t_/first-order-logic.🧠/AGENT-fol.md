# First-Order Logic & Reasoning Engine — b00t Agent Guide

## What & Why

First-order logic (FOL) extends propositional logic with predicates, variables, and quantifiers.
The b00t reasoning module uses FOL to derive new facts from the neumann triple store and trace
dependency/trait relationships across the knowledge graph.

## Core Concepts

### Predicates and Facts
```prolog
Clone(usize).                        % ground fact
Clone(Vec(?T)) :- Clone(?T).         % Horn clause rule
```
`A :- B` = "A is true if B is true" (B implies A).

### Horn Clauses (Datalog subset)
Rules where the head has exactly one positive literal. Every Datalog rule is a Horn clause.
Used by `crepe` and `ascent`.

```prolog
path(x, z) :- edge(x, y), path(y, z).   % transitive closure
```

### FOHH — First-Order Hereditary Harrop
Extends Horn clauses to allow `forall` and `if` in goal position. Required for generic Rust
function type-checking. Used by `chalk`.

```prolog
fooTypeChecks :-
  forall<T> { if (Eq(T, T)) { barWellFormed(T) } }.
```

Standard Horn clauses cannot express this — they forbid quantification in goal position.
Reference: Gopalan Nadathur, "A Proof Procedure for the Logic of Hereditary Harrop Formulas."

### Rust Trait → Horn Clause Mapping
```rust
trait Clone {}
impl Clone for usize {}
impl<T: Clone> Clone for Vec<T> {}
```
Lowers to:
```prolog
Clone(usize).
Clone(Vec(?T)) :- Clone(?T).
```
Proof search proves `Clone(Vec<Vec<usize>>)` by backward chaining to `Clone(usize)`.

## b00t Reasoning Stack

```
Source             Engine          Capability
──────────────     ──────────────  ──────────────────────────────
neumann SPO        crepe           Horn derivation, reachability
triples            ascent          Lattices, shortest path, aggregation
syn AST (future)   chalk-solve     FOHH, generic trait bounds
```

### crepe — Datalog proc-macro
```rust
use crepe::crepe;
crepe! {
    @input  struct Triple(String, String, String);   // S P O
    @output struct Reachable(String, String);
    @output struct DependsOn(String, String);

    Reachable(x.clone(), y.clone()) <- Triple(x, p, y), (p == "b00t:relatedTo");
    Reachable(x.clone(), z.clone()) <- Reachable(x, y), (/* ... */ true), Reachable(y, z);
    DependsOn(s.clone(), o.clone()) <- Triple(s, p, o), (p.contains("dependsOn"));
    DependsOn(s.clone(), z.clone()) <- DependsOn(s, y), DependsOn(y, z);
}
```
- Semi-naive evaluation, stratified negation, auto-indexed
- Output: `HashSet<T>` per relation
- Compiles to native Rust — Souffle speeds on 10⁶+ tuples

### ascent — Datalog + Lattices
```rust
use ascent::ascent;
use ascent::lattice::Dual;
ascent! {
    relation triple(String, String, String);
    relation path(String, String);
    lattice shortest_path(String, String, Dual<u32>);

    path(x.clone(), y.clone()) <-- triple(x, p, y), if is_edge_pred(p);
    path(x.clone(), z.clone()) <-- path(x, y), path(y, z);

    shortest_path(x.clone(), y.clone(), Dual(1)) <-- path(x, y);
    shortest_path(x.clone(), z.clone(), Dual(d + 1)) <--
        path(x, y), shortest_path(y, z, ?Dual(d));
}
```
- Lattice keyword: join semantics, only keeps minimum (via `Dual`)
- Parallel variant: `ascent_par!` (requires `Send + Sync` types)
- `ascent_run!` — inline variant with access to local scope variables

### chalk — FOHH trait solver
- `chalk-ir`: type IR (`Ty`, `TraitRef`, `Goal`, `ProgramClause`) — 100% docs
- `chalk-solve`: `RustIrDatabase` trait you must implement (~15 methods)
- `chalk-engine`: SLFT proof engine (Selective Linear Definite clause with Tabling)
- ⚠️ NOT a standalone "add fact, query" API — requires full `RustIrDatabase` impl

## Module: `b00t_c0re_lib::reasoning`

```
b00t-c0re-lib/src/reasoning/
├── mod.rs          — public API: ReasoningEngine, GraphQuery, GraphResult
├── graph_rules.rs  — crepe Horn rules over SPO triples
├── analytics.rs    — ascent lattice/aggregation rules
└── tests.rs        — unit + integration tests
```

### Public API
```rust
use b00t_c0re_lib::reasoning::{ReasoningEngine, GraphQuery};

let triples: Vec<(String, String, String)> = load_from_neumann("default")?;
let mut engine = ReasoningEngine::new(triples);
let result = engine.run()?;

// Horn-derived facts
println!("reachable pairs: {}", result.reachable.len());
println!("dependency chains: {}", result.depends_on.len());

// Lattice analytics
for (from, to, Dual(dist)) in &result.shortest_paths {
    println!("{from} → {to} in {dist} hops");
}
```

## Key Invariants

🤓 `crepe` uses `<-` (single arrow), `ascent` uses `<--` (double arrow). Don't mix them.
🤓 `ascent` lattice `?Dual(d)` in body pattern-matches the lattice value; `Dual(d+1)` in head emits.
🤓 For `String` columns in crepe/ascent, always `.clone()` in rule heads — they're non-Copy.
🤓 `ascent_par!` requires column types to be `Send + Sync`; `Arc<str>` over `String` for hot paths.
🤓 chalk `ImplId` / `TraitId` are opaque — you own the interner; build an `IndexVec` to map them.

## Crate Versions (verified 2026-06)
- `crepe = "0.2"` — 518 stars, 100% docs, proc-macro Datalog
- `ascent = "0.8"` — 556 stars, OOPSLA paper, lattice Datalog
- `chalk-ir = "0.104"` / `chalk-solve = "0.104"` — rust-lang/chalk, 2k stars, pinned versions
- `scryer-prolog = "0.10"` — FOHH-capable WAM Prolog (used by foras; prefer directly)

## Implementation — `b00t_c0re_lib::reasoning` (Phase 1+2 complete)

### Modules

| Module | Purpose | Key types |
|---|---|---|
| `graph_rules` | crepe Horn derivation | `HornResults`, `derive(triples)` |
| `analytics` | ascent lattice analytics | `AnalyticsResults`, `analyze(triples)` |
| `trait_lower` | syn AST → triples | `parse_source_triples(src)`, `parse_dir_triples(dir)` |
| `bound_checker` | instantiated generic Horn proving | `proves_implements(triples, subj, trait, depth)` |
| `neumann_bridge` | load from live neumann store | `ReasoningEngine::from_namespace(ns)` |

### Module design decisions

🤓 **crepe uses `Copy` for all relation types** (0.2.0 — changed from 0.1.x). String is non-Copy.
    Fix: intern all strings to `u32` IDs inside crepe; expose `HashSet<(String,String)>` externally.

🤓 **ascent shortest_path must NOT use transitive `path` as base case**.
    If `path(a,c)` is derived transitively, `shortest_path(a,c,Dual(1))` fires and collapses 2-hop to 1.
    Fix: separate `edge` (direct only) and `path` (transitive); shortest_path recurses over `edge`.

🤓 **`bound_checker::proves_implements`** only handles single-level unification (`Vec<T>` → `Vec<usize>`).
    Nested (`Vec<Vec<usize>>`) works via recursive depth-limited calls.
    Multi-param generics and lifetimes require chalk (Phase 3).

### CLI surface

```bash
# Run Horn+lattice reasoning over a namespace
b00t-cli data fabric reason --namespace autolearn --relation top_skills --top 5

# Run reasoning over ALL derived relations
b00t-cli data fabric reason --namespace autolearn --relation all --format json

# Scan Rust source files → ingest trait impl triples
b00t-cli data fabric scan-traits ./b00t-c0re-lib/src --namespace b00t-traits

# Query derived trait facts
b00t-cli data fabric query --namespace b00t-traits --predicate "b00t:implements" --format table
```

### Prove instantiated generics

```rust
use b00t_c0re_lib::reasoning::{trait_lower, bound_checker};

let src = r#"
    impl Clone for usize {}
    impl<T: Clone> Clone for Vec<T> {}
"#;
let triples = trait_lower::parse_source_triples(src).unwrap();

// Direct fact
assert!(bound_checker::proves_implements(&triples, "usize", "Clone", 3));
// Instantiated generic: Vec<usize>: Clone via T=usize satisfying T:Clone
assert!(bound_checker::proves_implements(&triples, "Vec<usize>", "Clone", 3));
// Nested: Vec<Vec<usize>>: Clone via depth-2 recursion
assert!(bound_checker::proves_implements(&triples, "Vec<Vec<usize>>", "Clone", 4));
```

### Phase 3 gap (chalk)

`bound_checker` handles ground and single-param generic cases. The remaining FOHH cases:
- Multi-param generics: `impl<A: Clone, B: Clone> Clone for (A, B)`
- Where-clause with complex predicates: `where Vec<T>: Clone`
- Lifetime bounds, associated types (`Iterator::Item`)
- `forall<T> { if Eq(T,T) { barWellFormed(T) } }` in goal position

All of these require chalk's `RustIrDatabase` + SLFT engine (Phase 3).
