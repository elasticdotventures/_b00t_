  ---
  Grok Knowledgebase — Executable Plan

  Desired state: b00t grok ask/digest/learn/status routes to irontology NeumannStore + semantic fusion search instead of the dead Qdrant cluster.

  ---
  Phase 0 — Fix rmcp imports (sm0l, ~30min)

  Goal: cargo build --release in vendor/irontology-mcp passes.

  # Verify exact rmcp types from b00t-mcp (source of truth for rmcp 0.8.5 usage)
  grep -n "use rmcp" /home/brianh/.b00t/b00t-mcp/src/mcp_server_rusty.rs

  # Fix main.rs imports — NeumannConfig is in storage_neumann::config, not root
  # Remove: use storage_neumann::NeumannConfig;
  # Add:    use storage_neumann::config::NeumannConfig;

  # Fix RequestContext path — likely rmcp::service::RequestContext or rmcp::RoleServer
  # Fix ToolDescription path — likely rmcp::model::Tool not ToolDescription
  # Fix Implementation fields (title, icons, website_url) — use Default::default() or ..Default::default()

  cargo build --release --manifest-path vendor/irontology-mcp/Cargo.toml 2>&1 | grep "error\["
  # output: (empty — clean build)

  Test RED→GREEN:
  # Red: binary doesn't exist yet
  ls vendor/irontology-mcp/target/release/irontology-mcp 2>&1
  # output: No such file or directory

  # Green: after fix
  cargo build --release --manifest-path vendor/irontology-mcp/Cargo.toml
  ls vendor/irontology-mcp/target/release/irontology-mcp
  # output: vendor/irontology-mcp/target/release/irontology-mcp

  # Smoke test — binary starts and responds
  echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}' \
    | vendor/irontology-mcp/target/release/irontology-mcp 2>/dev/null | head -1
  # output: {"jsonrpc":"2.0","result":{"protocolVersion":...,"serverInfo":{"name":"irontology-mcp"...

  ---
  Phase 1 — Register irontology as MCP server in Claude Code (sm0l, ~5min)

  # Register via b00t
  b00t mcp install irontology claudecode
  # output: ✅ irontology registered

  # Verify
  claude mcp list | grep irontology
  # output: irontology: vendor/irontology-mcp/target/release/irontology-mcp

  ---
  Phase 2 — Add persistence to NeumannStore (ch0nky, ~2hr)

  Problem: NeumannStore is in-memory only — lost on shutdown.
  Solution: Replace RwLock<HashMap> with sled embedded KV store.

  # Learn what's available
  b00t learn rust.🦀
  # check sled crate for embedded persistence

  Add to storage-neumann/Cargo.toml:
  sled = "0.34"

  Modify neumann.rs:
  - Replace embeddings: RwLock<HashMap<String, EmbeddingRecord>> → sled::Db at ~/.b00t/neumann/
  - has_blob() → db.contains_key(blob_id)
  - upsert_embeddings() → batch insert to sled
  - ingest_turtle() → persist triples as sled key triple::{subject}::{predicate} = object
  - related_objects() → sled scan prefix triple::{subject}::{predicate}

  TDD:
  # Red: persistence test (fails before sled)
  cargo test --manifest-path vendor/irontology-mcp/Cargo.toml -p storage-neumann persist
  # output: FAIL (no test yet)

  # Add test in storage-neumann/tests/persist.rs:
  #   ingest_turtle → restart NeumannStore → related_objects still returns data

  # Green: after sled implementation
  cargo test --manifest-path vendor/irontology-mcp/Cargo.toml -p storage-neumann
  # output: test result: ok. X passed

  ---
  Phase 3 — Implement real SearchBackend (ch0nky, ~3hr)

  Problem: DeterministicBackend returns synthetic results.
  Solution: Wire VectorBackend to NeumannStore + LexicalBackend to codegraph.

  Target architecture:
  fusion_search
    ├─ VectorBackend     → NeumannStore::query(SemanticQuery::Vector)
    ├─ LexicalBackend    → BM25 over stored blob text (sled full-text scan)
    ├─ GraphBackend      → SymbolGraph edge traversal
    └─ OntologyBackend   → NeumannStore::related_objects (RDF triple walk)

  Add embedding provider (needed for VectorBackend):
  # retrieval/Cargo.toml
  reqwest = { version = "0.11", features = ["json"] }  # for ollama embed endpoint

  // retrieval/src/embed.rs
  pub async fn embed_text(text: &str) -> Result<Vec<f32>> {
      // POST http://localhost:11434/api/embeddings (ollama)
      // model: nomic-embed-text (384-dim, runs on CPU)
      // fallback: hash-based mock for tests
  }

  TDD:
  # Check ollama is available for embeddings
  ollama list | grep nomic-embed
  # output: nomic-embed-text:latest ...

  # If missing:
  ollama pull nomic-embed-text
  # output: pulling nomic-embed-text...

  # Red: vector search returns real results (not synthetic)
  cargo test --manifest-path vendor/irontology-mcp/Cargo.toml -p retrieval vector_search_returns_ingested_content
  # output: FAIL

  # Green: after VectorBackend implementation
  cargo test --manifest-path vendor/irontology-mcp/Cargo.toml -p retrieval
  # output: test result: ok. X passed

  ---
  Phase 4 — Wire b00t grok to irontology (ch0nky, ~2hr)

  Modify b00t-c0re-lib/src/grok.rs:

  // Current (broken — Qdrant at 192.168.2.13:6333)
  let qdrant_url = env::var("QDRANT_URL")
      .unwrap_or_else(|_| "http://192.168.2.13:6333".to_string());

  // Replace with irontology MCP client
  // In GrokClient::initialize():
  //   1. Check GROK_BACKEND env var
  //   2. If "irontology" or Qdrant unreachable: spawn irontology-mcp binary
  //   3. Map GrokClient.ask() → mcp tool call "repo.search" + "ontology.related_resources"
  //   4. Map GrokClient.digest() → mcp tool call "repo.index" (Phase 5 — new tool)
  //   5. Map GrokClient.learn() → mcp tool call "repo.index" with URL crawl

  Add env var override:
  # ~/.b00t/env or hive profile:
  GROK_BACKEND=irontology  # routes all grok calls to irontology-mcp
  GROK_BACKEND=qdrant      # legacy (requires Qdrant running)
  # default: auto (tries Qdrant; fallback to irontology)

  TDD:
  # Red: grok ask routes to irontology
  cargo test -p b00t-c0re-lib grok_ask_uses_irontology_backend
  # output: FAIL

  # Green: after wiring
  cargo test -p b00t-c0re-lib -- grok
  # output: test result: ok. X passed

  ---
  Phase 5 — Add repo.index tool to irontology (ch0nky, ~1hr)

  Missing tool: grok digest/learn needs an ingestion path.

  // crates/mcp-server/src/tools/repo_index.rs
  pub struct RepoIndexTool { store: Arc<dyn KnowledgeStore>, ... }

  impl Tool for RepoIndexTool {
      fn name(&self) -> &str { "repo.index" }
      // params: { "topic": str, "content": str, "source": Option<str> }
      // action: chunk_text → embed → upsert_embeddings → Ok({"chunks_created": N})
  }

  ---
  Phase 6 — Integration smoke test (all tiers)

  # Start irontology MCP via hive (or standalone)
  just irontology-build
  vendor/irontology-mcp/target/release/irontology-mcp &

  # Digest something
  GROK_BACKEND=irontology b00t grok digest "b00t" "b00t is a hive agent framework using TOML datums and systemd for local system management"
  # output: ✅ digested: chunk_id=...

  # Ask something back
  GROK_BACKEND=irontology b00t grok ask "what is b00t"
  # output: results: [{ content: "b00t is a hive agent...", score: 0.87 }]

  # Status
  GROK_BACKEND=irontology b00t grok status
  # output: { "backend": "irontology", "blobs": N, "triples": N, "tools": 5 }

  # Full test suite
  cargo test -p b00t-c0re-lib -p b00t-cli -- grok 2>&1 | tail -3
  # output: test result: ok. X passed; 0 failed

  ---
  Checkpoints (git tags)

  # After Phase 0 binary compiles:
  git -C vendor/irontology-mcp tag v0.1.0-binary-green && git -C vendor/irontology-mcp push origin v0.1.0-binary-green

  # After Phase 4 grok wired:
  git tag grok-neumann-v1 && git push origin grok-neumann-v1

  ---
  Tier routing

  ┌────────────────────────┬────────────────────────┬───────────────────────────────────┐
  │         Phase          │     Cognitive tier     │               Model               │
  ├────────────────────────┼────────────────────────┼───────────────────────────────────┤
  │ 0 — fix imports        │ sm0l                   │ haiku — deterministic compile fix │
  ├────────────────────────┼────────────────────────┼───────────────────────────────────┤
  │ 1 — register MCP       │ sm0l                   │ haiku — 3 commands                │
  ├────────────────────────┼────────────────────────┼───────────────────────────────────┤
  │ 2 — sled persistence   │ ch0nky                 │ qwen3-coder-next local            │
  ├────────────────────────┼────────────────────────┼───────────────────────────────────┤
  │ 3 — real SearchBackend │ ch0nky                 │ qwen3-coder-next local            │
  ├────────────────────────┼────────────────────────┼───────────────────────────────────┤
  │ 4 — wire grok          │ ch0nky                 │ qwen3-coder-next local            │
  ├────────────────────────┼────────────────────────┼───────────────────────────────────┤
  │ 5 — repo.index tool    │ ch0nky                 │ qwen3-coder-next local            │
  ├────────────────────────┼────────────────────────┼───────────────────────────────────┤
  │ 6 — integration test   │ sm0l → verify frontier │ haiku → if fail: sonnet           │
  └────────────────────────┴────────────────────────┴───────────────────────────────────┘

  Start: Phase 0 → just irontology-build must be green before any other phase can proceed.

