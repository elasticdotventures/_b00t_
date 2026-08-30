# Foundry Local Epic — Phase 2: Concurrency + Model Registry + Foundry Local Backend

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `b00t-mcp/src/server_llm.rs`'s `SoulConfig`/`LocalBackend`/`LlmState` with (1) a model-capability registry seed using Phase 1's `ufo-types` types, (2) real per-backend concurrency limiting, and (3) Windows Foundry Local as a registered local backend with real dynamic-port discovery.

**Architecture:** Three additive tasks against one existing, well-tested file (`server_llm.rs`, 1195 lines, 9 existing tests). Each task preserves every existing test and public signature used by tests (`LlmState::from_config`/`from_config_full`) unchanged — new capability is threaded through `LlmState::new()`'s real construction path only, never the test-facing constructors.

**Tech Stack:** Rust, `ufo-types` (workspace git dependency, bumped to Phase 1's branch tip), `tokio::sync::Semaphore`, `reqwest` (blocking, for the Foundry Local discovery shell-out — mirrors this crate's own existing synchronous `SoulConfig::load()`/discovery functions, none of which are `async`).

**Spec:** https://github.com/elasticdotventures/_b00t_/issues/1199 (tracking issue; extends #1177 and fulfills #1182)

## Global Constraints

- `ufo-types` is already a `[workspace.dependencies]` entry in the root `Cargo.toml` (`rev = "19422ac1..."`). Bump this one line to `rev = "7874f5bd2fd5aa681f6c3e2e42fc2e21e9e3a17c"` — this is the tip of `ufo-types`' `feat/foundry-local-phase1-canonical-types` branch (PR https://github.com/PromptExecution/ufo-types/pull/12, not yet merged — same "pin to branch tip" convention already used for the current pin, per the comment already above that line explaining why a tag isn't used). Verify the exact 40-char SHA yourself with `git -C /mnt/d/promptjects/ufo-types log --oneline -1 feat/foundry-local-phase1-canonical-types` before using it — do not trust this plan's copy blindly if it's stale by the time you run it.
- `b00t-mcp/Cargo.toml` does not currently depend on `ufo-types` at all — add `ufo-types = { workspace = true }` to its `[dependencies]` section (alongside the existing `b00t-cli`/`b00t-c0re-lib`/etc. workspace-true lines).
- **Never change the signatures of `LlmState::from_config` or `LlmState::from_config_full`** — 8 of this file's 9 existing tests call `from_config` directly; a signature change would force touching every one of them for no behavioral reason. All new capability threads through `LlmState::new()` (the real, non-test construction path) via new internal helper(s) instead.
- Every new public field on `LocalBackend` MUST have `#[serde(default)]` (or an explicit `default = "..."` fn) — existing `server-soul.tomllm` files on disk (and this file's own `default_soul()` literal) must keep deserializing/constructing without every call site being forced to specify the new field.
- Work happens at `/mnt/d/promptjects/_b00t_` (== `D:\promptjects\_b00t_` on Windows, mounted into WSL) — edit files directly via normal tools against that path, no copying needed. The WSL shell has **no C linker at all**; every `cargo` command MUST go through `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; <cargo command> -j 2"`. This was verified working this session: `cargo check -p b00t-mcp --no-default-features -j 2` succeeded cleanly (20m18s, zero errors) via this exact route. Plain `git`/file-edit tools work directly against the WSL-mounted path.
- Scope every `cargo` command to `-p b00t-mcp` (never `--workspace` or unscoped `cargo test`/`cargo check`) — this is a 30-crate workspace; an unscoped build compiles far more than this plan touches and will be dramatically slower for no benefit. `--no-default-features` is NOT required for `-p b00t-mcp` builds in this plan (the earlier verification used it only to prove viability before this plan existed; b00t-mcp's own default feature set — check `b00t-mcp/Cargo.toml`'s `[features]` section, it may have none, in which case the flag is a no-op either way).
- `vendor/ledgrrr` inside this worktree was locally cloned (not a real submodule checkout) at commit `98abd845ac19e79dcec9ea0f685671b365eefc32` purely to satisfy `b00t-cli`'s transitive path dependency on `b00t-reflect-types` during compilation. Do not edit anything under `vendor/ledgrrr` in this plan — it is not part of this change and not meant to be committed as a modified submodule pointer. If `git status` shows it as dirty/changed, that's a pre-existing worktree-setup artifact, not something this plan's tasks caused — leave it alone.
- Foundry Local's own documentation states it is single-user, not built for concurrent multi-client serving (verified this session against Microsoft's own docs) — this is the concrete justification for defaulting its `max_concurrent` to `Some(1)` in Task 3, not an arbitrary choice.

---

### Task 1: Model registry seed — `LocalBackend.models`

**Files:**
- Modify: `b00t-mcp/Cargo.toml` (add `ufo-types = { workspace = true }` dependency)
- Modify: `Cargo.toml` (root workspace — bump the `ufo-types` rev)
- Modify: `b00t-mcp/src/server_llm.rs` (add `models` field to `LocalBackend`, add import, add a test)

**Interfaces:**
- Consumes: `ufo_types::ModelCapability` (from Phase 1, `ufo-types` crate — `pub struct ModelCapability { pub model_name: String, pub formats: Vec<DataFormat>, pub metadata: HashMap<String, String> }`, constructed via `ModelCapability::new(name, formats)`).
- Produces: `LocalBackend { ..., models: Vec<ModelCapability> }`. Later tasks/phases (Phase 3's coherence/reconciliation layer) will read this field to know what format(s) a backend's model(s) claim to serve. This task does not populate real model lists for the existing default backends (mistralrs/llama-cpp/vllm/candle-phi/qwen3-embed) — that's real domain knowledge belonging to a later, separately-scoped pass, not something to guess here. Only Task 3's new Foundry Local entry gets a populated `models` list, since Foundry Local's default model is already known and named elsewhere in this ecosystem (`FOUNDRY_LOCAL_MODEL = "phi-4-mini"` in ledgrrr's `internal_openai.rs`).

- [ ] **Step 1: Confirm the exact ufo-types branch tip SHA**

Run: `git -C /mnt/d/promptjects/ufo-types log --oneline -1 feat/foundry-local-phase1-canonical-types`
Expected output starts with a 7-char short SHA whose first 7 characters are `7874f5b`. If it differs, use the actual SHA you see for every step below instead of the one written in this plan.

- [ ] **Step 2: Bump the workspace ufo-types pin**

In the root `/mnt/d/promptjects/_b00t_/Cargo.toml`, find this exact line (in the `[workspace.dependencies]` section):

```toml
ufo-types = { git = "https://github.com/PromptExecution/ufo-types.git", rev = "19422ac1988f916c83e15260b1f1a8969ef8d65a" }
```

Replace it with (using the SHA you confirmed in Step 1, full 40 characters — get the full SHA via `git -C /mnt/d/promptjects/ufo-types rev-parse feat/foundry-local-phase1-canonical-types`):

```toml
ufo-types = { git = "https://github.com/PromptExecution/ufo-types.git", rev = "7874f5bd2fd5aa681f6c3e2e42fc2e21e9e3a17c" }
```

Do not change the comment block above that line — it still accurately explains why a rev-pin (not a tag) is used.

**Retrospective note (added after both `ufo-types` PRs merged):** the pin now on `main` reads `rev = "06f3a9992fc40c9d90a35c2f404ffce23886bd37"` — `ufo-types`' PR #14 (Phase 3) merge commit, itself built on PR #12 (Phase 1)'s merge. This is expected, healthy drift (the pin was bumped forward past both merges after this task landed), not a discrepancy with what actually shipped here.

- [ ] **Step 3: Add ufo-types as a b00t-mcp dependency**

In `b00t-mcp/Cargo.toml`, in its `[dependencies]` section, add this line alongside the existing workspace-true dependency lines (e.g. near `b00t-c0re-lib = { workspace = true }`):

```toml
ufo-types = { workspace = true }
```

- [ ] **Step 4: Write the failing test**

In `b00t-mcp/src/server_llm.rs`, inside the existing `#[cfg(test)] mod tests { ... }` block (add near the other `#[test]` functions, e.g. after `default_soul_includes_telnyx_as_a_remote_backend`), add:

```rust
    #[test]
    fn local_backend_models_field_defaults_to_empty_and_roundtrips() {
        use ufo_types::{DataFormat, ModelCapability};

        // Default-constructed (as every existing default_soul() entry is)
        // gets an empty models list — this field is additive, not required.
        let soul = default_soul("test-host");
        let mistralrs = soul
            .backends
            .local
            .iter()
            .find(|b| b.name == "mistralrs")
            .expect("mistralrs must be a default local backend");
        assert!(mistralrs.models.is_empty());

        // A backend WITH models set roundtrips through TOML (the real
        // on-disk config format `SoulConfig::load` reads/writes).
        let mut backend = LocalBackend {
            name: "test-backend".into(),
            port: 9999,
            kind: "openai-compat".into(),
            enabled: true,
            models: Vec::new(),
            max_concurrent: None,
        };
        backend.models.push(
            ModelCapability::new("test-model", vec![DataFormat::Json, DataFormat::PlainText])
                .with_metadata("quantization", "int4"),
        );
        let toml_str = toml::to_string_pretty(&backend).unwrap();
        let back: LocalBackend = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.models.len(), 1);
        assert_eq!(back.models[0].model_name, "test-model");
        assert_eq!(back.models[0].formats, vec![DataFormat::Json, DataFormat::PlainText]);
        assert_eq!(back.models[0].metadata.get("quantization"), Some(&"int4".to_string()));
    }
```

Note: this test constructs a `LocalBackend` struct literal directly with a `max_concurrent: None` field that doesn't exist yet — that's deliberate, it's also exercised by this task so Task 2 doesn't need to touch this test again. If you find this confusing, it's because Task 1 and Task 2 both touch `LocalBackend`'s field list; this task adds both new fields to the struct literal in one step to avoid a second edit to this same test in Task 2, but Task 2 owns implementing the actual concurrency *behavior* — this task only needs the field to exist and default correctly.

- [ ] **Step 2b: Run test to verify it fails**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 local_backend_models_field -- --nocapture"`
Expected: FAIL to compile — `ufo_types` not found as an import path yet, and `LocalBackend` has no `models`/`max_concurrent` fields yet.

- [ ] **Step 5: Write the minimal implementation**

In `b00t-mcp/src/server_llm.rs`, add the import near the top (with the other `use` statements):

```rust
use ufo_types::ModelCapability;
```

Then modify the `LocalBackend` struct definition to add both new fields (both go in this task, per the note above):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalBackend {
    pub name: String,
    pub port: u16,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Which data format(s) this backend's model(s) claim to serve — a
    /// registry seed, not yet consumed by any routing logic in this phase.
    /// Empty by default; existing `default_soul()` entries don't populate
    /// this (real per-model capability data is out of this task's scope).
    #[serde(default)]
    pub models: Vec<ModelCapability>,
    /// Caps concurrent in-flight requests proxied to this backend. `None`
    /// means unlimited (the pre-existing behavior for every backend before
    /// this field existed). Enforced in Task 2, not this task.
    #[serde(default)]
    pub max_concurrent: Option<u32>,
}
```

Now every existing `LocalBackend { ... }` struct literal in this file (in `default_soul()`) needs the two new fields added. Find `default_soul()`'s `local: vec![...]` block and add `models: Vec::new(), max_concurrent: None,` to each of the 5 existing entries (`mistralrs`, `llama-cpp`, `vllm`, `candle-phi`, `qwen3-embed`). For example, the first one becomes:

```rust
                LocalBackend { name: "mistralrs".into(), port: 8181, kind: "openai-compat".into(), enabled: true, models: Vec::new(), max_concurrent: None },
```

Apply the same two additions (`models: Vec::new(), max_concurrent: None`) to the other 4 entries in that same `vec![...]` block, keeping every other field exactly as it already is.

- [ ] **Step 6: Run test to verify it passes**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 local_backend_models_field -- --nocapture"`
Expected: PASS

- [ ] **Step 7: Run the full server_llm test module to confirm no breakage**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 server_llm:: -- --nocapture"`
Expected: PASS — all 10 tests in this module (9 pre-existing + 1 new) green.

- [ ] **Step 8: Commit**

```bash
cd /mnt/d/promptjects/_b00t_
git add Cargo.toml b00t-mcp/Cargo.toml b00t-mcp/src/server_llm.rs
git commit -m "feat(server_llm): add LocalBackend.models registry seed and max_concurrent field"
```

---

### Task 2: Concurrency enforcement

**Files:**
- Modify: `b00t-mcp/src/server_llm.rs` (`resolve_upstream`, `LlmState`, `proxy_chat`, `forward_chat_verbatim`, `proxy_embeddings`, plus tests)

**Interfaces:**
- Consumes: `LocalBackend.max_concurrent: Option<u32>` (from Task 1), `discover_local`'s existing `Option<(String, String)>` return (name, url) — unchanged signature.
- Produces: `resolve_upstream(soul, for_embeddings) -> (String, String, Option<u32>)` (signature CHANGES — third element is the resolved backend's `max_concurrent`, looked up by name; `None` for remote/explicit-URL/fallback resolutions). `LlmState` gains `chat_semaphore: Option<Arc<tokio::sync::Semaphore>>` and `embeddings_semaphore: Option<Arc<tokio::sync::Semaphore>>`. `LlmState::from_config`/`from_config_full` signatures and behavior are UNCHANGED (both always produce `chat_semaphore: None, embeddings_semaphore: None` — unlimited, matching every pre-existing test's expectations). A new `LlmState::from_config_full_with_concurrency(upstream_url, upstream_key, embeddings_upstream_url, embeddings_upstream_key, chat_max_concurrent: Option<u32>, embeddings_max_concurrent: Option<u32>) -> Self` is what `LlmState::new()` calls instead of `from_config_full`.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block, near `test_key_create_and_validate`:

```rust
    #[tokio::test]
    async fn concurrency_limit_serializes_requests_to_a_backend() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _temp_home = TempHome::new();

        // Fake upstream that tracks the max number of simultaneously
        // in-flight requests it observed, via a slow handler that holds a
        // counter up for long enough to overlap if the semaphore didn't
        // actually serialize anything.
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));
        let concurrent_for_handler = concurrent.clone();
        let max_observed_for_handler = max_observed.clone();
        let app = Router::new().route(
            "/chat/completions",
            axum::routing::post(move |_body: Bytes| {
                let concurrent = concurrent_for_handler.clone();
                let max_observed = max_observed_for_handler.clone();
                async move {
                    let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                    max_observed.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    concurrent.fetch_sub(1, Ordering::SeqCst);
                    Json(stop_upstream_response("ok"))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let upstream_url = format!("http://{addr}");

        let state = Arc::new(LlmState::from_config_full_with_concurrency(
            &upstream_url, "", &upstream_url, "", Some(1), None,
        ));

        let request_body = json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "hi"}]
        });
        let body_bytes = Bytes::from(serde_json::to_vec(&request_body).unwrap());

        // Fire 3 concurrent requests through proxy_chat.
        let mut handles = Vec::new();
        for _ in 0..3 {
            let state = state.clone();
            let body = body_bytes.clone();
            handles.push(tokio::spawn(async move {
                proxy_chat(State((state, true)), HeaderMap::new(), body).await.into_response()
            }));
        }
        for h in handles {
            let resp = h.await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        assert_eq!(
            max_observed.load(Ordering::SeqCst),
            1,
            "max_concurrent=Some(1) must serialize requests to exactly 1 in flight at a time"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 concurrency_limit_serializes -- --nocapture"`
Expected: FAIL to compile — `LlmState::from_config_full_with_concurrency` doesn't exist yet.

- [ ] **Step 3: Write the minimal implementation**

First, change `resolve_upstream`'s signature and every return point to also produce the resolved backend's `max_concurrent`. Replace the whole function:

```rust
/// Resolves the upstream to proxy to. `for_embeddings` matters because a local
/// backend can be embeddings-only (b00t-embed-serve, kind="embeddings") — a
/// chat request must never land there, and an embeddings request should prefer
/// it over a general chat backend that may not implement /v1/embeddings at all.
/// The third return element is the resolved local backend's `max_concurrent`
/// (looked up by name in `soul.backends.local`); `None` for the explicit-URL
/// override, remote-backend, and no-upstream-configured fallback paths, none
/// of which have a concept of a per-backend concurrency cap in this file.
fn resolve_upstream(soul: &SoulConfig, for_embeddings: bool) -> (String, String, Option<u32>) {
    let explicit_url_var = if for_embeddings { "B00T_SERVER_EMBEDDINGS_UPSTREAM_URL" } else { "B00T_SERVER_UPSTREAM_URL" };
    if let Ok(url) = std::env::var(explicit_url_var) {
        if !url.is_empty() {
            let key = std::env::var("B00T_SERVER_UPSTREAM_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .unwrap_or_default();
            eprintln!("🌐 upstream (explicit): {}", url);
            return (url, key, None);
        }
    }
    if for_embeddings {
        // Prefer an embeddings-only local backend; fall back to a general
        // openai-compat local backend in case it also serves /v1/embeddings.
        if let Some((name, url)) = discover_local(soul, Some("embeddings")) {
            eprintln!("📍 upstream (soul/local {}, embeddings): {}", name, url);
            let max_concurrent = soul.backends.local.iter().find(|b| b.name == name).and_then(|b| b.max_concurrent);
            return (url, String::new(), max_concurrent);
        }
        if let Some((name, url)) = discover_local(soul, None) {
            eprintln!("📍 upstream (soul/local {}, chat backend used for embeddings): {}", name, url);
            let max_concurrent = soul.backends.local.iter().find(|b| b.name == name).and_then(|b| b.max_concurrent);
            return (url, String::new(), max_concurrent);
        }
    } else if let Some((name, url)) = discover_local(soul, None) {
        eprintln!("📍 upstream (soul/local {}): {}", name, url);
        let max_concurrent = soul.backends.local.iter().find(|b| b.name == name).and_then(|b| b.max_concurrent);
        return (url, String::new(), max_concurrent);
    }
    if let Some((name, key, url)) = discover_remote(soul) {
        eprintln!("🌐 upstream (soul/remote {}): {}", name, url);
        return (url, key, None);
    }
    eprintln!("⚠️  No upstream configured — populate ~/.b00t/{}", SOUL_PATH);
    ("http://localhost:8181/v1".to_string(), String::new(), None)
}
```

Now add `use tokio::sync::Semaphore;` to the top-level imports (near the existing `use tokio::sync::RwLock;`).

Add two fields to `LlmState`, right after `embeddings_upstream_key`:

```rust
    /// `None` = unlimited (the behavior for every backend before this field
    /// existed, and always the case for `from_config`/`from_config_full` —
    /// only `LlmState::new()`'s real construction path can produce `Some`).
    pub chat_semaphore: Option<Arc<Semaphore>>,
    pub embeddings_semaphore: Option<Arc<Semaphore>>,
```

Change `LlmState::new()` to call the new concurrency-aware constructor:

```rust
    pub fn new() -> Self {
        let soul = SoulConfig::load();
        let (url, key, chat_max_concurrent) = resolve_upstream(&soul, false);
        let (embed_url, embed_key, embeddings_max_concurrent) = resolve_upstream(&soul, true);
        Self::from_config_full_with_concurrency(&url, &key, &embed_url, &embed_key, chat_max_concurrent, embeddings_max_concurrent)
    }
```

Change `from_config` and `from_config_full` to delegate to the new constructor with `None, None` (preserving their exact prior behavior and signatures):

```rust
    /// Convenience for tests / callers that don't care about a distinct
    /// embeddings backend — uses the same upstream for both. Always
    /// unlimited concurrency (`None, None`) — use
    /// `from_config_full_with_concurrency` directly if a test needs to
    /// exercise concurrency limiting.
    pub fn from_config(upstream_url: &str, upstream_key: &str) -> Self {
        Self::from_config_full(upstream_url, upstream_key, upstream_url, upstream_key)
    }

    pub fn from_config_full(
        upstream_url: &str,
        upstream_key: &str,
        embeddings_upstream_url: &str,
        embeddings_upstream_key: &str,
    ) -> Self {
        Self::from_config_full_with_concurrency(upstream_url, upstream_key, embeddings_upstream_url, embeddings_upstream_key, None, None)
    }

    pub fn from_config_full_with_concurrency(
        upstream_url: &str,
        upstream_key: &str,
        embeddings_upstream_url: &str,
        embeddings_upstream_key: &str,
        chat_max_concurrent: Option<u32>,
        embeddings_max_concurrent: Option<u32>,
    ) -> Self {
        let home = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));
        let keys_file = home.join("server-keys.json");
        let keys = load_keys_from_file(&keys_file);
        let mtime = std::fs::metadata(&keys_file).and_then(|m| m.modified()).ok();
        Self {
            upstream_url: upstream_url.trim_end_matches('/').to_string(),
            upstream_key: upstream_key.to_string(),
            embeddings_upstream_url: embeddings_upstream_url.trim_end_matches('/').to_string(),
            embeddings_upstream_key: embeddings_upstream_key.to_string(),
            chat_semaphore: chat_max_concurrent.map(|n| Arc::new(Semaphore::new(n as usize))),
            embeddings_semaphore: embeddings_max_concurrent.map(|n| Arc::new(Semaphore::new(n as usize))),
            keys: Arc::new(RwLock::new(keys)),
            keys_file,
            spotlight_log: home.join("spotlight.jsonl"),
            keys_file_mtime: Arc::new(RwLock::new(mtime)),
        }
    }
```

Now gate the three upstream-calling handlers. In `proxy_chat`, wrap the `send_upstream_chat` call (the `first = match send_upstream_chat(...)` block) with a permit acquire — insert this right before that `let start = std::time::Instant::now();` line:

```rust
    let _permit = match &state.chat_semaphore {
        Some(sem) => Some(sem.clone().acquire_owned().await.expect("semaphore not closed")),
        None => None,
    };
```

(`_permit` stays alive for the rest of the function via normal Rust scoping — it's dropped, releasing the slot, when `proxy_chat` returns.)

Apply the identical pattern to `forward_chat_verbatim` (insert right before its `let start = std::time::Instant::now();` line, using `state.chat_semaphore` — note `state` here is `&LlmState`, not `Arc<LlmState>`, so use `state.chat_semaphore.clone()` inside the match, not `state.chat_semaphore` moved) and to `proxy_embeddings` (using `state.embeddings_semaphore`, right before its own `let start = std::time::Instant::now();` line).

- [ ] **Step 4: Run test to verify it passes**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 concurrency_limit_serializes -- --nocapture"`
Expected: PASS

- [ ] **Step 5: Run the full server_llm test module**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 server_llm:: -- --nocapture"`
Expected: PASS — all 11 tests (10 from Task 1 + this new one) green. This is the step that proves `from_config`/`from_config_full`'s signature preservation actually held — every pre-existing test calling them must still compile and pass unchanged.

- [ ] **Step 6: Commit**

```bash
cd /mnt/d/promptjects/_b00t_
git add b00t-mcp/src/server_llm.rs
git commit -m "feat(server_llm): enforce per-backend max_concurrent via a request semaphore"
```

---

### Task 3: Windows Foundry Local backend

**Files:**
- Modify: `b00t-mcp/Cargo.toml` (add `blocking` to the existing `reqwest` feature list)
- Modify: `b00t-mcp/src/server_llm.rs` (`discover_local`, `default_soul`, new discovery functions, tests)

**Interfaces:**
- Consumes: nothing new from earlier tasks structurally, but populates Task 1's `models` field and Task 2's `max_concurrent` field on its new `LocalBackend` entry.
- Produces: `fn discover_foundry_local_endpoint() -> Result<Option<String>, String>` (and its three private helpers `parse_foundry_endpoint`/`discover_foundry_rest_endpoint`/`normalize_foundry_endpoint`), a `default_soul()` entry named `"foundry-local"`, and a special case inside `discover_local()`'s loop that calls this function instead of the generic fixed-port TCP probe when it encounters that entry.

- [ ] **Step 1: Add the reqwest blocking feature**

In `b00t-mcp/Cargo.toml`, find:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

Change to:

```toml
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }
```

- [ ] **Step 2: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block, near the other `default_soul_includes_*` tests:

```rust
    #[test]
    fn default_soul_includes_foundry_local_as_a_local_backend() {
        let soul = default_soul("test-host");
        let foundry = soul
            .backends
            .local
            .iter()
            .find(|b| b.name == "foundry-local")
            .expect("foundry-local must be a default local backend");
        assert_eq!(foundry.kind, "openai-compat");
        assert!(foundry.enabled);
        // Foundry Local is documented single-user, not built for concurrent
        // multi-client serving — default to serializing requests to it.
        assert_eq!(foundry.max_concurrent, Some(1));
        assert!(!foundry.models.is_empty(), "foundry-local should declare its known default model");
        assert_eq!(foundry.models[0].model_name, "phi-4-mini");
    }

    #[test]
    fn foundry_local_is_positioned_after_vllm_and_before_candle_phi() {
        // Preserves the existing fallback-priority ordering: real GPU/NPU
        // runtimes first, Foundry Local next (also hardware-accelerated when
        // present), candle-phi (CPU-only, ~0.53 tok/s) stays the absolute
        // last resort.
        let soul = default_soul("test-host");
        let names: Vec<&str> = soul.backends.local.iter().map(|b| b.name.as_str()).collect();
        let vllm_pos = names.iter().position(|n| *n == "vllm").unwrap();
        let foundry_pos = names.iter().position(|n| *n == "foundry-local").unwrap();
        let candle_pos = names.iter().position(|n| *n == "candle-phi").unwrap();
        assert!(vllm_pos < foundry_pos, "foundry-local must come after vllm");
        assert!(foundry_pos < candle_pos, "foundry-local must come before candle-phi");
    }

    #[test]
    fn parse_foundry_endpoint_extracts_http_url_from_cli_output() {
        // Real sample shape `foundry service status` produces (per the
        // proven ledgrrr port of this parser) — a human-readable line
        // containing a bare http:// URL among other text/punctuation.
        let raw = "Model management service is running on http://127.0.0.1:5273/\nSome other line.";
        let found = parse_foundry_endpoint(raw);
        assert_eq!(found.as_deref(), Some("http://127.0.0.1:5273"));
    }

    #[test]
    fn parse_foundry_endpoint_returns_none_when_no_url_present() {
        assert!(parse_foundry_endpoint("service is not running").is_none());
    }

    #[test]
    fn normalize_foundry_endpoint_strips_known_suffixes() {
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/"), "http://127.0.0.1:5273");
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/v1/chat/completions"), "http://127.0.0.1:5273");
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/v1"), "http://127.0.0.1:5273");
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/openai"), "http://127.0.0.1:5273");
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 foundry -- --nocapture"`
Expected: FAIL to compile — `foundry-local` entry, `parse_foundry_endpoint`, `normalize_foundry_endpoint` don't exist yet.

- [ ] **Step 4: Write the minimal implementation**

Add these three functions to `server_llm.rs`, near `resolve_upstream` (e.g. right after it, before the `// ── State ──` section header):

```rust
/// Real Windows Foundry Local model name this ecosystem already names
/// elsewhere (`ledgrrr`'s `ledgerr-host::internal_openai::FOUNDRY_LOCAL_MODEL`).
const FOUNDRY_LOCAL_MODEL: &str = "phi-4-mini";

/// Discovers Foundry Local's live REST endpoint. Unlike every other local
/// backend in `default_soul()`, Foundry Local does not listen on a fixed,
/// known port — it's assigned dynamically per-launch. Ported from
/// `ledgrrr`'s `ledgerr-host::internal_openai::discover_foundry_local_endpoint`
/// (same shell-out + parse approach, same env var override name kept for
/// operator familiarity across both codebases).
fn discover_foundry_local_endpoint() -> Result<Option<String>, String> {
    if let Ok(endpoint) = std::env::var("LEDGERR_FOUNDRY_LOCAL_ENDPOINT") {
        let trimmed = endpoint.trim();
        if !trimmed.is_empty() {
            return Ok(Some(normalize_foundry_endpoint(trimmed)));
        }
    }

    let output = std::process::Command::new("foundry")
        .args(["service", "status"])
        .output()
        .map_err(|error| format!("failed to run `foundry service status`: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    if !output.status.success() {
        return Err(format!(
            "`foundry service status` exited with {}: {}",
            output.status,
            combined.trim()
        ));
    }

    let Some(endpoint) = parse_foundry_endpoint(&combined) else {
        return Ok(None);
    };

    Ok(Some(discover_foundry_rest_endpoint(&endpoint).unwrap_or(endpoint)))
}

fn parse_foundry_endpoint(raw: &str) -> Option<String> {
    raw.split(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | '[' | ']'))
        .find_map(|token| {
            let endpoint = token
                .trim_matches(|ch| matches!(ch, '.' | ';' | ')' | '('))
                .trim_end_matches('/');
            if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
                Some(normalize_foundry_endpoint(endpoint))
            } else {
                None
            }
        })
}

fn discover_foundry_rest_endpoint(endpoint: &str) -> Option<String> {
    use std::time::Duration;

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct FoundryStatus {
        endpoints: Vec<String>,
    }

    let status_url = format!("{}/openai/status", normalize_foundry_endpoint(endpoint));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .connect_timeout(Duration::from_secs(1))
        .build()
        .ok()?;
    let status = client.get(&status_url).send().ok()?.json::<FoundryStatus>().ok()?;
    status
        .endpoints
        .into_iter()
        .find(|endpoint| endpoint.starts_with("http://") || endpoint.starts_with("https://"))
        .map(|endpoint| normalize_foundry_endpoint(&endpoint))
}

fn normalize_foundry_endpoint(endpoint: &str) -> String {
    endpoint
        .trim()
        .trim_end_matches('/')
        .trim_end_matches("/v1/chat/completions")
        .trim_end_matches("/v1")
        .trim_end_matches("/openai")
        .to_string()
}
```

Now modify `discover_local()` to special-case the `"foundry-local"` entry. Replace the whole function:

```rust
/// `want_kind`: when `Some("embeddings")`, only considers backends whose `kind`
/// is exactly "embeddings" (e.g. b00t-embed-serve, which has no /v1/chat/completions
/// at all) — chat/models discovery must never land on an embeddings-only backend.
/// When `None`, only considers "openai-compat" backends (the historical default,
/// used for chat/models) — an embeddings-only backend is never a valid chat target.
fn discover_local(soul: &SoulConfig, want_kind: Option<&str>) -> Option<(String, String)> {
    let target_kind = want_kind.unwrap_or("openai-compat");
    for be in &soul.backends.local {
        if !be.enabled || be.kind != target_kind { continue; }
        // Foundry Local has no fixed port (its `port` field is an unused
        // sentinel — see its default_soul() entry) — it needs a real
        // shell-out discovery step instead of the generic TCP probe below.
        if be.name == "foundry-local" {
            match discover_foundry_local_endpoint() {
                Ok(Some(endpoint)) => {
                    eprintln!("🔍 local backend (soul): {} (foundry-local dynamic discovery)", be.name);
                    return Some((be.name.clone(), format!("{}/v1", endpoint)));
                }
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("⚠️  foundry-local discovery failed: {e}");
                    continue;
                }
            }
        }
        let addr: SocketAddr = format!("127.0.0.1:{}", be.port).parse().ok()?;
        if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
            eprintln!("🔍 local backend (soul): {} :{} (kind={})", be.name, be.port, be.kind);
            return Some((be.name.clone(), format!("http://127.0.0.1:{}/v1", be.port)));
        }
    }
    None
}
```

Now add the new entry to `default_soul()`'s `local: vec![...]` block, positioned after `vllm` and before `candle-phi`:

```rust
                LocalBackend { name: "vllm".into(), port: 8000, kind: "openai-compat".into(), enabled: true, models: Vec::new(), max_concurrent: None },
                // 🤓 Windows Foundry Local — real Microsoft local inference
                // runtime, dynamic port (no fixed `port` value applies; the
                // literal 0 below is an unused sentinel — see discover_local's
                // foundry-local special case). Foundry Local's own docs state
                // it is single-user, not built for concurrent multi-client
                // serving, hence max_concurrent: Some(1).
                LocalBackend {
                    name: "foundry-local".into(),
                    port: 0,
                    kind: "openai-compat".into(),
                    enabled: true,
                    models: vec![ModelCapability::new(FOUNDRY_LOCAL_MODEL, vec![DataFormat::Json, DataFormat::PlainText])],
                    max_concurrent: Some(1),
                },
```

(This inserts between the existing `vllm` line and the `candle-phi` line — leave both of those exactly as they are, including the doc comment already above `candle-phi`.)

Add the import for `DataFormat` — change the existing `use ufo_types::ModelCapability;` line (from Task 1) to:

```rust
use ufo_types::{DataFormat, ModelCapability};
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 foundry -- --nocapture"`
Expected: PASS (5 new tests)

- [ ] **Step 6: Run the full server_llm test module**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo test -p b00t-mcp -j 2 server_llm:: -- --nocapture"`
Expected: PASS — all 16 tests (11 from Tasks 1-2 + 5 new) green.

- [ ] **Step 7: Build with default features too, to catch anything the earlier --no-default-features check masked**

Run: `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; cargo build -p b00t-mcp -j 2 2>&1 | Select-String -Pattern 'error|warning: unused'"`
Expected: no `error` lines. `warning: unused` lines pre-existing in this crate (seen during this session's earlier verification build) are not a regression — only flag NEW ones that reference `server_llm.rs` or files this plan touched.

- [ ] **Step 8: Commit**

```bash
cd /mnt/d/promptjects/_b00t_
git add b00t-mcp/Cargo.toml b00t-mcp/src/server_llm.rs
git commit -m "feat(server_llm): add Windows Foundry Local as a discovered local backend"
```

---

## Final Verification Checklist (for the validating/committing agent)

Run these against `/mnt/d/promptjects/_b00t_` / `D:\promptjects\_b00t_` on the feature branch, after all 3 tasks are complete, before opening the PR. Every `cargo` command goes through `pwsh.exe -NoProfile -Command "cd D:\promptjects\_b00t_; <command>"` with `-j 2` and scoped to `-p b00t-mcp`; `git`/`grep` run directly from WSL:

- [ ] `cargo test -p b00t-mcp -j 2 server_llm:: -- --nocapture` — all 16 tests pass (9 pre-existing + 7 new across the 3 tasks).
- [ ] `cargo build -p b00t-mcp -j 2` — no errors. Check for new warnings specifically in `server_llm.rs` (pre-existing warnings elsewhere in the crate, seen during this session's earlier verification build, are not this plan's concern).
- [ ] `cargo fmt --check -p b00t-mcp` — passes (if it fails on whitespace-only drift in code this plan touched, run `cargo fmt -p b00t-mcp`, verify via `git diff` the change is formatting-only, re-run the test suite, then include the formatting fix in the final commit or a small follow-up commit).
- [ ] Confirm `LlmState::from_config` and `LlmState::from_config_full`'s signatures are byte-identical to before this plan — `git diff main -- b00t-mcp/src/server_llm.rs | grep -A3 "pub fn from_config"` should show no signature-line changes (only the new `from_config_full_with_concurrency` function should be new).
- [ ] Confirm `vendor/ledgrrr` is NOT part of the diff (`git status --porcelain` should not list it, or if it appears as a submodule pointer change, it must not be staged/committed).
- [ ] Confirm the root `Cargo.toml`'s `ufo-types` rev bump is the only change to that file (`git diff main -- Cargo.toml` should show exactly one changed line).
- [ ] Open the PR against `elasticdotventures/_b00t_` `main`, referencing and closing issue #1199 (`Closes #1199` in the PR body). Also mention it fulfills `#1182` in the body (do not close #1182 automatically unless you're confident this PR's scope fully covers what #1182 asked for — check its text first).
- [ ] Do not touch `ledgrrr` (including anything under `vendor/ledgrrr`) or `ufo-types` in this PR — those are separate repos/phases.
