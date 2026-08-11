# ScopeStore Global-Scope Harmonization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give `ScopeId::Global` an atomic multi-key transaction primitive (`TransactionalScopeStore`), implemented identically on `RedbScopeStore` and `RedisScopeStore`, and rewire `b00t-cli`'s `agent_kv`/`session_kv` facade to use it.

**Architecture:** Add an additive `TransactionalScopeStore` supertrait (base `ScopeStore` untouched) with a `ScopeEnvelope{v, gen, expires_at}` wire format shared by both backends. `RedbScopeStore` implements it with one `begin_write`; `RedisScopeStore` implements it with one Lua `EVAL` via a new `RedisComms::eval_script` primitive. CAS mismatch returns `ScopeError::WriteRejected` identically on both backends — the parity test suite proves this.

**Tech Stack:** Rust, redb 4.1.0, redis crate 0.32.7 (`redis::cmd("EVAL")`, no new feature flags), chrono, serde_json.

## Global Constraints

- Base `ScopeStore` trait (`get_raw`/`set_raw`/`scope_id`/`parent`) is NOT modified — `TransactionalScopeStore: ScopeStore` is additive only.
- Transactions are bounded to a single `ScopeId` (no cross-scope atomicity).
- No legacy bare-value compatibility code — hard cutover, old un-enveloped keys just expire.
- No multi-region/multi-cloud replication, no `pipeline_store_nats.rs` changes, no `soul.rs` HTTP surface changes, no Upstash/managed-vendor adapter.
- `agent_kv`/`session_kv` in `b00t-cli/src/commands/redis.rs` keep their existing public signatures permanently — this is a facade, not a shim to delete.
- Run `cargo test -p b00t-c0re-gov -p b00t-c0re-lib -p b00t-cli` before considering the plan done; every existing test must still pass.

---

### Task 1: `RedisComms::eval_script` — raw Lua EVAL primitive

**Files:**
- Modify: `b00t-c0re-lib/src/redis.rs` (add method to the existing `impl RedisComms` block, alongside `set`/`get`/`expire`)

**Interfaces:**
- Produces: `RedisComms::eval_script(&self, script: &str, keys: &[String], args: &[String]) -> B00tResult<String>`

- [ ] **Step 1: Write the failing test**

Add to `b00t-c0re-lib/src/redis.rs`'s existing `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn test_eval_script_echoes_keys_and_args() {
        let config = RedisConfig::default();
        let comms = RedisComms::new(config, "test-agent".to_string()).unwrap();
        if !comms.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        let result = comms
            .eval_script(
                "return KEYS[1] .. ':' .. ARGV[1]",
                &["k1".to_string()],
                &["a1".to_string()],
            )
            .unwrap();
        assert_eq!(result, "k1:a1");
    }
```

- [ ] **Step 2: Run test to verify it fails to compile**

Run: `cd b00t-c0re-lib && cargo test test_eval_script_echoes_keys_and_args 2>&1 | tail -20`
Expected: FAIL — `eval_script` not found on `RedisComms`.

- [ ] **Step 3: Implement `eval_script`**

Add to the `impl RedisComms` block in `b00t-c0re-lib/src/redis.rs`, near `expire`:

```rust
    /// Execute a Lua script with the given KEYS and ARGV, returning the raw
    /// string result (caller parses further, e.g. as JSON). Uses a plain
    /// `EVAL` command rather than `redis::Script` so no new crate feature
    /// flag is needed — matches this file's existing `redis::cmd(...)` style.
    pub fn eval_script(&self, script: &str, keys: &[String], args: &[String]) -> B00tResult<String> {
        let mut conn = self.get_connection()?;
        let mut cmd = redis::cmd("EVAL");
        cmd.arg(script).arg(keys.len());
        for k in keys {
            cmd.arg(k);
        }
        for a in args {
            cmd.arg(a);
        }
        cmd.query(&mut conn).context("Failed to EVAL script on Redis")
    }
```

- [ ] **Step 4: Run test to verify it passes (or skips cleanly without Redis)**

Run: `cd b00t-c0re-lib && cargo test test_eval_script_echoes_keys_and_args -- --nocapture 2>&1 | tail -20`
Expected: PASS, or "skipping: no Redis reachable in this environment" printed and test still reports PASS (early `return` inside a `#[test]` is a pass, not a skip marker — this matches the existing `get_set_round_trips_when_redis_is_actually_available` pattern in `redis_scope_store.rs`).

- [ ] **Step 5: Full crate build check**

Run: `cd b00t-c0re-lib && cargo build 2>&1 | tail -30`
Expected: builds clean, no warnings about unused imports.

- [ ] **Step 6: Commit**

```bash
git add b00t-c0re-lib/src/redis.rs
git commit -m "feat(redis): add RedisComms::eval_script Lua EVAL primitive"
```

---

### Task 2: `TransactionalScopeStore` trait + `ScopeOp`/`ScopeOpResult`/`ScopeEnvelope`

**Files:**
- Modify: `b00t-c0re-gov/src/scope_store.rs`

**Interfaces:**
- Consumes: `ScopeError` from `crate::errors` (already imported as `ScopeResult`; add `ScopeError` too).
- Produces (used by Tasks 3, 4, 6):
  - `pub enum ScopeOp { Get { key: String }, Put { key: String, value: Value, expect_gen: Option<u64>, expires_at: Option<DateTime<Utc>> }, Delete { key: String, expect_gen: Option<u64> } }`
  - `pub enum ScopeOpResult { Value { value: Option<Value>, gen: u64 }, Written { gen: u64 }, Deleted }`
  - `pub struct ScopeEnvelope { pub v: Value, pub gen: u64, pub expires_at: Option<DateTime<Utc>> }` with `pub fn is_expired(&self, now: DateTime<Utc>) -> bool`
  - `pub trait TransactionalScopeStore: ScopeStore { fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>>; }`

- [ ] **Step 1: Write the failing test**

Add to `b00t-c0re-gov/src/scope_store.rs`'s `#[cfg(test)] mod tests`, using the existing `MemScopeStore` extended with a manual `TransactionalScopeStore` impl to prove the trait compiles and is usable:

```rust
    impl TransactionalScopeStore for MemScopeStore {
        fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>> {
            let now = Utc::now();
            // Pass 1: CAS pre-check.
            for op in &ops {
                let (key, expect_gen) = match op {
                    ScopeOp::Put { key, expect_gen, .. } => (key, *expect_gen),
                    ScopeOp::Delete { key, expect_gen } => (key, *expect_gen),
                    ScopeOp::Get { .. } => continue,
                };
                if let Some(expected) = expect_gen {
                    let current_gen = self
                        .data
                        .get(key)
                        .and_then(|v| serde_json::from_value::<ScopeEnvelope>(v.clone()).ok())
                        .map(|e| e.gen)
                        .unwrap_or(0);
                    if current_gen != expected {
                        return Err(ScopeError::WriteRejected(format!(
                            "CAS mismatch on {key}: expected gen {expected}, found {current_gen}"
                        )));
                    }
                }
            }
            // Pass 2: apply.
            let mut results = Vec::with_capacity(ops.len());
            for op in ops {
                match op {
                    ScopeOp::Get { key } => {
                        let env = self
                            .data
                            .get(&key)
                            .and_then(|v| serde_json::from_value::<ScopeEnvelope>(v.clone()).ok())
                            .filter(|e| !e.is_expired(now));
                        results.push(ScopeOpResult::Value {
                            value: env.as_ref().map(|e| e.v.clone()),
                            gen: env.map(|e| e.gen).unwrap_or(0),
                        });
                    }
                    ScopeOp::Put { key, value, expires_at, .. } => {
                        let current_gen = self
                            .data
                            .get(&key)
                            .and_then(|v| serde_json::from_value::<ScopeEnvelope>(v.clone()).ok())
                            .map(|e| e.gen)
                            .unwrap_or(0);
                        let new_gen = current_gen + 1;
                        let env = ScopeEnvelope { v: value, gen: new_gen, expires_at };
                        self.data.insert(key, serde_json::to_value(&env).unwrap());
                        results.push(ScopeOpResult::Written { gen: new_gen });
                    }
                    ScopeOp::Delete { key, .. } => {
                        self.data.remove(&key);
                        results.push(ScopeOpResult::Deleted);
                    }
                }
            }
            Ok(results)
        }
    }

    #[test]
    fn transaction_put_then_get_round_trips_with_generation() {
        let mut store = MemScopeStore { id: ScopeId::Global, parent: None, data: HashMap::new() };
        let results = store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Written { gen: 1 }]);

        let results = store.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(
            results,
            vec![ScopeOpResult::Value { value: Some(Value::String("v1".into())), gen: 1 }]
        );
    }

    #[test]
    fn transaction_cas_mismatch_rejects_whole_batch() {
        let mut store = MemScopeStore { id: ScopeId::Global, parent: None, data: HashMap::new() };
        store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();

        let err = store
            .transaction(vec![
                ScopeOp::Put {
                    key: "k".into(),
                    value: Value::String("v2".into()),
                    expect_gen: Some(99), // wrong — actual gen is 1
                    expires_at: None,
                },
                ScopeOp::Put {
                    key: "other".into(),
                    value: Value::String("should-not-land".into()),
                    expect_gen: None,
                    expires_at: None,
                },
            ])
            .unwrap_err();
        assert!(matches!(err, ScopeError::WriteRejected(_)));
        // Second op in the batch must NOT have landed — all-or-nothing.
        assert_eq!(store.get_raw("other").unwrap(), None);
    }

    #[test]
    fn transaction_multi_key_batch_is_atomic_on_success() {
        let mut store = MemScopeStore { id: ScopeId::Global, parent: None, data: HashMap::new() };
        let results = store
            .transaction(vec![
                ScopeOp::Put {
                    key: "sm:state".into(),
                    value: Value::String("Y".into()),
                    expect_gen: None,
                    expires_at: None,
                },
                ScopeOp::Put {
                    key: "sm:log".into(),
                    value: Value::String("transitioned to Y".into()),
                    expect_gen: None,
                    expires_at: None,
                },
            ])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Written { gen: 1 }, ScopeOpResult::Written { gen: 1 }]);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd b00t-c0re-gov && cargo test scope_store:: 2>&1 | tail -30`
Expected: FAIL to compile — `ScopeOp`, `ScopeOpResult`, `ScopeEnvelope`, `TransactionalScopeStore` don't exist yet.

- [ ] **Step 3: Implement the types and trait**

Add near the top of `b00t-c0re-gov/src/scope_store.rs`, after the existing `use` block (add `chrono::{DateTime, Utc}` to imports; add `ScopeError` to the existing `use crate::errors::ScopeResult;` line so it reads `use crate::errors::{ScopeError, ScopeResult};`):

```rust
/// Envelope wrapping every value written through `TransactionalScopeStore` —
/// carries the generation used for CAS and an optional lazy-checked expiry.
/// Shared by every backend so they can never silently diverge on wire
/// format (the exact interchangeability gap ADR #902 flags for `set_raw`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeEnvelope {
    pub v: Value,
    pub gen: u64,
    pub expires_at: Option<DateTime<Utc>>,
}

impl ScopeEnvelope {
    /// True when `now` is at or past this envelope's expiry, if it has one.
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        self.expires_at.map(|exp| exp <= now).unwrap_or(false)
    }
}

/// A single operation within a `TransactionalScopeStore::transaction()` batch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScopeOp {
    Get {
        key: String,
    },
    Put {
        key: String,
        value: Value,
        /// CAS guard: fail the whole batch if the key's current generation
        /// doesn't match. `None` means unconditional write.
        expect_gen: Option<u64>,
        expires_at: Option<DateTime<Utc>>,
    },
    Delete {
        key: String,
        expect_gen: Option<u64>,
    },
}

/// Result of one `ScopeOp` within a successful transaction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScopeOpResult {
    Value { value: Option<Value>, gen: u64 },
    Written { gen: u64 },
    Deleted,
}

/// Additive capability on top of `ScopeStore`: atomic, same-scope,
/// multi-key batches with per-key CAS. Not every `ScopeStore` needs to
/// implement this — most `Repo`/`Node` callers use plain `get_raw`/`set_raw`.
/// Kept as a separate trait (rather than widening `ScopeStore` itself) per
/// #894's existing object-safety caution.
///
/// A CAS mismatch anywhere in the batch aborts the ENTIRE batch — no
/// partial writes — and returns `ScopeError::WriteRejected`, identically on
/// every backend (this is #897 and the concrete fix for ADR #902's gap:
/// "nothing in the trait says the consistency model is interchangeable").
pub trait TransactionalScopeStore: ScopeStore {
    fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>>;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd b00t-c0re-gov && cargo test scope_store:: 2>&1 | tail -40`
Expected: PASS — all 4 new tests plus the existing 5 `scope_store::tests::*` tests green.

- [ ] **Step 5: Commit**

```bash
git add b00t-c0re-gov/src/scope_store.rs
git commit -m "feat(scope-store): add TransactionalScopeStore trait, ScopeOp/ScopeOpResult, ScopeEnvelope"
```

---

### Task 3: `RedbScopeStore::transaction`

**Files:**
- Modify: `b00t-c0re-gov/src/redb_scope_store.rs`

**Interfaces:**
- Consumes: `TransactionalScopeStore`, `ScopeOp`, `ScopeOpResult`, `ScopeEnvelope` from Task 2 (`crate::scope_store::{..., ScopeOp, ScopeOpResult, ScopeEnvelope, TransactionalScopeStore}`).
- Produces: `impl TransactionalScopeStore for RedbScopeStore`.

- [ ] **Step 1: Write the failing tests**

Add to `b00t-c0re-gov/src/redb_scope_store.rs`'s `#[cfg(test)] mod tests`:

```rust
    use crate::scope_store::{ScopeEnvelope, ScopeOp, ScopeOpResult, TransactionalScopeStore};
    use chrono::Utc;

    #[test]
    fn transaction_put_then_get_round_trips_with_generation() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();

        let results = store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Written { gen: 1 }]);

        let results = store.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(
            results,
            vec![ScopeOpResult::Value { value: Some(Value::String("v1".into())), gen: 1 }]
        );
    }

    #[test]
    fn transaction_cas_mismatch_rejects_whole_batch_and_persists_nothing() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
        store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();

        let err = store
            .transaction(vec![
                ScopeOp::Put {
                    key: "k".into(),
                    value: Value::String("v2".into()),
                    expect_gen: Some(99),
                    expires_at: None,
                },
                ScopeOp::Put {
                    key: "other".into(),
                    value: Value::String("should-not-land".into()),
                    expect_gen: None,
                    expires_at: None,
                },
            ])
            .unwrap_err();
        assert!(matches!(err, ScopeError::WriteRejected(_)));
        assert_eq!(store.get_raw("other").unwrap(), None);
        // Original value untouched.
        let results = store.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(
            results,
            vec![ScopeOpResult::Value { value: Some(Value::String("v1".into())), gen: 1 }]
        );
    }

    #[test]
    fn transaction_expired_value_reads_as_absent() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
        store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: Some(Utc::now() - chrono::Duration::seconds(1)), // already expired
            }])
            .unwrap();

        let results = store.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(results, vec![ScopeOpResult::Value { value: None, gen: 0 }]);
    }

    #[test]
    fn transaction_delete_removes_key() {
        let dir = tempdir().unwrap();
        let mut store =
            RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
        store
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();
        let results = store
            .transaction(vec![ScopeOp::Delete { key: "k".into(), expect_gen: None }])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Deleted]);
        assert_eq!(store.get_raw("k").unwrap(), None);
    }
```

(`ScopeEnvelope` import above is used by the helper in Step 3, not directly by the tests — keep the `use` as written since Step 3's helper lives in the same module and the compiler will flag it unused only if Step 3 is skipped; implement Step 3 first if your toolchain checks per-block.)

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd b00t-c0re-gov && cargo test redb_scope_store:: 2>&1 | tail -30`
Expected: FAIL to compile — no `TransactionalScopeStore` impl for `RedbScopeStore`.

- [ ] **Step 3: Implement `TransactionalScopeStore for RedbScopeStore`**

Add to `b00t-c0re-gov/src/redb_scope_store.rs`, after the existing `impl ScopeStore for RedbScopeStore` block. Update the `use` block at the top of the file to:

```rust
use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::{ScopeEnvelope, ScopeId, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use chrono::Utc;
use redb::{Database, ReadableDatabase, ReadableTable, Table, TableDefinition};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
```

Then add a private helper and the trait impl:

```rust
fn read_envelope(
    table: &impl ReadableTable<&'static str, &'static str>,
    key: &str,
) -> ScopeResult<Option<ScopeEnvelope>> {
    let Some(guard) = table
        .get(key)
        .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?
    else {
        return Ok(None);
    };
    let env: ScopeEnvelope = serde_json::from_str(guard.value())?;
    Ok(Some(env))
}

impl TransactionalScopeStore for RedbScopeStore {
    fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>> {
        let now = Utc::now();
        let txn = self
            .db
            .begin_write()
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        let mut results = Vec::with_capacity(ops.len());
        {
            let mut table: Table<&str, &str> = txn
                .open_table(KV_TABLE)
                .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;

            // Pass 1: CAS pre-check — abort before any write on mismatch.
            for op in &ops {
                let (key, expect_gen) = match op {
                    ScopeOp::Put { key, expect_gen, .. } => (key, *expect_gen),
                    ScopeOp::Delete { key, expect_gen } => (key, *expect_gen),
                    ScopeOp::Get { .. } => continue,
                };
                if let Some(expected) = expect_gen {
                    let current_gen = read_envelope(&table, key)?.map(|e| e.gen).unwrap_or(0);
                    if current_gen != expected {
                        return Err(ScopeError::WriteRejected(format!(
                            "CAS mismatch on {key}: expected gen {expected}, found {current_gen}"
                        )));
                    }
                }
            }

            // Pass 2: apply.
            for op in ops {
                match op {
                    ScopeOp::Get { key } => {
                        let env = read_envelope(&table, &key)?.filter(|e| !e.is_expired(now));
                        results.push(ScopeOpResult::Value {
                            value: env.as_ref().map(|e| e.v.clone()),
                            gen: env.map(|e| e.gen).unwrap_or(0),
                        });
                    }
                    ScopeOp::Put { key, value, expires_at, .. } => {
                        let current_gen = read_envelope(&table, &key)?.map(|e| e.gen).unwrap_or(0);
                        let new_gen = current_gen + 1;
                        let env = ScopeEnvelope { v: value, gen: new_gen, expires_at };
                        let serialized = serde_json::to_string(&env)?;
                        table
                            .insert(key.as_str(), serialized.as_str())
                            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
                        results.push(ScopeOpResult::Written { gen: new_gen });
                    }
                    ScopeOp::Delete { key, .. } => {
                        table
                            .remove(key.as_str())
                            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
                        results.push(ScopeOpResult::Deleted);
                    }
                }
            }
        }
        txn.commit()
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        Ok(results)
    }
}
```

Note: `get_raw`'s existing implementation opens its own read-only table via `begin_read`/`TableDoesNotExist` handling — that path is untouched. The `read_envelope` helper here is new, used only inside `transaction`'s write-table.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd b00t-c0re-gov && cargo test redb_scope_store:: 2>&1 | tail -60`
Expected: PASS — the 4 new tests plus all existing `redb_scope_store::tests::*` tests green.

- [ ] **Step 5: Commit**

```bash
git add b00t-c0re-gov/src/redb_scope_store.rs
git commit -m "feat(redb-scope-store): implement TransactionalScopeStore via begin_write"
```

---

### Task 4: `RedisScopeStore::transaction` via Lua `EVAL`

**Files:**
- Modify: `b00t-c0re-gov/src/redis_scope_store.rs`

**Interfaces:**
- Consumes: `RedisComms::eval_script` (Task 1), `TransactionalScopeStore`/`ScopeOp`/`ScopeOpResult`/`ScopeEnvelope` (Task 2).
- Produces: `impl TransactionalScopeStore for RedisScopeStore`.

- [ ] **Step 1: Write the failing tests**

Add to `b00t-c0re-gov/src/redis_scope_store.rs`'s `#[cfg(test)] mod tests`:

```rust
    use crate::scope_store::{ScopeOp, ScopeOpResult, TransactionalScopeStore};

    #[test]
    fn transaction_put_then_get_round_trips_when_redis_is_actually_available() {
        let mut s = store();
        if !s.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        let results = s
            .transaction(vec![ScopeOp::Put {
                key: "k".into(),
                value: Value::String("v1".into()),
                expect_gen: None,
                expires_at: None,
            }])
            .unwrap();
        assert_eq!(results, vec![ScopeOpResult::Written { gen: 1 }]);

        let results = s.transaction(vec![ScopeOp::Get { key: "k".into() }]).unwrap();
        assert_eq!(
            results,
            vec![ScopeOpResult::Value { value: Some(Value::String("v1".into())), gen: 1 }]
        );
    }

    #[test]
    fn transaction_cas_mismatch_rejects_whole_batch_when_redis_is_actually_available() {
        let mut s = store();
        if !s.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        s.transaction(vec![ScopeOp::Put {
            key: "cas-test".into(),
            value: Value::String("v1".into()),
            expect_gen: None,
            expires_at: None,
        }])
        .unwrap();

        let err = s
            .transaction(vec![
                ScopeOp::Put {
                    key: "cas-test".into(),
                    value: Value::String("v2".into()),
                    expect_gen: Some(99),
                    expires_at: None,
                },
                ScopeOp::Put {
                    key: "cas-test-other".into(),
                    value: Value::String("should-not-land".into()),
                    expect_gen: None,
                    expires_at: None,
                },
            ])
            .unwrap_err();
        assert!(matches!(err, ScopeError::WriteRejected(_)));
        assert_eq!(s.get_raw("cas-test-other").unwrap(), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd b00t-c0re-gov && cargo test redis_scope_store:: 2>&1 | tail -30`
Expected: FAIL to compile — no `TransactionalScopeStore` impl for `RedisScopeStore`.

- [ ] **Step 3: Implement `TransactionalScopeStore for RedisScopeStore`**

Update the `use` block at the top of `b00t-c0re-gov/src/redis_scope_store.rs`:

```rust
use crate::errors::{ScopeError, ScopeResult};
use crate::scope_store::{ScopeId, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use b00t_c0re_lib::redis::{RedisComms, RedisConfig};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
```

Add the Lua script as a module-level constant and DTOs for its JSON output, then the trait impl, all in `redis_scope_store.rs`:

```rust
/// Applies a `ScopeOp` batch atomically: CAS pre-check pass (aborts the
/// whole call on any mismatch, before any write), then an apply pass.
/// KEYS[i] pairs positionally with the i-th entry of the ARGV[1] JSON op
/// array; ARGV[2] is the caller's `now` as Unix seconds (avoids relying on
/// Lua-internal TIME for determinism across Redis/Valkey configurations).
const TRANSACTION_SCRIPT: &str = r#"
local ops = cjson.decode(ARGV[1])
local now = tonumber(ARGV[2])

for i, op in ipairs(ops) do
  if op.expect_gen ~= nil then
    local raw = redis.call('GET', KEYS[i])
    local current_gen = 0
    if raw then
      current_gen = cjson.decode(raw).gen
    end
    if current_gen ~= op.expect_gen then
      return cjson.encode({error = 'cas_mismatch', key = KEYS[i], expected = op.expect_gen, found = current_gen})
    end
  end
end

local results = {}
for i, op in ipairs(ops) do
  if op.op == 'get' then
    local raw = redis.call('GET', KEYS[i])
    if raw then
      local env = cjson.decode(raw)
      if env.expires_at ~= nil and env.expires_at <= now then
        results[i] = {type = 'value', gen = 0}
      else
        results[i] = {type = 'value', value = env.v, gen = env.gen}
      end
    else
      results[i] = {type = 'value', gen = 0}
    end
  elseif op.op == 'put' then
    local raw = redis.call('GET', KEYS[i])
    local current_gen = 0
    if raw then
      current_gen = cjson.decode(raw).gen
    end
    local new_gen = current_gen + 1
    local env = {v = op.value, gen = new_gen, expires_at = op.expires_at}
    redis.call('SET', KEYS[i], cjson.encode(env))
    if op.expires_at ~= nil then
      local ttl = math.floor(op.expires_at - now)
      if ttl > 0 then
        redis.call('EXPIRE', KEYS[i], ttl)
      end
    end
    results[i] = {type = 'written', gen = new_gen}
  elseif op.op == 'delete' then
    redis.call('DEL', KEYS[i])
    results[i] = {type = 'deleted'}
  end
end

return cjson.encode({ok = results})
"#;

#[derive(Serialize)]
struct LuaOp {
    op: &'static str,
    expect_gen: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
}

#[derive(Deserialize)]
struct LuaOpResult {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    value: Option<Value>,
    #[serde(default)]
    gen: u64,
}

#[derive(Deserialize)]
struct LuaScriptOutput {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    key: Option<String>,
    #[serde(default)]
    expected: Option<u64>,
    #[serde(default)]
    found: Option<u64>,
    #[serde(default)]
    ok: Option<Vec<LuaOpResult>>,
}

impl TransactionalScopeStore for RedisScopeStore {
    fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>> {
        let now = Utc::now();
        let mut keys = Vec::with_capacity(ops.len());
        let mut lua_ops = Vec::with_capacity(ops.len());

        for op in &ops {
            match op {
                ScopeOp::Get { key } => {
                    keys.push(self.scoped_key(key));
                    lua_ops.push(LuaOp { op: "get", expect_gen: None, value: None, expires_at: None });
                }
                ScopeOp::Put { key, value, expect_gen, expires_at } => {
                    keys.push(self.scoped_key(key));
                    lua_ops.push(LuaOp {
                        op: "put",
                        expect_gen: *expect_gen,
                        value: Some(value.clone()),
                        expires_at: expires_at.map(|e| e.timestamp()),
                    });
                }
                ScopeOp::Delete { key, expect_gen } => {
                    keys.push(self.scoped_key(key));
                    lua_ops.push(LuaOp { op: "delete", expect_gen: *expect_gen, value: None, expires_at: None });
                }
            }
        }

        let ops_json = serde_json::to_string(&lua_ops)?;
        let raw = self
            .comms
            .eval_script(TRANSACTION_SCRIPT, &keys, &[ops_json, now.timestamp().to_string()])
            .map_err(|e| ScopeError::BackendUnavailable(e.to_string()))?;
        let output: LuaScriptOutput = serde_json::from_str(&raw)?;

        if let Some(err) = output.error {
            if err == "cas_mismatch" {
                return Err(ScopeError::WriteRejected(format!(
                    "CAS mismatch on {}: expected gen {:?}, found {:?}",
                    output.key.unwrap_or_default(),
                    output.expected,
                    output.found
                )));
            }
            return Err(ScopeError::BackendUnavailable(format!("transaction script error: {err}")));
        }

        let raw_results = output.ok.unwrap_or_default();
        let mut results = Vec::with_capacity(raw_results.len());
        for (i, r) in raw_results.into_iter().enumerate() {
            let result = match r.kind.as_str() {
                "value" => ScopeOpResult::Value { value: r.value, gen: r.gen },
                "written" => ScopeOpResult::Written { gen: r.gen },
                "deleted" => ScopeOpResult::Deleted,
                other => {
                    return Err(ScopeError::BackendUnavailable(format!(
                        "unrecognized transaction result kind '{other}' at index {i}"
                    )));
                }
            };
            results.push(result);
        }
        Ok(results)
    }
}
```

- [ ] **Step 4: Run tests to verify they pass (or skip cleanly without Redis)**

Run: `cd b00t-c0re-gov && cargo test redis_scope_store:: -- --nocapture 2>&1 | tail -60`
Expected: PASS — either real round-trips against a reachable Valkey/Redis, or clean "skipping: no Redis reachable" prints with the tests still reporting PASS, matching this file's existing `get_set_round_trips_when_redis_is_actually_available` pattern.

- [ ] **Step 5: Full crate build check**

Run: `cd b00t-c0re-gov && cargo build 2>&1 | tail -30 && cargo clippy --all-targets 2>&1 | tail -40`
Expected: builds clean; address any clippy warnings on the new code (not pre-existing ones).

- [ ] **Step 6: Commit**

```bash
git add b00t-c0re-gov/src/redis_scope_store.rs
git commit -m "feat(redis-scope-store): implement TransactionalScopeStore via Lua EVAL"
```

---

### Task 5: Cross-backend parity test — the actual regression test for ADR #902

**Files:**
- Create: `b00t-c0re-gov/tests/transactional_scope_store_parity.rs`

**Interfaces:**
- Consumes: `RedbScopeStore::open`, `RedisScopeStore::open`/`is_available`, `TransactionalScopeStore::transaction`, `ScopeOp`, `ScopeOpResult`, `ScopeError`, `ScopeId`, `ScopeStore::get_raw` (all from Tasks 2–4).

This is the test that proves the two backends now agree on transaction outcomes, closing the gap ADR #902 named ("nothing in the trait itself says [the consistency model is interchangeable]"). It runs the same scenario against both backends via a small helper that takes `&mut dyn TransactionalScopeStore`.

- [ ] **Step 1: Write the test file (this IS the failing-test step — the file doesn't exist yet)**

```rust
//! Cross-backend parity for `TransactionalScopeStore` — proves RedbScopeStore
//! and RedisScopeStore agree on transaction outcomes, per ADR #902
//! (docs/architecture/SCOPESTORE_CONCURRENCY_ADR.md) and issue #897.

use b00t_c0re_gov::errors::ScopeError;
use b00t_c0re_gov::redb_scope_store::RedbScopeStore;
use b00t_c0re_gov::redis_scope_store::RedisScopeStore;
use b00t_c0re_gov::scope_store::{ScopeId, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use b00t_c0re_lib::redis::RedisConfig;
use serde_json::Value;

fn cas_mismatch_rejects_whole_batch(store: &mut dyn TransactionalScopeStore) {
    store
        .transaction(vec![ScopeOp::Put {
            key: "parity:k".into(),
            value: Value::String("v1".into()),
            expect_gen: None,
            expires_at: None,
        }])
        .unwrap();

    let err = store
        .transaction(vec![
            ScopeOp::Put {
                key: "parity:k".into(),
                value: Value::String("v2".into()),
                expect_gen: Some(99),
                expires_at: None,
            },
            ScopeOp::Put {
                key: "parity:other".into(),
                value: Value::String("should-not-land".into()),
                expect_gen: None,
                expires_at: None,
            },
        ])
        .unwrap_err();

    assert!(
        matches!(err, ScopeError::WriteRejected(_)),
        "expected WriteRejected on CAS mismatch, got: {err:?}"
    );
    assert_eq!(
        store.get_raw("parity:other").unwrap(),
        None,
        "second op in a rejected batch must not have landed"
    );
}

fn successful_cas_advances_generation(store: &mut dyn TransactionalScopeStore) {
    let r1 = store
        .transaction(vec![ScopeOp::Put {
            key: "parity:gen".into(),
            value: Value::String("v1".into()),
            expect_gen: None,
            expires_at: None,
        }])
        .unwrap();
    assert_eq!(r1, vec![ScopeOpResult::Written { gen: 1 }]);

    let r2 = store
        .transaction(vec![ScopeOp::Put {
            key: "parity:gen".into(),
            value: Value::String("v2".into()),
            expect_gen: Some(1),
            expires_at: None,
        }])
        .unwrap();
    assert_eq!(r2, vec![ScopeOpResult::Written { gen: 2 }]);
}

#[test]
fn redb_cas_mismatch_rejects_whole_batch() {
    let dir = tempfile::tempdir().unwrap();
    let mut store =
        RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
    cas_mismatch_rejects_whole_batch(&mut store);
}

#[test]
fn redb_successful_cas_advances_generation() {
    let dir = tempfile::tempdir().unwrap();
    let mut store =
        RedbScopeStore::open(dir.path().join("scope.redb"), ScopeId::Global, None).unwrap();
    successful_cas_advances_generation(&mut store);
}

#[test]
fn redis_cas_mismatch_rejects_whole_batch_when_available() {
    let mut store = RedisScopeStore::open(RedisConfig::default(), ScopeId::Global, None).unwrap();
    if !store.is_available() {
        eprintln!("skipping: no Redis reachable in this environment");
        return;
    }
    cas_mismatch_rejects_whole_batch(&mut store);
}

#[test]
fn redis_successful_cas_advances_generation_when_available() {
    let mut store = RedisScopeStore::open(RedisConfig::default(), ScopeId::Global, None).unwrap();
    if !store.is_available() {
        eprintln!("skipping: no Redis reachable in this environment");
        return;
    }
    successful_cas_advances_generation(&mut store);
}
```

Note: this requires `errors`, `redb_scope_store`, `redis_scope_store`, `scope_store` to be `pub` modules of `b00t-c0re-gov` (they already are — see `lib.rs`) and requires `b00t-c0re-gov/Cargo.toml`'s `[dev-dependencies]` to include `tempfile` (already present) — no Cargo.toml change needed for this task.

- [ ] **Step 2: Run to verify it fails first (sanity — should actually pass immediately since Tasks 2–4 are done; run it anyway to confirm wiring)**

Run: `cd b00t-c0re-gov && cargo test --test transactional_scope_store_parity 2>&1 | tail -40`
Expected: PASS (all 4 tests — 2 always run against redb, 2 skip cleanly if no Redis reachable). If this fails to compile, the module visibility or an earlier task's exact type name is wrong — fix at the source of the mismatch, not by changing this file's expectations.

- [ ] **Step 3: Commit**

```bash
git add b00t-c0re-gov/tests/transactional_scope_store_parity.rs
git commit -m "test(scope-store): cross-backend transaction parity suite (closes ADR #902 gap)"
```

---

### Task 6: Harmonize `agent_kv`/`session_kv` onto `ScopeStore::Global`

**Files:**
- Modify: `b00t-cli/src/commands/redis.rs`

**Interfaces:**
- Consumes: `RedisScopeStore::open`, `TransactionalScopeStore::transaction`, `ScopeStore::get_raw`, `ScopeOp`, `ScopeOpResult`, `ScopeId` (from `b00t_c0re_gov::{redis_scope_store::RedisScopeStore, scope_store::{...}}`), `RedisConfig` (from `b00t_c0re_lib::redis`).
- Produces: unchanged public signatures for `agent_kv::{register_agent, get_agent_status, broadcast}` and `session_kv::{store_session, get_session, clear_session}` — internals only change.

This is the permanent facade Brian asked to keep (naming sugar pointing at `ScopeStore`, not a temporary shim). `agent_kv::list_agents` and `kv::{get,set,del,exists,publish}`/`get_kv_store`/`has_real_kv_backend`/`get_backend_type` are untouched — they stay on the existing `KvStore` path (out of scope per the design doc; only `agent_kv`'s register/status and `session_kv` move).

- [ ] **Step 1: Write the failing tests**

Add to `b00t-cli/src/commands/redis.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn agent_kv_register_and_get_status_round_trip_when_redis_is_actually_available() {
        let store = global_scope_store();
        if !store.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        agent_kv::register_agent("parity-test-agent", "online").unwrap();
        let status = agent_kv::get_agent_status("parity-test-agent").unwrap();
        assert_eq!(status, Some("online".to_string()));
    }

    #[test]
    fn session_kv_store_and_get_round_trip_when_redis_is_actually_available() {
        let store = global_scope_store();
        if !store.is_available() {
            eprintln!("skipping: no Redis reachable in this environment");
            return;
        }
        let mut data = HashMap::new();
        data.insert("k".to_string(), serde_json::json!("v"));
        session_kv::store_session("parity-test-session", &data).unwrap();
        let round_tripped = session_kv::get_session("parity-test-session").unwrap();
        assert_eq!(round_tripped, Some(data));

        let deleted = session_kv::clear_session("parity-test-session").unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(session_kv::get_session("parity-test-session").unwrap(), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd b00t-cli && cargo test commands::redis:: 2>&1 | tail -30`
Expected: FAIL to compile — `global_scope_store` doesn't exist yet.

- [ ] **Step 3: Implement the facade rewrite**

Replace the top of `b00t-cli/src/commands/redis.rs` (imports) with:

```rust
use anyhow::Result;
use b00t_c0re_gov::redis_scope_store::RedisScopeStore;
use b00t_c0re_gov::scope_store::{ScopeId, ScopeOp, ScopeOpResult, ScopeStore, TransactionalScopeStore};
use b00t_c0re_lib::kv_store::{KvBackend, KvConfig, KvStore};
use b00t_c0re_lib::redis::{BroadcastPriority, RedisConfig};
use chrono::{Duration, Utc};
use std::collections::HashMap;

/// The one company-wide `ScopeStore::Global` instance, backed by
/// `RedisConfig::default()` (localhost:6379) — same connection default
/// `KvConfig`/`kv_store.rs` already use. `RedisScopeStore::open` never
/// fails without a live connection (lazy client handle), matching
/// `get_kv_store()`'s always-succeeds contract below.
fn global_scope_store() -> RedisScopeStore {
    RedisScopeStore::open(RedisConfig::default(), ScopeId::Global, None)
        .expect("RedisScopeStore::open is infallible without a live connection")
}
```

(`get_kv_store`, `has_real_kv_backend`, `get_backend_type`, and `pub mod kv` stay exactly as they are — only `agent_kv` and `session_kv` change below.)

Replace the `agent_kv` module body (keep `pub mod agent_kv { use super::*; ... }` wrapper, only these three functions change — `list_agents` is untouched):

```rust
    /// Register agent status. TTL 5 minutes, same as before — now enforced
    /// via ScopeEnvelope's expires_at instead of Redis SETEX.
    pub fn register_agent(agent_id: &str, status: &str) -> Result<()> {
        let key = format!("b00t:agents:{}", agent_id);
        let mut store = global_scope_store();
        store.transaction(vec![ScopeOp::Put {
            key,
            value: serde_json::Value::String(status.to_string()),
            expect_gen: None,
            expires_at: Some(Utc::now() + Duration::seconds(300)),
        }])?;
        Ok(())
    }

    /// Get agent status.
    pub fn get_agent_status(agent_id: &str) -> Result<Option<String>> {
        let key = format!("b00t:agents:{}", agent_id);
        let store = global_scope_store();
        match store.get_raw(&key)? {
            None => Ok(None),
            Some(envelope_json) => {
                let envelope: b00t_c0re_gov::scope_store::ScopeEnvelope =
                    serde_json::from_value(envelope_json)?;
                if envelope.is_expired(Utc::now()) {
                    return Ok(None);
                }
                Ok(envelope.v.as_str().map(|s| s.to_string()))
            }
        }
    }
```

Replace the `session_kv` module body (`store_session`, `get_session`, `clear_session` — TTL and behavior unchanged, now via `ScopeStore::Global` instead of the raw `KvStore` path):

```rust
    /// Store session data. TTL 1 hour, same as before.
    pub fn store_session(
        session_id: &str,
        data: &HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let key = format!("b00t:sessions:{}", session_id);
        let value = serde_json::to_value(data)?;
        let mut store = global_scope_store();
        store.transaction(vec![ScopeOp::Put {
            key,
            value,
            expect_gen: None,
            expires_at: Some(Utc::now() + Duration::seconds(3600)),
        }])?;
        Ok(())
    }

    /// Retrieve session data.
    pub fn get_session(session_id: &str) -> Result<Option<HashMap<String, serde_json::Value>>> {
        let key = format!("b00t:sessions:{}", session_id);
        let store = global_scope_store();
        match store.get_raw(&key)? {
            None => Ok(None),
            Some(envelope_json) => {
                let envelope: b00t_c0re_gov::scope_store::ScopeEnvelope =
                    serde_json::from_value(envelope_json)?;
                if envelope.is_expired(Utc::now()) {
                    return Ok(None);
                }
                let data: HashMap<String, serde_json::Value> = serde_json::from_value(envelope.v)?;
                Ok(Some(data))
            }
        }
    }

    /// Clear session data. Returns 1 if a key was deleted, 0 if it was
    /// already absent — matches the old `kv::del` return-count contract.
    pub fn clear_session(session_id: &str) -> Result<usize> {
        let key = format!("b00t:sessions:{}", session_id);
        let mut store = global_scope_store();
        let existed = store.get_raw(&key)?.is_some();
        if !existed {
            return Ok(0);
        }
        store.transaction(vec![ScopeOp::Delete { key, expect_gen: None }])?;
        Ok(1)
    }
```

`broadcast` in `agent_kv` is unchanged (still uses `kv::publish`, pub/sub does not move — leave its existing body exactly as-is).

- [ ] **Step 4: Run tests to verify they pass (or skip cleanly without Redis)**

Run: `cd b00t-cli && cargo test commands::redis:: -- --nocapture 2>&1 | tail -60`
Expected: PASS — new tests plus the existing `test_kv_store_creation`/`test_backend_detection` tests (those two are untouched, still exercise the old `KvStore` path used by `kv::*`).

- [ ] **Step 5: Fix unused-import fallout**

Run: `cd b00t-cli && cargo build 2>&1 | tail -40`
`KvBackend`, `KvConfig`, `KvStore` are still used by `get_kv_store`/`has_real_kv_backend`/`get_backend_type`/`kv::*` — expect no unused-import warnings from those. If the compiler flags anything else unused, remove only that import, don't restructure further.

- [ ] **Step 6: Commit**

```bash
git add b00t-cli/src/commands/redis.rs
git commit -m "refactor(agent-kv): harmonize agent_kv/session_kv onto ScopeStore::Global"
```

---

### Task 7: Full workspace regression pass

**Files:** none (verification only)

- [ ] **Step 1: Run full test suites for every touched crate**

Run: `cd /home/brianh/.b00t/.claude/worktrees/task-893-global-store-harmonization && cargo test -p b00t-c0re-lib -p b00t-c0re-gov -p b00t-cli 2>&1 | tail -150`
Expected: PASS for every test — no regressions in `redis.rs` (b00t-c0re-lib), `scope_store.rs`, `redb_scope_store.rs`, `redis_scope_store.rs`, `commands/redis.rs`, `commands/redis_cli.rs`, `commands/doctor_cmd.rs`, `commands/init.rs`, or anything else in these three crates.

- [ ] **Step 2: Workspace-wide clippy check on touched crates**

Run: `cargo clippy -p b00t-c0re-lib -p b00t-c0re-gov -p b00t-cli --all-targets 2>&1 | tail -80`
Expected: no new warnings introduced by this plan's changes (pre-existing warnings in untouched code are not this plan's responsibility).

- [ ] **Step 3: Confirm design doc + plan + all 6 implementation commits are present**

Run: `git log --oneline origin/main..HEAD`
Expected: 8 commits — design doc, plan doc (this file, committed alongside or just before Task 1), and the 6 implementation commits from Tasks 1–6.

- [ ] **Step 4: Final commit if the plan doc itself wasn't yet committed**

```bash
git add docs/superpowers/plans/2026-08-10-global-store-harmonization-plan.md
git commit -m "docs: ScopeStore Global-scope harmonization implementation plan"
```

(Skip if already committed in an earlier step — check `git status --short` first.)

**Do not push or open a PR as part of this plan** — stop after the local regression pass is green and report the branch name and commit list back for review.
