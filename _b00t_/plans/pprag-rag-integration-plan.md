# Proxy-Pointer-RAG Integration Plan (#788)

Status: **design/pending** — no code integration exists yet. This document is
the scoped deliverable for issue #788; it does not implement a working
backend.

## 0. Naming collision — read this first

b00t already uses the term "proxy-pointer" for an unrelated, existing concept:
a provenance-pointer chain (`Evidence → ProvenancePointer → source chunk →
document`) implemented in `b00t-c0re-lib/src/doc_pipeline.rs` (see module doc
`//! ## Proxy-Pointer RAG` at line 28, and the `ev:001 → chunk:0 →
arxiv:2404.17842 → https://...` example in
`b00t-c0re-lib/tests/doc_pipeline_operational_test.rs`). That is a **generic
provenance-tracking pattern**, not the PromptExecution fork of the
`Proxy-Pointer/Proxy-Pointer-RAG` Python package this issue is about. Grepping
the repo for "proxy-pointer" surfaces those five doc_pipeline references and
nothing else — confirming the issue's "zero integration" claim for the actual
`pprag` library. Any implementation work MUST NOT conflate the two; consider
referring to the library integration as `pprag` in code/config to avoid
ambiguity with the existing provenance-pointer terminology.

## 1. What Proxy-Pointer-RAG (pprag) actually is

- Upstream: `github.com/Proxy-Pointer/Proxy-Pointer-RAG`; fork:
  `github.com/PromptExecution/Proxy-Pointer-RAG`.
- Python package (`pip install pprag`), not a Rust crate.
- Structural, pointer-based retrieval (not naive chunk-embedding RAG) —
  reports 100% on FinanceBench in upstream docs.
- Three CLI surfaces:
  - `pprag text ask` — text-only structural retrieval + QA.
  - `pprag multimodal serve` — anchor-aware retrieval with visual citations.
  - `pprag compare serve` — agentic cross-document comparison
    ("DocComparator").
- Embedding stack: Gemini `gemini-embedding-001` (1536d) + FAISS index.

## 2. Existing b00t RAG/knowledge surfaces it could plug into

| Surface | File | Role |
|---|---|---|
| `KnowledgeStoreBackend` trait | `b00t-c0re-lib/src/irontology_bridge.rs:117` | `async fn query/upsert_facts/upsert_edges`, `fn try_new(StoreConfig)`. Already implemented by `NeumannStore`, `HelixDBStore`, `OxigraphStore` (compile-time-selected `ActiveKnowledgeStore`), plus `ZvecStore`, `GrafeoStore`, `QdrantStore`, `DataFabricPipeline` in `b00t-c0re-lib/src/data_fabric/*.rs`. |
| RAGLite integration | `b00t-c0re-lib/src/rag.rs` | `RagLightConfig`/`RagLightManager` — shells out to a Python RAGLite venv for doc loading/indexing/query (`LoaderType`, `DocumentSource`, `IndexingJob`). Closest existing precedent for "wrap an external Python RAG CLI/venv as a b00t backend." |
| Dual backend RAG | `b00t-c0re-lib/src/dual_grok.rs` | Combines RAGLight + Irontology query paths — the natural place to add a third path. |
| `grok` CLI | `b00t-cli/src/commands/grok.rs` | User-facing `b00t grok ask "query" --rag=<backend>` entry point; `raglite.cli.toml` shows the `--rag=raglite` flag convention. |
| Datum precedents | `_b00t_/raglite.cli.toml`, `_b00t_/grok-guru.mcp.toml`, `_b00t_/rag-api.api.toml`, `_b00t_/grok.stack.toml` | Concrete TOML patterns this integration should follow (readiness/repair via `b00t.rhai`, `capability`/`protocol` requires-blocks, `[[b00t.usage]]`). |

`RagLightConfig` is the strongest precedent: it already models "external
Python RAG tool driven by a venv path + CLI subprocess," which is exactly
pprag's shape. A `PpragConfig` following the same struct layout
(`venv_path`, tool path, index/db path, `max_concurrent_jobs`, embedding
model, `LlmConfig`) is the lowest-friction way to add a backend without
inventing a new integration style.

## 3. Integration vectors (ranked)

1. **MCP wrapper** (`_b00t_/pprag.mcp.toml`, drafted alongside this plan) —
   wraps `pprag text ask` / `pprag multimodal serve` / `pprag compare serve`
   as stdio MCP tools, analogous to `grok-guru.mcp.toml`. Lowest integration
   risk: no Rust trait implementation required, just process wiring +
   `b00t.requires` capability declarations (`gemini-embeddings`, `faiss` as
   local vector store).
2. **`KnowledgeStoreBackend` impl** — a `PpragStore` implementing `try_new`/
   `query`/`upsert_facts`/`upsert_edges` by shelling out to the `pprag` CLI
   (mirroring how `RagLightManager` shells to RAGLite). Registers as a fourth
   backend alongside Neumann/HelixDB/Oxigraph behind a `store-pprag` Cargo
   feature, consistent with the existing `cfg_if!`/`compile_error!`
   mutual-exclusion pattern in `irontology_bridge.rs`. Higher effort: needs a
   `FactRecord`/`EdgeRecord` ↔ pprag document-pointer mapping.
3. **Re-ranker on `grok` results** — use `pprag`'s structural retrieval as a
   post-hoc re-ranker over existing RAGLite/Irontology hits in
   `dual_grok.rs`, rather than a primary store. Medium effort, avoids owning
   ingestion.
4. **DocComparator → review/evidence pipeline** — wire `pprag compare serve`
   into the FOL/evidence pipeline already described in `doc_pipeline.rs` for
   cross-document diffing. Speculative; no existing hook point identified.

**Recommendation**: land vector 1 (MCP wrapper) first — it requires zero
changes to `KnowledgeStoreBackend`/`dual_grok.rs` and gives immediate
CLI/agent access to `pprag`'s retrieval quality. Vector 2 (native backend)
is the natural follow-up once real usage validates the FactRecord mapping
is worth the maintenance cost of another compiled backend.

## 4. Open questions / blockers for implementation

- `pprag` requires a Gemini API key (`gemini-embedding-001`) — need a
  `b00t.requires` capability entry (`embeddings` / `gemini-embeddings-v1`)
  analogous to `rag-api.api.toml`'s `openai-embeddings-v1` requirement, and a
  secret-vault entry (`b00t-secret-vault.tomllmd` pattern) for the API key.
- FAISS index storage location/lifecycle (parallel to
  `raglite.cli.toml`'s `data_dir`/`db_url`) is undefined — needs a decision
  before any ingestion path is written.
- No test/benchmark harness in this repo exercises pprag's FinanceBench
  claims; those numbers are upstream-reported only and unverified here.
- Fork drift: PromptExecution's fork vs. upstream `Proxy-Pointer/
  Proxy-Pointer-RAG` — pin a fork commit/tag before wiring `install`/`update`
  commands in `pprag.cli.toml`.

## 5. Deliverable for #788

This plan, `_b00t_/pprag.cli.toml`, and `_b00t_/pprag.mcp.toml` satisfy the
issue's stated requirement ("Create `_b00t_/pprag.cli.toml` and/or
`_b00t_/pprag.mcp.toml` documenting the integration path") without attempting
a blind, untested `KnowledgeStoreBackend` implementation. Both datums are
marked pending/not-installable as-is (no verified fork commit, no secret
wired) — flipping them to installable is follow-up work, not this issue.
