# CRDT State-Machine Telemetry → ledgrrr

**Date:** 2026-09-12
**Status:** Proposed
**Motivated by:** this session's brainstorming on b00t job/hive infrastructure. User's framing: "ledgrrr will
receive CRDT logs of the state machine and have its own distinct states of operation and be able to
measure how much time it spends on each task with telemetry." One of four sibling sub-projects from the
same session (others: `2026-09-12-gke-arc-runner-migration-design.md`, already written; a `RepoBuildCI`
job-type spec and an `h00k` cost/time-prediction spec, status unknown at time of writing — see Open
Questions).

**Why CRDT specifically, not just an event log:** b00t job execution is not guaranteed single-writer. A
job can be delegated across the NATS hive mesh (`_b00t_#1299`, `b00t-cli/src/bin/b00t-comms.rs`,
`NatsMeshNode`) — multiple peers may observe or relay the same job's state transitions, hosts can be
offline and sync later, and there is deliberately no captain/coordinator with authority to serialize
writes (the mesh's own design: "captain is convention-only," no durable registration). A telemetry
mechanism for this environment needs to merge cleanly with no central arbiter and no lost/duplicated
updates regardless of delivery order or replay — that is the CRDT requirement, not a buzzword choice.

## Scope

**In scope (v1):**
1. One state machine instrumented: **b00t job execution checkpoints** (`b00t job run`/`status`/
   `checkpoints`, backed by `job_executor.rs` per the dstack design's own reference to that file). Not
   `b00t task` (pending/in-progress/done), not agent-to-agent NATS chat state, not the sibling `h00k`
   promise lifecycle — those are future producers against the same event shape (§ Future Producers), not
   built here.
2. The CRDT primitive itself: a **grow-only, content-hash-deduplicated append log** of immutable
   state-transition events (a G-Set, not a general document CRDT) — see § CRDT Design for why this is
   sufficient and why heavier options are rejected.
3. Transport from producer(s) to ledgrrr: NATS pub/sub over the existing hive mesh, reusing
   `NatsMeshNode`/`b00t-comms` rather than inventing a second message bus.
4. ledgrrr ingestion: a new `OperationKind` variant following the exact pattern already established by
   `RecordDecision`/`RecordCost` in `ledgrrr/crates/ledger-core/src/ledger_ops.rs` (immutable,
   content-hashed, replay-safe/idempotent — matching Phase 2's "deterministic content-hash transaction
   IDs as the replay/idempotency key" convention, not a new ingestion philosophy).
5. Derived, computed-not-stored per-job/per-step time-spent measurement (§ Data Flow).
6. ledgrrr's own new state machine for *this pipeline specifically* (§ ledgrrr's States) — narrow reading
   of "ledgrrr... have its own distinct states of operation": this is about the ingestion pipeline's
   state, not ledgrrr growing a general-purpose state-machine engine.

**Out of scope (v1):**
- Any CRDT type beyond the append-only G-Set (no LWW-register, no PN-Counter, no Automerge/yrs document
  CRDT — see § CRDT Design for why).
- Instrumenting `b00t task`, agent chat/presence state, or `h00k` promises — noted as future producers,
  not designed here.
- Historical backfill of job runs that happened before this ships.
- A UI/dashboard for the derived time measurements — ledgrrr already has query/reporting surfaces
  (`ledgerr-mcp`); this design only specifies what gets ingested, not a new presentation layer.
- Reconciling this design's transport choice with whatever `h00k`'s spec proposes for its own
  event-emission — flagged as an open question, not resolved here, since that spec doesn't exist yet at
  time of writing.

## Relationship to `2026-08-30-dstack-multicloud-locality-cost-design.md`

That design's `JobUtilizationEvent` (job_id, backend, cold_start_duration, run_duration, estimated_cost)
is **not replaced or duplicated** by this design. Reconciliation: `JobUtilizationEvent` remains the
ledger-entry *summary* — one record per completed job, matching its own "small outbox, fire-and-forget"
transport (a single-writer POST from `job_executor.rs` once a job finishes, no CRDT needed for a
single-writer summary). This design's CRDT log is the *substrate that summary is computed from* when
multiple writers/hosts are involved: instead of `job_executor.rs` unilaterally deciding
`cold_start_duration`/`run_duration` from its own local clock and POSTing once, each state transition
(`queued`, `started`, `checkpoint:<name>`, `completed`/`failed`) is appended to the CRDT log by whichever
peer observes it, ledgrrr merges the log for a given `job_id`, and *derives* the same
`JobUtilizationEvent` fields from the merged, deduplicated, time-sorted result. Same schema, same
ledger-entry kind — this design changes how it's populated (durable multi-writer merge) for jobs that go
through the hive mesh, and is a strict superset: a job with exactly one observer degenerates to the
existing single-writer behavior with no change in output.

## CRDT Design

**Chosen primitive: grow-only set (G-Set) of immutable `JobStateEvent` records, deduplicated by content
hash.** Each event:

```rust
pub struct JobStateEvent {
    pub job_id: String,           // matches JobUtilizationEvent.job_id
    pub step: Option<String>,     // None for job-level queued/completed/failed; Some(name) for a checkpoint
    pub transition: String,       // "queued" | "started" | "checkpoint" | "completed" | "failed"
    pub observed_at: DateTime<Utc>,  // producer's wall clock
    pub observed_by: String,      // hive agent_id that emitted this (from b00t-comms.agent.toml pid)
    pub content_hash: String,     // sha256(job_id|step|transition|observed_by) — NOT observed_at,
                                   // so a re-delivered/retried copy of the *same* logical event
                                   // dedupes even if observed_at differs by network jitter
}
```

Merge rule: union of events by `content_hash`, full stop — no vector clocks, no last-writer-wins
conflict resolution, because nothing is ever mutated or retracted. This is the identical pattern already
established in this codebase by `SoulDataFramerr` (`b00t-c0re-lib/src/soul_dataframerr.rs`: "Append-only:
rows are never mutated (immutable log, CRDT-safe across sessions)") — this design applies that existing,
proven house convention to hive-distributed job telemetry rather than inventing a new one.

**Rejected alternatives:**
- **Automerge / yrs (general document CRDTs):** zero existing usage anywhere in `_b00t_` (verified:
  `grep -ri "automerge\|yrs\|yjs"` across `*.rs`/`*.toml` returns no dependency, only this design's own
  research). Both exist to merge arbitrary structured/nested mutable documents (rich text, JSON trees).
  A state-transition log has no such structure — it is a set of facts that are true forever once
  observed. Pulling in a document-CRDT library for a G-Set is solving a much smaller problem with a much
  bigger hammer.
- **PN-Counter for "time spent per state":** rejected because time-spent is not something that needs
  concurrent increment/decrement from multiple writers — it's a pure function of two timestamps
  (`observed_at` of transition N+1 minus transition N) computed once over the already-merged, sorted
  event log. Modeling it as a counter CRDT would require deciding how concurrent increments compose,
  which doesn't apply here: the *events* are the CRDT; the *duration* is a downstream derived value with
  ordinary arithmetic, not a value that itself needs conflict-free merge semantics.
- **LWW-register per (job_id, step):** rejected for the same reason as PN-Counter — there is no "current
  value" being overwritten; every transition is a distinct, permanently-true fact, so a set beats a
  register.

## Architecture / Data Flow

```
job_executor.rs (any hive peer running/observing a job)
   │ emits JobStateEvent on each transition
   ▼
NATS mesh (existing NatsMeshNode, subject: b00t.hive.mesh.channel.ledgrrr-telemetry)
   │ pub/sub — any number of producers, no coordinator required
   ▼
ledgrrr-telemetry-relay (new, small: a NATS subscriber that batches events and calls
   ledgerr-mcp's existing MCP surface — not a new ingestion path, reuses the contract
   ledger_ops.rs already exposes)
   │
   ▼
ledger-core: OperationKind::RecordJobStateEvent { job_id, step, transition, observed_at, observed_by,
   content_hash } — appended as an immutable, content-hash-keyed entry, mirroring RecordDecision/
   RecordCost's existing shape exactly (same content-hashed-evidence pattern, new variant)
   │
   ▼
On query (not on every write): merge all RecordJobStateEvent rows for a job_id by content_hash,
   sort by observed_at, compute per-step duration as consecutive-event deltas, and produce a
   JobUtilizationEvent-shaped summary row (§ Relationship section) — either materialized
   lazily on read or opportunistically re-materialized whenever a job's event set changes
   (implementation detail, not load-bearing to this design; start with lazy/on-read, the
   cheaper option, per YAGNI).
```

### ledgrrr's States (the pipeline's own state machine)

Narrow reading, per Scope: this is the *ingestion pipeline's* state for a given `job_id`'s event set, not
a general new ledgrrr subsystem:

```
received → merged → measured
```

- `received`: at least one `RecordJobStateEvent` row exists for this `job_id`.
- `merged`: a `completed` or `failed` transition has been observed in the merged set (job telemetry is
  now complete, though more redundant/duplicate events may still arrive and dedupe harmlessly).
- `measured`: the derived `JobUtilizationEvent`-shaped summary has been computed at least once for this
  `job_id`.

This state is itself derivable from the same underlying G-Set (it's a query, not additional stored
state) — kept here only as a documented interpretation of "distinct states of operation," not a new
column/table.

## Error Handling

- **NATS auth failure**: this design's transport is currently **blocked** on the live auth-propagation
  bug this same session found in `b00t-comms`/`NatsHiveTransport` (`NatsMeshNode`'s `async_nats::connect()`
  not honoring `HIVE_NATS_USER`/`HIVE_NATS_PASSWORD` from the URL) — see Open Questions. Until fixed, no
  hive peer can reliably publish `JobStateEvent`s over NATS at all.
- **Duplicate/replayed events**: handled by design (content-hash dedup is the whole point of the G-Set
  choice) — not an error case, a normal operating condition.
- **Relay down / ledgrrr unreachable**: events are not lost — NATS mesh delivery is fire-and-forget per
  `NatsMeshNode`'s existing semantics (no durable queue group specified in this design; if durability
  across a relay outage matters, that's a NATS JetStream consumer configuration decision for the relay,
  not a change to the event shape — flagged as an implementation detail, not blocking this design).
- **Out-of-order delivery**: irrelevant to merge correctness (G-Set union doesn't care about order) but
  affects derived duration computation until all relevant events have arrived — the `measured` state is
  therefore explicitly re-computable, not a one-shot.

## Testing / Validation

- Unit: `JobStateEvent` content-hash dedup — two events with identical `(job_id, step, transition,
  observed_by)` but different `observed_at` produce the same hash and merge to one row.
- Integration: two simulated hive peers publish overlapping/out-of-order events for the same `job_id`
  over a real local NATS server (already available in this environment per this session's work);
  confirm the merged, derived summary matches hand-computed expected durations regardless of publish
  order.
- Regression: a job observed by exactly one peer (the common case today) produces byte-identical
  `JobUtilizationEvent`-shaped output to what `job_executor.rs`'s existing single-writer POST would have
  produced — proving this design is a strict superset, not a behavior change for the simple case.

## Future Producers (not built here)

Once this event shape and transport exist, these become straightforward additional producers rather than
new designs: `b00t task` status transitions, `h00k` promise lifecycle transitions (pending → resolved/
rejected, once that sibling spec defines its states), and NATS mesh presence/gossip events themselves
(already flowing over the same mesh, currently not persisted anywhere).

## Open Questions

- **Blocking dependency**: the NATS auth-propagation bug (found this session, `b00t-comms`/
  `NatsHiveTransport`) must be fixed before any part of this ships — this design does not include that
  fix, it's tracked against issue `_b00t_#1299`'s follow-ups.
- The `RepoBuildCI` and `h00k` sibling specs did not exist at time of writing — if `h00k` proposes its
  own event-emission/transport mechanism, reconcile against this design's NATS-based transport rather
  than shipping two parallel telemetry pipes; whoever writes that spec second should read this one.
- Whether the `ledgrrr-telemetry-relay` should be a standalone small binary/service or a mode of an
  existing ledgrrr component (e.g. `ledgerr-mcp`) — left to implementation; either satisfies this design.
- Retention/compaction policy for the G-Set as it grows unboundedly over time (it is, by construction,
  append-only forever) — not addressed here; existing ledger-core append-only storage presumably already
  has an answer for this at the storage layer, inherited rather than redesigned.
