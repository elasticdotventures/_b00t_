# Symbolica 2.0 Review — Flashsheets & Type-Erased Evaluation

Issue: #795 (review/evaluation only — adoption work is tracked separately in #796)

## Verdict up front

Symbolica is a real, actively maintained Rust/Python computer-algebra crate with a
genuinely interesting type-erased evaluator pattern. b00t's flashsheets, however, do
not currently have *any* formula evaluator to compare it against, and b00t's existing
"type-erased" surfaces are a different kind of type-erasure (reflection/classifier
metadata, not numeric-domain dispatch). The issue body's claims about Symbolica's API
check out against the vendor's own release post; its claims about how well it maps
onto b00t's current code are aspirational, not descriptive of what exists today.

## What Symbolica actually is (verified)

- Real crate on crates.io: `symbolica = "2.2.0"` (confirmed via `cargo info symbolica`),
  description "A blazing fast computer algebra system", repo now at
  `github.com/symbolica-dev/symbolica` (the issue's `benruijl/symbolica` link is the
  original author's repo/org before the move — still resolves).
- **License is non-standard**: `cargo info` reports `license: unknown` — cargo can't
  parse an SPDX identifier from it. This matches the vendor's own framing ("free for
  hobbyists / non-commercial single-core use; paid license for commercial or
  multi-core use"). This is **not** a permissive OSS license (MIT/Apache-2.0) and
  must be weighed before adding it as a workspace dependency.
- v2.0 highlights, cross-checked against `https://symbolica.io/posts/symbolica_2_0_release/`:
  - **Programmable symbols**: hooks for normalization, printing, derivatives, series
    expansion, and numeric evaluation attach to individual symbols.
  - **Evaluators**: expressions compile to instruction programs; backends include a
    JIT (now the default Python backend), custom ASM/C++/CUDA codegen, and
    double-float arithmetic (~106-bit / 31 decimal digits, ~3x faster than
    arbitrary-precision).
  - **Type-erased evaluation**, the pattern the issue centers on:
    ```rust
    pub struct EvaluationInfo {
        eval_fns: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    }
    pub trait ExternalFunction<T>: Fn(&[T]) -> T + Send + Sync + DynClone {}
    ```
    Callbacks per numeric domain (`f64`, `Complex<f64>`, arbitrary-precision `Float`,
    etc.) are registered and looked up by `TypeId`, and an `EvaluationDomain` trait
    lets a domain declare a fallback (e.g. SIMD-vectorize a scalar callback) so an
    expression graph doesn't need to be rewritten per numeric type.
  - The GitHub README does not itself describe `EvaluationDomain`/`ExternalFunction`
    — those signatures live in the release post / rustdoc, not the repo's front page.
    Treat the exact trait shape as "as documented in the 2.0 post," not as something
    independently re-derived here.

## What b00t actually has today (verified by reading the code, not the issue)

**Flashsheets** (`b00t-lib-chat/src/flash_sheet.rs`, added in PR #781,
`2e724193 feat: flash sheets, state-machine viz, type introspection`):
- `FlashSheet` is a typed sheet over `Dataframe`/`ColumnValue` with rows, columns,
  and cells addressed by `CellAddress`. Every row/column/cell/sheet wraps a
  `SoulConcept` carrying `Vec<SymbolicRule>` (`id`, `expression: String`, metadata) —
  so there's a place to *attach* a rule string, and `CellExpression` has a `Formula`
  variant.
- **`CellExpression::Formula(String)` is an opaque, unparsed string.** There is no
  parser, no evaluator, no expression compiler, no numeric-domain dispatch anywhere
  in this file or its tests. `SymbolicRule { expression: "SUM(score)" }` is stored
  and round-tripped in tests but never interpreted. Mutation only happens through
  `CellGuard`/`CellHook` trait objects (`Arc<dyn CellGuard>`, `Arc<dyn CellHook>`)
  that validate/react to a `CellChange` — ordinary Rust dynamic dispatch, not
  multi-domain numeric evaluation.
- Conclusion: flashsheets have a *slot* shaped like where a symbolic evaluator would
  go (the `Formula`/`SymbolicRule` string fields), but nothing computes over it yet.
  "Symbolica enables formula rendering / interactive computation / expression→kernel
  pipeline in flashsheets" (issue body) describes a future integration, not a gap in
  an existing evaluator.

**Type-erasure patterns that do exist in b00t**, and how they differ from Symbolica's:
- `b00t-lib-chat/src/type_introspection.rs`: `impl_type_introspection!` macro-derives
  a `TypeDescriptor` (classifier string, field/variant names, `std::any::type_name`)
  per struct/enum/trait for runtime reflection — a metadata registry, not a
  computation/evaluation mechanism. No `std::any::TypeId` map, no numeric dispatch.
- `b00t-cli`'s `BootDatum::type_id()` (`b00t-cli/src/lib.rs`,
  `b00t-cli/src/datum_types.rs`) is a **domain method returning a string prefix**
  (e.g. `.skill` → `"skill"`) used for datum classification — unrelated to Rust's
  `std::any::TypeId`, despite the name collision. Grepping the workspace for
  `TypeId`/`type_erased`/`EvaluationDomain` turns up no `std::any::TypeId`-keyed
  dispatch anywhere in `b00t-cli` or `b00t-lib-chat`; the only real hits are this
  comment/test naming coincidence and unrelated vendored SDK code
  (`vendor/runpod-sdk`).
- The issue's claim that this pattern is "directly applicable to b00t's dual-backend
  grok system (RAGLight vs Irontology — swap evaluation domain without changing the
  expression)" is a plausible-sounding analogy but unverified: RAGLite/Irontology
  are retrieval/embedding backends (see `_b00t_/raglite.cli.toml`,
  `_b00t_/irontology.mcp.toml`), not numeric evaluators, and nothing in their code
  currently does `TypeId`-keyed backend swapping either. This is a design idea worth
  exploring, not an existing pattern Symbolica would be "matched against."

## Comparison

| Dimension | Symbolica 2.0 | b00t today |
|---|---|---|
| Expression representation | Full symbolic AST with normalization/printing/derivative hooks | Opaque `String` in `CellExpression::Formula` / `SymbolicRule.expression` |
| Evaluation | Compiled kernels (JIT/ASM/C++/CUDA), double-float & arbitrary precision | None — formulas are stored, never evaluated |
| Multi-domain numeric dispatch | `TypeId`-keyed `ExternalFunction<T>` + `EvaluationDomain` fallback | Not present anywhere in the workspace |
| Type erasure in b00t's own code | N/A | Reflection metadata only (`TypeIntrospection`), not evaluation |
| License | Non-commercial/single-core free, paid otherwise (not OSI-permissive) | N/A — b00t is the consumer |
| Rust-native | Yes, `symbolica` crate links directly | N/A |

## Recommendation

1. **Don't fabricate a mapping that isn't there yet.** Flashsheets need a formula
   evaluator before "Symbolica vs. b00t's type-erased evaluation" is a real
   comparison — right now it's Symbolica vs. nothing.
2. **License is the first gate**, not the API. Before any adoption work (#796),
   confirm whether b00t/flashsheets usage would be commercial and whether
   single-core-only is acceptable, since `symbolica`'s license is not a standard
   permissive OSS license.
3. **If flashsheets grow a real evaluator**, Symbolica's `TypeId`-keyed
   `ExternalFunction<T>` + `EvaluationDomain` fallback pattern (not the whole CAS) is
   the part worth adopting narrowly: it's a clean, documented way to let one
   `CellExpression::Formula` evaluate against `f64`, `Complex<f64>`, or
   arbitrary-precision without rewriting the sheet — that scoped adoption is what
   #796 should implement, gated on the license question above.
4. **Don't route this through the RAGLite/Irontology "dual-backend grok" analogy**
   — that's a separate, unverified idea (retrieval backend selection, not numeric
   evaluation) and conflating it with Symbolica risks scope creep in #796.

## References

- Release post: https://symbolica.io/posts/symbolica_2_0_release/
- Repo: https://github.com/benruijl/symbolica (now https://github.com/symbolica-dev/symbolica)
- Docs: https://symbolica.io/docs/
- crates.io: `symbolica = "2.2.0"` (verified via `cargo info symbolica`, 2026-08-09)
- b00t flashsheets: `b00t-lib-chat/src/flash_sheet.rs` (PR #781, commit `2e724193`)
- b00t type introspection: `b00t-lib-chat/src/type_introspection.rs`
- b00t datum type_id: `b00t-cli/src/lib.rs`, `b00t-cli/src/datum_types.rs`
