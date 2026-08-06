---
# Rust trait system lowering to logic — FOHH clause model

The Rust compiler's trait solver maps trait/impl declarations into a logical
inference system. The semiformal syntax grammar is a "reasonable-grammar-domain-
syntax" — machine-validatable with bounded AST depth.

## Grammar (BNF)

```
ProgramClause ::= Predicate "."
                | Predicate ":-" Body "."
Body          ::= Predicate ("," Predicate)*
Predicate     ::= Identifier "<" Type ("," Type)* ">"
                | Identifier "(" Type ("," Type)* ")"
```

The grammar closed set of connectives is: `:-` (implication, Horn), `,` (conjunction),
`forall<T> { ... }` (universal quantification over types), `if (Goal) { ... }`
(implication in goal position for generic functions). No negation, no disjunction,
no existential quantification at the solver level — only the solver's internal
unification provides existential behavior.

## Rust → Logic mapping

| Rust construct | Logic representation |
|---|---|
| `trait Clone { }` | Predicate symbol `Clone` |
| `impl Clone for usize` | Program clause: `Clone(usize).` |
| `impl<T> Clone for Vec<T> where T: Clone` | Program clause: `Clone(Vec<?T>) :- Clone(?T).` |
| `fn foo<T: Eq<T>>() { bar::<T>() }` | `forall<T> { if (Eq(T,T)) { barWellFormed(T) } }` — requires FOHH |

The beyond-Horn requirement for generic functions is what distinguishes Rust's
trait system from standard Prolog: FOHH (First-Order Hereditary Harrop) clauses
allow `forall` and `if` in goal bodies, which standard Horn clauses do not. This
is the formal justification for Chalk's SLG resolution.

## Machine validation

The grammar is tractable for automated validation because:
- Closed set of connectives (no user-extensible operators)
- Bounded AST depth per clause
- Typed term constructors matching the trait resolver's internal `ChalkArena`
- No negation-as-failure (Rust uses coinduction for auto traits instead)

Source: https://rustc-dev-guide.rust-lang.org/traits/lowering-to-logic.html

## Where b00t already employs this concept

b00t's datum/ontology layer is a hand-rolled logic-programming system over
`BootDatum` facts. Mapping it onto the Rust trait-solver vocabulary above
exposes what's solid (ground Horn clauses) and what's missing (FOHH's
`forall`/`if` for transitive, role-conditional goals).

| Rustc/Chalk concept | b00t analog | File |
|---|---|---|
| `Ty<I>` / `TyKind<I>` | `InternedDatum` / `BootDatum` | `b00t-cli/src/datum_store.rs:44` (analogy table at `:1-14`) |
| `Interner` trait | `DatumStore` trait | `b00t-cli/src/datum_store.rs:80` |
| Program clause `Clone(Vec<?T>) :- Clone(?T).` | `impl Satisfies<C> for E` | `crates/ufo-types/src/satisfies.rs:33` |
| Ground fact `Clone(usize).` | `b00t:type`/`b00t:requiredFor` triple | `b00t-cli/src/commands/ontology.rs:277` (`sparql_query`) |
| Trait coherence check | `DatumStore::validate_references()` | `b00t-cli/src/datum_store.rs:104` |
| Goal conjunction (Horn body) | `DatumQuery` predicate chain | `b00t-cli/src/datum_store.rs:341-346` |

### 1. `DatumStore::query()` — Horn clauses, already correct

```rust
store.query().proves_role().depends_on("git.cli").run()
```

is literally the goal `Role(?D), DependsOn(?D, git.cli).` — each `.proves_*()`
or `.depends_on()` call pushes one conjunct (`b00t-cli/src/datum_store.rs:347`),
and `.run()` (`:432`) is the solver: filter every fact in the store against the
conjunction. This is pure Horn — no negation, no disjunction — which matches
the "closed set of connectives" tractability property from the grammar above.
`#[must_use]` on `DatumQuery` (`:340`) enforces "a query that isn't `.run()` is
a bug," i.e. an unresolved goal must not be silently discarded.

`validate_references()` (`:104`) is the coherence pass: it's Chalk's overlapping-impl
check repurposed to catch dangling `depends_on`/`skills`/`entangled_*` edges — a
`MissingDependency` error is the b00t equivalent of "no clause proves this predicate."

### 2. `ontology sparql_query` — facts only, no `:-` yet

`sparql_query()` (`b00t-cli/src/commands/ontology.rs:262`) emits ground triples
like `["git", "b00t:requiredFor", "developer"]` — this is a fact database
(`RequiredFor(git, developer).`), not a clause database. There is no `:-` body
and no transitive closure: asking "what does role `orchestrator` require,
including everything `orchestrator`'s direct deps in turn require" is not
expressible today — it would need a `forall<D> { if (RequiredFor(D, R)) {
RequiredForTransitive(D, R) } }`-shaped rule, i.e. genuine FOHH, not another
flat triple emission. If/when `b00t-cli ontology sparql --predicate all` needs
transitive blessing resolution, model it after Chalk's SLG resolution
(`b00t learn chalk-interner`) rather than hand-rolling a graph BFS — the
`DatumStore::query()` conjunction API is already the right shape to extend.

### 3. `Satisfies<C>` — the closest 1:1 mapping, plus a third truth value

`impl Satisfies<AuRdEligibility> for AuRdActivity` (`crates/ufo-types/src/satisfies.rs:212`
doc example) typechecks exactly like a program clause: the `impl` block *is*
`AuRdEligibility(AuRdActivity) :- <fn body>.`, except evaluated at runtime
against instance data instead of at trait-solve time against types. Where it
diverges from the rustc model: `Disposition` (`:119`) is three-valued
(`Satisfied` / `Violated` / `Unknown`), not the two-valued true/false that Horn
resolution assumes. That third state is structurally the same problem Rust
solves with coinduction for auto traits (no negation-as-failure, per the
grammar's "Machine validation" section above) — `Unknown` is an honest
admission that the solver couldn't reach a fixed point, not a `false`.
`EvidenceBridge::evaluate()` (`:254`) then attaches the NS-9/NS-10 evidence
nodes, turning every clause resolution into an audit-trail entry — `Satisfies<T>`
is this FOHH mapping wired directly to arc-kit-au provenance, per the doc
comments in `satisfies.rs` itself (no separate "Tax-Lawyer platform" tie-in
verified beyond this crate — don't cite one).

### 4. Role/blessing authorization — the FOHH gap

`b00t blessing --manifest --role <R>` walks the `depends_on` graph to build a
tool-authorization manifest. Expressed as a goal, that's
`forall<R> { if (Requires(R, S)) { Authorized(R, S) } }` — universal
quantification over roles with a conditional body, the exact beyond-Horn case
the grammar calls out (`generic functions` row in the Rust → Logic table
above). Today this is implemented as an ad hoc graph walk rather than a solved
goal; `DatumQuery` conjunctions already cover the Horn fragment (`proves_role()
+ depends_on()`), so the remaining gap is specifically the `forall`/`if`
transitive-authorization case, not the whole system.

### 5. Command guards — Horn bodies, deliberately no negation

The Command Guards table (`pip install *` → 🦨, `rm -rf /` → 🚫 BLOCKED) and the
Checkpoint Gate's "ANY gate fail → DENY" are conjunctive goal bodies evaluated
left-to-right with halt-on-first-failure — Horn clauses by construction, with
no negation-as-failure. That's not an oversight: it mirrors why Rust's trait
solver avoids negation for auto traits (soundness under coinduction). Any
future guard DSL should keep that closed connective set rather than adding
`NOT`/`OR`, or it loses the "machine-validatable, bounded AST depth" property
this whole grammar is built on.

<!-- b00t:map v1
summary: Rustc trait solver lowers trait/impl decls to FOHH (Horn + forall/if) clauses; b00t's DatumStore::query() is already Horn-clause-shaped, role/blessing authorization is the FOHH gap
tags: rustc, chalk, fohh, horn-clause, trait-solver, datum-store, satisfies, ontology, logic-programming
tier: frontier
cmds: b00t-cli datum show <key>, b00t-cli ontology sparql --subject <X> --predicate all, b00t learn chalk-interner
complexity: 7
-->
