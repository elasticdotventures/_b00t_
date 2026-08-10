# Design: ScopeStore Global-Scope Harmonization (Transactions + kv_store.rs Facade)

**Status**: Proposed
**Related**: #893 (ScopeStore umbrella), #894 (`ScopeStore` trait object-safety), #897
(generation token — this design implements it), #902 (concurrency ADR, see
`docs/architecture/SCOPESTORE_CONCURRENCY_ADR.md`)

## Context

`b00t-c0re-gov/src/scope_store.rs` already defines the trait this codebase needs for a
sovereign, company-wide store of truth: `ScopeId::{Repo, Node, Global}` with explicit
repo → node → global resolution. Two backends exist — `RedbScopeStore` (local, embedded,
fail-fast on a second writer process) and `RedisScopeStore` (distributed, explicitly a
stub: last-write-wins, no ordering guarantee, per ADR #902).

Separately, `b00t-c0re-lib/src/kv_store.rs::KvStore` is an older, independent KV
abstraction (backend priority Valkey > Redis > ForgeKV > File) used by
`b00t-cli/src/commands/redis.rs`'s `agent_kv` (register/broadcast agent presence) and
`session_kv` (session storage) modules — single-node (`localhost:6379`) only.
`scope_store.rs`'s own module doc already flags this as duplication to consolidate, not
reinvent.

There is no Upstash usage anywhere in this codebase (verified 2026-08-10 — the only
"upstash" reference in either `promptexecution` or `.b00t` is the unrelated
`@upstash/context7-mcp` docs-lookup MCP package). This design does not introduce Upstash
or any managed vendor; it stays sovereign-only. `RedisScopeStore` is built on a plain
Redis connection URL, so a managed Redis-protocol endpoint remains a config-level swap
later if ever wanted — no code in this design depends on that.

## Goals

1. Give `Global` scope a real atomic multi-key transaction primitive, so application
   state machines can transition state and append an audit/log entry as one all-or-nothing
   operation — the actual motivating use case, and the substance of #897.
2. Harmonize `kv_store.rs`'s `agent_kv`/`session_kv` onto `ScopeStore::Global`, keeping
   their existing names and call sites permanently as an idiomatic naming layer (not a
   temporary shim to delete later — the naming convention itself is useful).
3. Stay sovereign-default: one self-hosted Valkey instance (or small non-sharded HA
   pair) as the single source of truth. Many agent processes, wherever they run, talk to
   that one instance over the network.

## Non-goals

- **Multi-region / multi-cloud replication.** Out of scope. This design assumes a single
  authoritative Valkey instance; clients may be anywhere, the data is not.
- **`pipeline_store_nats.rs` and `soul.rs`'s `/v1/kv` HTTP surface.** Real duplication,
  explicitly deferred to a follow-up design, not folded in here.
- **Cross-scope transactions** (e.g. one atomic write spanning a `Node` key and a
  `Global` key). Batches are bounded to a single `ScopeId`.
- **Solving redb's cross-process `DatabaseAlreadyOpen` failure mode.** Still one
  writer-process per scope file; this design does not change that.
- **Upstash or any managed-vendor adapter.** Not built here; the trait's existing
  backend-transparency leaves it possible later without touching callers.

## Architecture

`ScopeId::Global`, backed by `RedisScopeStore` pointed at one sovereign Valkey instance,
becomes the company-wide store of truth. A new `TransactionalScopeStore` supertrait sits
on top of the existing, unchanged `ScopeStore` trait (kept minimal deliberately — see
#894's object-safety concerns already on record):

```rust
pub trait TransactionalScopeStore: ScopeStore {
    fn transaction(&mut self, ops: Vec<ScopeOp>) -> ScopeResult<Vec<ScopeOpResult>>;
}

pub enum ScopeOp {
    Get { key: String },
    Put { key: String, value: Value, expect_gen: Option<u64> },
    Delete { key: String, expect_gen: Option<u64> },
}

pub enum ScopeOpResult {
    Value { value: Option<Value>, gen: u64 },
    Written { gen: u64 },
    Deleted,
}
```

`RedbScopeStore` implements it via one `begin_write` transaction (redb already
serializes writers within a process — see ADR #902). `RedisScopeStore` implements it via
a single Lua `EVAL` script, so Valkey applies the whole batch with no interleaving.

Values written through the transactional path are wrapped in an envelope:

```json
{"v": <value>, "gen": <u64>, "expires_at": "<RFC3339, optional>"}
```

`gen` is the per-key generation, checked against `expect_gen` for CAS semantics inside a
batch. `expires_at` gives TTL: checked lazily on every read (envelope past its
`expires_at` reads as absent), and mirrored to a native Redis `EXPIRE` on the underlying
key for actual memory eviction — belt and suspenders, since redb has no native TTL and
needs the lazy check regardless.

`kv_store.rs`'s `agent_kv::register_agent`/`get_agent_status`/`broadcast` and
`session_kv::store_session`/`get_session`/`clear_session` keep their current signatures
and call sites. Internally, `register_agent`/`store_session` become a single `Put` via
`transaction()` (TTL from `expires_at`, not the old `SETEX`); `broadcast`'s pub/sub stays
on the existing Redis pub/sub path unchanged — pub/sub is not a KV concern and does not
move to `ScopeStore`.

## Data flow

- `register_agent(id, status)` → `ScopeOp::Put{key: "b00t:agents:{id}", value: status,
  expect_gen: None}` → `Global.transaction()` → Lua script applies it, sets `gen=1` (or
  bumps), sets `EXPIRE 300` → returns `gen`.
- A state-machine transition: caller issues
  `ScopeOp::Put{key: "sm:{id}:state", value: Y, expect_gen: Some(current_gen)}` +
  `ScopeOp::Put{key: "sm:{id}:log", value: ..., expect_gen: None}` in one
  `transaction()` call. Either both land or neither does.

## Error handling

A CAS mismatch (`expect_gen` doesn't match the stored `gen`) surfaces as
`ScopeError::WriteRejected` **identically on both backends** — this is the concrete fix
for the gap ADR #902 named: "nothing in the trait itself says [the consistency model
is interchangeable]." Callers respond to `WriteRejected` by re-reading and recomputing,
never by waiting on a lock — per the ADR's explicit warning against porting redb-shaped
retry policy onto the redis backend. `ScopeError::BackendUnavailable` remains
transient/retryable on both, unchanged.

Redb's separate, still-open cross-process `DatabaseAlreadyOpen` failure mode (a second
*process* opening the same scope file for writing) is untouched by this design — it
remains a named non-goal, not silently absorbed into the new CAS error path.

## Migration

Hard cutover, no legacy bare-value compatibility shim: `agent_kv`/`session_kv` data is
short-TTL by nature (5 minutes / 1 hour today) and disposable — old bare (pre-envelope)
Redis keys are simply left to expire out naturally after deploy. No dual-read code path.

## Testing

- A single `ScopeOp` batch test suite (CAS success, CAS mismatch → `WriteRejected`,
  unconditional put, TTL expiry) run against **both** `RedbScopeStore` and a real (or
  testcontainer) Valkey via `RedisScopeStore` — this is the regression test that closes
  ADR #902's gap: proving the two backends now actually agree on transaction outcomes,
  not just trait-level type compatibility.
- `agent_kv`/`session_kv` facade tests updated to run against real Valkey instead of
  today's file-fallback, asserting identical external behavior at the old call sites.
