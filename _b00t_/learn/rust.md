---
PyO3 feature control: default-features = false in Cargo workspace dependencies may not fully disable PyO3 if other crates enable it transitively

b00t is big - use metaprogramming: generics, macros, compiler plugins to reduce context
DRY idiomatic semantic abstractions. 


---
agentic role invariant: Use sealed trait + const NAME + Cow str + KnownRole enum for type-level role invariants. RoleRef<T>::new() validates at construction. resolve_role() returns KnownRole sum type (zero-cost), not RoleRef<Worker> (which subverts the invariant).

---
DataFrameSchema generic trait: Don't couple schema traits to one format. Define a generic DataFrameSchema trait with DataType, ColumnDef, CellValue, DataFrame. Implement FocusSchema, GrpcSchema, SqlSchema, ArrowSchema against it. Protocol handlers transform between formats via transform<S1, S2>(). Validate row width, column names, nullable constraints at the trait level.

---
Sealed trait invariants: Use sealed AgenticRole trait + const NAME + Cow str inner + KnownRole enum for type-level role dispatch. Zero-cost, no heap allocation for default path, compile-time invariant enforcement. Do NOT return RoleRef<Worker> from resolve_role() when the resolved name might be "executive" — return KnownRole sum type instead.

---
tokio blocking pitfall: reqwest::blocking::Client panics with "Cannot drop a runtime in a context where blocking is not allowed" when used in a tokio async context (#[tokio::main]). Use curl via std::process::Command instead for CLI tools that might be called from async runtimes.

---
Anti-pattern: adding features on broken infrastructure. ALWAYS fix build issues before adding features. Working around a problem instead of fixing it compounds technical debt and makes the eventual fix harder. Verify `cargo build --features <flag>` compiles before building features that depend on it.

---
Stale pinned deps rot: candle-core was pinned at 0.4 for months while latest was 0.10. The rand_distr/half f16 trait conflict was already fixed upstream. Bumping to 0.10 resolved 20 compile errors with zero code changes. Audit pinned versions quarterly: `cargo search <crate>` vs Cargo.toml.

---
Sealed enum invariants: If resolve_role() returns RoleRef<Worker> but the string says "executive", the type invariant is already broken. Don't paper over it — return a proper sum type (KnownRole) with exhaustively matched variants. The sealed trait + const NAME pattern is only as strong as its factory functions.

---
Version drift: Audit pinned dependency versions quarterly. candle-core 0.4 sat broken for months while 0.10 fixed the rand_distr/half f16 conflict upstream. `cargo search <crate>` before every feature build. A version bump with zero code changes is always cheaper than a workaround.

---
Naming is deployment: The l3dg3rr→ledgerr-mcp→ledg3rr→ledgrrr polyseme maps to proto→linux→cloud→windows. A single codebase with platform-suffix builds prevents fork drift. Use the trait system to abstract platform differences (systemd vs docker for WSL, stdio vs gRPC for cloud).
