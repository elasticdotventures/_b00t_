---
name: datum-macro
type: skill
hint: Feasibility: Rust macros as dynamic datums. Three tiers: macro_rules! literals, proc_macro attributes, inventory-crate distributed statics.
version: 1.0.0
tags: [domain:rust-macros, domain:b00t-arch, rust, macro, datum, inventory, proc-macro, codegen, static, compile-time, b00t, chalk, datum-store]
tier: ch0nky
complexity: 6
description: |-
  Three tiers of Rust macro datums: macro_rules! literals (zero new deps), inventory distributed statics (recommended for offline/airgap), proc_macro attributes (most ergonomic but needs new crate).
---

## What

The datum-macro skill covers three tiers of using Rust macros as dynamic datums, all operating at compile time. Tier 1 uses `macro_rules!` to create `BootDatum` literals at compile time with zero new dependencies — useful for core datums baked into the binary. Tier 2 uses the `inventory` crate for distributed static collection where any crate registers a datum at link time and `inventory::iter::<BootDatum>` collects all registered datums without file I/O — this enables offline/airgap binaries with core datums baked in. Tier 3 uses a `proc_macro` attribute (`#[b00t_datum(type = "cli", hint = "...")]`) which is the most ergonomic but requires a new crate.

The Chalk Interner pattern connects here: a `DatumStore` trait would unify all three tiers, with implementations for `TomlFileStore`, `InventoryStore`, `SqliteStore`, and `QdrantStore`. Macros register into `InventoryStore`; the TOML scanner registers into `TomlFileStore`; `get_all_datums()` merges both with operator-added datums overriding builtins by key.

## When to Use

Use Tier 1 (macro_rules!) when you need to define core datums inline without dependencies. Use Tier 2 (inventory) when building offline/airgap binaries with blessed tooling baked in. Use Tier 3 (proc_macro) when ergonomics matter more than dependency footprint.

## How

1. For Tier 1: define a `macro_rules! datum { ... }` that creates `BootDatum` literals. Use `LazyLock` since `BootDatum` uses `String`.
2. For Tier 2: add `inventory` as an optional feature flag. Use `inventory::submit!` in any crate and `inventory::iter::<BootDatum>` in `get_all_datums()`.
3. For Tier 3: create a proc_macro crate with `#[b00t_datum(...)]` attribute that generates `BootDatum` construction + `inventory::submit!`.
4. Note: macros are compile-time only. TOML scan remains canonical for operator-extensible datums.

<!-- b00t:map v1
summary: Rust macro datums — macro_rules!/inventory/proc_macro tiers; all compile-time; DatumStore trait bridges macro+TOML
tags: rust, macro, datum, inventory, compile-time, chalk, datum-store
tier: ch0nky
cmds: b00t learn datum-macro
complexity: 6
-->
