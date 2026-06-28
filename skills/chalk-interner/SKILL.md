---
name: chalk-interner
type: skill
hint: Chalk Interner pattern: abstract representation of types/IR behind a trait; enables arena, Box, or integer-indexed storage without changing consumers.
version: 1.0.0
tags: [transferable, domain:type-theory, chalk, interner, trait-solver, fol, type-ir, intern, ty, tykind, rust, datum-store]
tier: frontier
complexity: 7
description: |-
  Chalk Interner pattern: Ty<I>(I::InternedTy) is the HANDLE; TyKind<I> is the OPEN form; the Interner trait bridges them.
---

## What

The Chalk Interner pattern separates a type handle from its open form using a trait. `Ty<I>(I::InternedTy)` is the handle; `TyKind<I>` is the open enum with all variants. The `Interner` trait bridges them with `intern_ty()` and `ty_data()` — consumers only see `Ty<I>` and call `.data()`, never choosing the storage strategy. The embedder (rustc or tests) picks arena-interned integers, `Box<TyKind>`, or `Arc`.

Mapping to b00t: `Ty<I>` → `Datum<S: DatumStore>` handle, `TyKind<I>` → `BootDatum`, `Interner` → `DatumStore` trait, arena-interned → future irontology graph store, Box strategy → current `HashMap<String, BootDatum>` (eager), integer index → future SQLite rowid + lazy-load. A Chalk-style abstraction would let future stores be lazy (SQLite, Qdrant) without changing callers. The FOL/Horn clause connection: Chalk compiles Rust trait goals into Prolog-style Horn clauses; b00t's `compile_datum_triples` → `graph_rules::derive` → `find_adjacent` follows the same pattern.

## When to Use

Use the Chalk Interner pattern when designing a trait-based abstraction layer over multiple storage backends. Load this skill when working on the `DatumStore` trait, adding SQLite/Qdrant backends, or understanding how b00t's triple graph relates to type theory.

## How

1. Define a trait with associated types: `InternedTy` (opaque storage) and methods `intern_ty()` / `ty_data()`.
2. Consumers use the handle type only — they never pick the storage strategy.
3. Implement the trait for each storage backend (eager HashMap, lazy SQLite, graph-based).
4. The abstraction ensures storage changes do not affect consumers.

<!-- b00t:map v1
summary: Chalk Interner — abstract type-IR storage via trait; maps to b00t DatumStore pattern
tags: chalk, interner, fol, type-ir, rust, b00t, datum-store
tier: frontier
cmds: b00t learn chalk-interner
complexity: 7
-->
