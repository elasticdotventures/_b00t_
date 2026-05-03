# Irontology Capability Delta (86920ab -> a479f23)

Updated submodule: `vendor/irontology-mcp`
- from: `86920ab`
- to: `a479f23`

## High-Signal Capability Changes

1. Persistent Neumann storage hardened
- Sled-backed persistence matured, with explicit config semantics and persistence tests.
- Error propagation improved (sled/JSON errors surfaced instead of silent discard).

2. MCP ingestion (`repo.index`) hardened
- Content size limits enforced (`MAX_CONTENT_BYTES`, `MAX_CHUNKS`).
- UTF-8-safe chunking fixes and corrected persisted-count reporting.
- Additional validation around IRI and indexing paths.

3. Store-backed retrieval path completed (PRD-1 phase)
- Retrieval backend wiring completed across CLI + MCP entrypoints.
- Runtime safety fix avoids Tokio panics when no runtime is present.

4. Search/read determinism and correctness
- `repo.read_symbol` found-detection corrected.
- `repo.search` concurrent store lookups and deterministic ordering for symbol results.

5. MCP server compatibility/transport fixes
- rmcp 0.8.5 build/runtime compatibility repairs.
- Tool registry/startup transport test coverage expanded.

## b00t Integration Impact

Required in this PR:
- `b00t-c0re-lib/src/irontology_bridge.rs`
  - `NeumannConfig.data_dir` -> `NeumannConfig.data_path`
  - `NeumannStore::new(...)` -> `NeumannStore::try_new(...)?`

Why:
- Updated irontology API now expects `data_path: Option<PathBuf>`.

## Follow-up Candidate Patches (optional)

- Evaluate migration from deprecated constructor calls to `try_new` in any remaining b00t-side wrappers.
- Add a small integration test that exercises bridge client initialization against the updated Neumann config type.
