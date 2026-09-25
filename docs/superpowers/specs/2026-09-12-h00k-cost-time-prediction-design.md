# h00k: Predictive Cost & Time Estimates for b00t Jobs

**Date:** 2026-09-12
**Status:** Proposed
**Motivated by:** user request (same brainstorming session as the GKE+ARC runner migration spec):
"promptexecution should be able to register a 'h00k' (b00t hook) — new concept that registers an async
promise and anticipates the time & cost [future, FOCUS] and that is based on other similar jobs." This
is the fourth of four sibling sub-projects from that session (GKE+ARC runner migration — spec'd at
`2026-09-12-gke-arc-runner-migration-design.md`; `RepoBuildCI` structured job type; CRDT state-machine
telemetry into ledgrrr — the latter two spec'd separately/in parallel).

**Critical prior art — this is smaller than it first looks, because most of the substrate already
exists:**

1. **`b00t focus`** (`b00t-cli/src/commands/focus.rs`) already reads/aggregates real FOCUS
   (FinOps Open Cost & Usage Spec) records from `focus_records.jsonl`, written by ledgrrr-mcp, via
   `FocusJsonlSequence`/`FocusSchema`/`AbDataFrame` (`datum_schema.rs`). `focus aggregate --group-by
   <dimension> --metric BilledCost` already computes historical cost aggregation by dimension — this
   **is** the "based on other similar jobs" cost corpus the user is asking for; it is not new.
2. **`b00t budget`** (`b00t-cli/src/commands/budget.rs`) already has `Simulate` ("check if a job would
   be allowed based on budget") and `Record`/`Check`/`Init`, backed by `BudgetController`/`BudgetState`
   — a real threshold/gate mechanism, but keyed on a running spend tally for a `Stack`, not a per-job
   historical prediction. Complementary to h00k, not a substitute: h00k predicts a number, `budget
   simulate`-style logic can consume that number as a gate input.
3. **`2026-08-30-dstack-multicloud-locality-cost-design.md`** designs `JobUtilizationEvent` (job_id,
   backend, `cold_start_duration`, `run_duration`, `estimated_cost`) as a **new, not-yet-built**
   ledgrrr `ledger-core` ledger-entry-kind. FOCUS itself has no wall-clock-duration column (it is a
   cost/usage spec, not a job-timing spec) — so **time** prediction has no data source until
   `JobUtilizationEvent` lands; **cost** prediction can start today against existing FOCUS records.
   **Consistency note for whoever implements the dstack design's `JobUtilizationEvent`:** emit it into
   the *same* FOCUS record store/schema (`focus_records.jsonl` / `FocusSchema`) that `b00t focus`
   already reads, rather than a second, parallel ledger-entry-kind — extend `FocusSchema` with
   duration columns (FOCUS's extension mechanism already supports custom/x- columns per spec) rather
   than standing up a second historical corpus h00k (or anything else) would need to query twice.
4. `ledgrrr/crates/ledgerr-gcp-billing` maps real GCP BigQuery FOCUS export rows into
   `ledgerr_focus::CostAndUsageRow` — confirms `ledgerr_focus`'s `CostAndUsageRow` is the canonical
   Rust FOCUS record type this whole system already converges on.

Given that, h00k is **not** a new cost-accounting pipeline. It is a thin predictive layer: given a job
about to run, query the existing FOCUS corpus (cost) and, once available, the `JobUtilizationEvent`
corpus (time), for records matching a similarity key, and surface a prediction — before the job
starts, not after.

**Out of scope (this design):**
- Any new cost-accounting/telemetry pipeline — reuses FOCUS records and (once built)
  `JobUtilizationEvent`; does not invent a third.
- A general ML/similarity model. v1 similarity is exact-match on (job/datum name, backend) — see
  Components.
- Enforcing budget gates itself — `b00t budget simulate` already owns "should this be allowed"; h00k
  only owns "what will this likely cost/take," and can be fed as an input to that existing gate.
- The generic `Dependency<'a, C: Constraint>`/`ufo-types` DAG pattern noted as forward-compatibility
  in the dstack design — h00k's similarity key is deliberately simple (a tuple, not a constraint
  graph) for v1; migrating it onto that generic pattern later, if/when `ufo-types`' DAG construct
  exists, is a mechanical follow-up, not designed here.
- Prediction-accuracy tracking (predicted vs. actual, feeding back into confidence intervals) — real
  and valuable, but a second-order enhancement once v1's basic predict/resolve loop exists and has
  accumulated enough resolved h00ks to make accuracy tracking meaningful. Noted under Open Questions.

---

## Architecture

h00k models a registered async operation as a **promise with three states**: `predicted` (registered,
not yet resolved — this is the "anticipates" moment) → `running` (optional, set when the underlying job
actually starts) → `resolved` | `failed` (the real outcome is known). This mirrors a JS Promise's
pending/fulfilled/rejected, which is where the "async promise" framing in the user's request comes
from — but it is a passive record, not a real Rust `Future`; nothing about h00k drives execution, it
only annotates a job b00t's existing `job`/`task`/`provider` machinery is already going to run.

```
caller (b00t job run / a CI step / a human)
   │
   ▼
b00t hook register <job-name> --backend <backend>
   │  1. build similarity key: (job_name, backend)
   │  2. query existing FOCUS corpus:  b00t focus aggregate --group-by job_name --metric BilledCost
   │     (filtered to the similarity key, most-recent-N records)
   │  3. query JobUtilizationEvent corpus for the same key, if that store exists yet (else: "no
   │     time-history yet" — explicit, not a fabricated number)
   │  4. compute median + p90 of matched cost/duration records
   │  5. persist a HookRecord{state: predicted, prediction, similarity_key, created_at} — see Components
   ▼
prints/returns: "predicted cost $X (p90 $Y), predicted duration Zm (p90 Wm), based on N prior runs"
   │
   ▼ (job actually runs, via whatever mechanism the caller was already going to use)
   │
b00t hook resolve <hook-id> --cost <actual> --duration <actual> | --failed
   │  updates HookRecord{state: resolved|failed, actual}
   ▼
(fast-follow, not v1: compare predicted vs actual, feed a running accuracy metric)
```

## Components

1. **`b00t-cli/src/commands/hook.rs`** (new) — a `HookCommands` enum following the exact pattern of
   `job.rs`/`budget.rs` (this codebase's established CLI-command-module shape; no new architectural
   pattern introduced):
   - `register { job_name: String, backend: Option<String>, min_samples: usize (default 3) }` — runs
     the prediction flow above, prints a human-readable estimate (or `--json`), returns a `hook_id`.
   - `resolve { hook_id: String, cost: Option<f64>, duration_secs: Option<u64>, failed: bool }` —
     records the actual outcome.
   - `show { hook_id: String }` / `list { job_name: Option<String>, state: Option<String> }` —
     inspection.
   No new library trait/abstraction in v1 — this is CLI-first, matching how `job`/`budget`/`focus`
   already work standalone. Extracting a shared trait only becomes worth it once a second call site
   (e.g. `RepoBuildCI`, or CI itself) actually needs to call registration programmatically rather than
   shelling out — YAGNI until that's real, not speculative.
2. **Similarity key (v1): `(job_name: String, backend: Option<String>)`** — exact match only. `job_name`
   is the same string already used as FOCUS's `group_by` dimension and as `JobUtilizationEvent`'s
   `job_id` prefix (the stable identifier, not a per-invocation UUID). No input-size bucketing, no
   fuzzy matching — simplest thing that could work, and the same key both existing corpora already
   index by, so no new indexing scheme is needed on either side.
3. **Prediction method (v1): median + p90 over the last `min_samples`..50 matching records.** Not a
   model — a direct statistic over `b00t focus aggregate`'s existing output (cost) and, once it exists,
   `JobUtilizationEvent` records (time). If fewer than `min_samples` (default 3) matching records
   exist, `register` returns an explicit `insufficient_history` result rather than guessing — a
   prediction with n=1 is worse than admitting there isn't one yet.
4. **`HookRecord` storage**: persisted the same way `budget.rs`'s `BudgetState` already is — a small
   JSON state file (`~/.b00t/hooks/<hook_id>.json` or a single append-only `hooks.jsonl`, mirroring
   `focus_records.jsonl`'s own shape) rather than a database. Matches this codebase's existing
   file-based-state convention for CLI tools (`BudgetState` in `budget.rs`, `focus_records.jsonl`),
   avoids introducing a new storage dependency for what is, in v1, a low-volume, single-operator
   record type.
5. **`FocusSchema` duration extension** (only once the dstack design's `JobUtilizationEvent` is
   actually implemented — not part of this design's own deliverable, listed here so the two specs
   agree on the target shape): add `ColdStartDuration`/`RunDuration` as FOCUS custom columns on the
   existing schema rather than a parallel ledger-entry-kind, per the consistency note above.

## Data Flow

1. A caller (a human, a CI step, or — later — `RepoBuildCI`) runs `b00t hook register build-and-test
   --backend gke-arc-cpu` before dispatching the actual job.
2. `hook.rs` shells into the same FOCUS-reading path `focus.rs` already uses
   (`FocusJsonlSequence`/`FocusSchema`) filtered to rows whose dimension matches `job_name` (and
   `backend` if present), takes the most recent matching records, computes median/p90 `BilledCost`.
3. It attempts the same query shape against a `JobUtilizationEvent`-backed source for duration; if that
   source doesn't exist yet (pre-dstack-design-implementation), the duration half of the result is
   `None` with an explicit reason string, not zero or a stale guess.
4. Writes a `HookRecord` in `predicted` state, prints/returns the estimate, including `n` (sample
   count) so the caller can judge confidence themselves (a median of 3 vs. a median of 40 should read
   differently — surfacing `n` is cheaper than computing a confidence interval and gives the caller the
   same information).
5. When the real job finishes (however it finishes — this design does not hook into `job_executor.rs`
   directly in v1; the caller is responsible for calling `hook resolve`, matching how `budget record`
   already requires an explicit call rather than automatic instrumentation), `hook resolve` updates the
   record to `resolved`/`failed` with the actual figures.

## Error Handling

- **No matching history** (`n < min_samples`): `register` succeeds but returns
  `prediction: insufficient_history` explicitly — never a fabricated number from an unrelated job.
- **`resolve` called on an unknown `hook_id`**: clear error naming the id, no silent no-op.
- **`resolve` called twice on the same `hook_id`**: second call errors (`already resolved`) rather than
  silently overwriting — a resolved prediction is a completed record, not a mutable cache entry.
- **FOCUS store unreadable/missing** (`focus_records.jsonl` absent): same behavior `focus query` already
  has today (a clear "no records" result, not a crash) — h00k does not add new failure modes here,
  just consumes the existing command's own error handling.

## Testing / Validation

- Unit tests for the median/p90 computation over a fixture set of matching/non-matching FOCUS rows
  (reuse `focus.rs`'s existing test fixtures/patterns where present).
- `register` with `n=0,1,2` (below `min_samples`) returns `insufficient_history`; `n=3+` returns a real
  prediction — both asserted.
- `resolve` on an already-resolved hook errors; on an unknown id errors; on a valid `predicted` hook
  transitions state and stores actuals — three cases.
- No live-cost/live-job integration test in v1 (nothing here executes real cloud spend) — this design
  only predicts and records, consistent with `budget simulate`'s own read-only nature.

## Open Questions

1. **Where does `JobUtilizationEvent` actually land** (which crate/store) — this design assumes it
   becomes a FOCUS-schema extension per the consistency note above; if whoever implements the dstack
   design instead builds a separate ledger-entry-kind, h00k's time-prediction query target needs a
   one-line update to point at that instead. Not a redesign either way, but worth resolving when that
   work starts so h00k isn't left querying a store that never materializes as assumed.
2. **Prediction-accuracy tracking** (predicted vs. actual delta, surfaced back to the caller or fed into
   a confidence adjustment) — explicitly deferred per Out of Scope; revisit once enough `resolved`
   `HookRecord`s exist for it to be meaningful (dozens, not a handful).
3. **Automatic `resolve`** — should `job_executor.rs` (or whatever eventually runs `RepoBuildCI` jobs)
   call `hook resolve` automatically on job completion, instead of requiring an explicit caller step?
   Deferred: v1 mirrors `budget record`'s existing manual-call convention; wiring it automatically is a
   natural fast-follow once there's a single call site (e.g. `RepoBuildCI`) to wire it into, rather than
   guessing at the integration point now.
