# ADR: ScopeStore concurrency model — redb (v1) vs redis (v2)

**Status**: Accepted (v1 boundary only — see "What this ADR does not resolve")
**Related**: _b00t_ issues #893 (ScopeStore umbrella), #902 (this ADR's origin),
#897 (generation token, not yet implemented), #894 (`ScopeStore` trait),
`b00t-c0re-gov/src/redb_scope_store.rs`, `redis_scope_store.rs`

## Context

`ScopeStore`'s trait seam (`get_raw`/`set_raw`/`scope_id`/`parent`) is
backend-transparent — `RedbScopeStore` and `RedisScopeStore` are
interchangeable at the type level. Issue #902 flags the real risk: **the
consistency model is not** interchangeable, and nothing in the trait itself
says so. A caller written and tested against the redb backend can hold
assumptions that are silently false against the redis backend, with no
compile-time or type-level signal that anything changed.

This ADR states plainly what each backend actually guarantees today, so
that gap is documented rather than assumed.

## Decision: what each backend actually guarantees

### v1 — `RedbScopeStore` (local, embedded)

Verified directly against `redb` 4.1.0's source
(`redb::Database::open`/`create`, `src/db.rs`), not assumed:

> "If the file has been opened for writing (i.e. as a `Database`)
> `DatabaseError::DatabaseAlreadyOpen` will be returned on platforms which
> support file locks (macOS, Windows, Linux)."

Concretely:
- **Cross-process**: redb takes an OS-level file lock on open-for-write. A
  second process opening the *same scope file* for writing gets a hard,
  immediate error (`DatabaseAlreadyOpen`) — not a blocking wait, not silent
  corruption, not a merge. `RedbScopeStore::open` currently surfaces this
  as `ScopeError::BackendUnavailable` (see the "known simplification" note
  in that file's commit — this is mapped as retryable/transient today,
  which is arguably correct for "try again after the other process
  closes it," but is NOT the same as "this request was invalid").
- **Within one process**: redb's own transaction manager serializes write
  transactions (`begin_write`) — `RedbScopeStore`'s `Arc<Database>` shared
  across threads/tasks is safe without any locking of our own; redb queues
  concurrent writers internally.
- **Net effect**: exactly one *process* may hold a given scope file open
  for writing at a time. A multi-agent node running several agent
  processes against the same node-scope file needs an explicit
  single-writer-process architecture (e.g. one long-lived process owns the
  scope file and others go through it, or an external lock/queue) — #902's
  own framing ("needs an explicit locking/queueing policy, not an assumed
  one") is correct and still applies; this ADR does not solve that
  architecture question, only confirms redb's actual failure mode so that
  design can be built on fact rather than guesswork.

### v2 — `RedisScopeStore` (distributed, stub)

- **No file lock, no single-writer enforcement of any kind.** Multiple
  processes/nodes can `SET` the same key concurrently; Redis's own
  semantics apply: last `SET` to actually execute wins, full stop. There
  is no error, no rejection, no signal to either writer that a race
  occurred.
- **No ordering guarantee across writers.** Two processes writing "at the
  same time" (wall-clock) may commit in either order depending on network
  latency to the Redis server; nothing in `RedisScopeStore` today
  timestamps, versions, or otherwise orders concurrent writes.
- **Net effect**: a write that "succeeds" (returns `Ok(())`) tells the
  caller nothing about whether it was the only writer, the last writer, or
  got immediately overwritten a moment later.

### Side by side

| | redb v1 | redis v2 |
|---|---|---|
| Concurrent write from a 2nd process | Fails fast (`DatabaseAlreadyOpen`) | Silently succeeds, last-write-wins |
| Concurrent write from a 2nd thread in the *same* process | Serialized safely (redb internal) | Same — Redis serializes per-command server-side |
| Caller can detect "I was racing another writer" | Yes (the error) | **No** |
| Network partition tolerance | N/A (local file) | Eventual; a partitioned writer may write to a stale view with no local signal |

## What this means for callers today

**Do not port code from the redb backend to the redis backend (or vice
versa) assuming identical failure behavior.** Concretely:

- Code that treats "write succeeded" as "I know I was the only writer at
  that instant" is correct *only* under redb's fail-fast guarantee, and
  silently wrong under redis.
- Code that retries on `ScopeError::BackendUnavailable` because "the other
  writer will finish and release the lock soon" is a redb-shaped retry
  policy; under redis there is no lock to wait out, so the same retry loop
  either does nothing useful or (worse) races itself.
- `ScopeChainView::get_raw_with_audit`'s audit log (#900) records *where*
  a value resolved, not *whether it was consistent at the moment of read*
  — this is unaffected by which backend is in the chain, but should not be
  mistaken for a concurrency guarantee either.

## Generation token (#897) — explicitly not yet implemented

Issue #897 ("generation token on ScopeStore — cross-query read consistency
for concurrent agents") is **not built** as of this ADR. Flagging this
honestly rather than describing behavior that doesn't exist yet:

A generation token, once built, would need backend-specific mechanics —
it is not a drop-in constant-cost addition to either backend:

- **Under redb**: cheap. A monotonic counter can live in the same
  transaction as every write (single-writer-per-process, already
  serialized by redb itself) — incrementing it is just one more key in the
  same `begin_write` transaction, atomic by construction.
- **Under redis**: not free. There is no single serializing writer to hang
  a monotonic counter off of "for free" the way redb's transaction gives
  you one. A distributed generation counter needs `INCR` (atomic on the
  Redis server, but that's a *second* round-trip per write, not bundled
  with the value write unless done in a `MULTI`/Lua script) or a
  vector-clock-per-writer scheme if cross-node ordering without a single
  point of coordination matters. Whichever is chosen, it changes
  `RedisScopeStore::set_raw`'s cost profile (currently one Redis command
  per write) — that tradeoff needs its own decision when #897 is actually
  scoped, not assumed here.

## What breaks at cutover (redb → redis migration)

If a scope currently backed by `RedbScopeStore` is later migrated to
`RedisScopeStore` (e.g. a node scope outgrowing local-file semantics),
anything relying on redb's fail-fast concurrent-writer error breaks
silently — the equivalent redis write will simply succeed and clobber.
**A cutover must be paired with an explicit audit of any caller that
matches on `ScopeError::BackendUnavailable` as a concurrency signal**, not
just a storage-location change.

## Recommendation

1. Treat `ScopeError::BackendUnavailable` as backend-specific in
   *meaning*, even though it's backend-agnostic in *type* — redb's use of
   it (transient-but-real lock contention) and redis's use of it
   (connection-level failure only, never a concurrency signal) are
   different enough that a caller-facing doc comment on `ScopeError` noting
   this would help; not changing the error taxonomy itself in this ADR
   (that's a larger API change, out of scope here).
2. Scope #897 (generation token) as its own follow-up with the two
   backends' actual cost/consistency tradeoffs above as its starting
   context, not a green-field design.
3. Any future work that lets a single logical scope migrate backends (redb
   → redis) must include the caller-audit step above as an explicit,
   named task — not an assumed side effect of swapping the trait impl.
