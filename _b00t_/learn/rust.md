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
