# `RepoBuildCI`: a Typed Multi-Check Job Task for Repo Verification

**Date:** 2026-09-12
**Status:** Proposed
**Motivated by:** this session delegated "verify _b00t_ issue #1299 / PR #1310" to two coding-agent
harnesses (`pi`, `opencode`) in parallel. Both had to improvise the same shape of result by hand in
prose: a named list of checks (`cargo test`, `announce`, `discover`, `broadcast roundtrip`), each with
its own PASS/FAIL/blocked status and a short evidence line — because nothing in `b00t job`'s schema
captures "run N named checks against a repo, report per-check structured results" as a first-class
concept. Today's closest existing thing, `_b00t_/build-and-test.job.toml`, gets this granularity by
hand-writing one `[[b00t.job.steps]]` per stage with a multi-line bash blob per step — which loses
per-*command* results within a step (its own lint/test stages already paper over this with `|| true`,
because a single failing command anywhere in the blob would otherwise fail the whole step with no
indication of which line broke).

**Sibling sub-projects from the same brainstorming session:** GKE+ARC runner migration (spec'd at
`docs/superpowers/specs/2026-09-12-gke-arc-runner-migration-design.md` — covers *where* GitHub-Actions-
triggered CI compute runs; unrelated trigger path from this design, see Architecture); `h00k`
cost/time-prediction primitive; CRDT state-machine telemetry into ledgrrr (both spec'd in parallel).
This design's completion output is shaped to feed both of those without modification.

---

## Scope

**In scope:** a new `JobTask::RepoBuildCi` variant (`b00t-cli/src/datum_job.rs`) — a single job step that
runs a **named, ordered list of checks** against a repo/worktree, executes each independently (so one
check's failure doesn't hide whether earlier/later checks passed), and reports a structured per-check
result. Authorable in a `.job.toml` file exactly like existing `[[b00t.job.steps]]` entries, and
runnable via the existing `b00t job run <name>` path — no new CLI subcommand, no new job-file format,
no new execution engine. This is a new *task type* inside the system that already exists.

**Out of scope:**
- Any change to `.github/workflows/*.yml` or ARC/GKE. GitHub Actions' own scheduler decides which
  runner executes a workflow step; `RepoBuildCi` is a `b00t job` task type, invoked by `b00t job run`
  (by a human, an agent, or — optionally, as a *consumer*, not a dependency of this design — a CI
  workflow step that itself shells out to `b00t job run some-repo-build-ci-job`). This design does not
  make `b00t job` a GitHub Actions runner replacement or a competing scheduler.
- Backend dispatch (`backend = "dstack"` / future `CodeRunnerActionProcess`) for `RepoBuildCi` steps.
  The existing `backend`/`batch` fields on `JobStep` already provide this orthogonally — a
  `RepoBuildCi` task runs exactly where its containing step runs, whether that's local (`backend =
  None`, today's default and this design's only supported mode) or dispatched (`backend = "dstack"`,
  once `BatchJobSpec` gains a way to carry an arbitrary in-container command — it doesn't today, see
  Open Questions). Not building that plumbing here.
- Migrating `build-and-test.job.toml` to the new task type. It keeps working unmodified; migrating it
  is a natural but separate follow-up once `RepoBuildCi` exists and has at least one real user.
- Automatic check *discovery* (e.g. "figure out what checks a repo needs by inspecting its
  Cargo.toml/package.json"). Checks are always explicitly authored in the job file — no magic.
- Retry/backoff policy beyond what `JobConfig.retry_failed_steps` already provides at the step level.

## Architecture

`RepoBuildCi` sits at the same level as `Bash`/`Python`/`Agent` in the existing `JobTask` enum — it is
one more way to fill in a `[[b00t.job.steps]].task`, not a parallel system:

```
[[b00t.job.steps]]
name = "verify-b00t-comms"
description = "Verify b00t-comms builds, tests, and round-trips over NATS"
checkpoint = "verified"

[b00t.job.steps.task]
type = "repo_build_ci"
repo_path = "."                       # cwd-relative, or absolute worktree path
ref = "task/pi-agent-harness"         # optional; if set, checked out (git fetch + checkout) before checks run
needs_gpu = false                     # advisory metadata only (see Open Questions) — not enforced here

[[b00t.job.steps.task.checks]]
name = "cargo-test"
command = "cargo test -p b00t-cli --bin b00t-comms"
timeout_ms = 120000

[[b00t.job.steps.task.checks]]
name = "announce"
command = "cargo run -q -p b00t-cli --bin b00t-comms -- announce"
timeout_ms = 15000
allow_fail = false                    # default; a failed check still runs later checks (see Data Flow)

[[b00t.job.steps.task.checks]]
name = "discover"
command = "cargo run -q -p b00t-cli --bin b00t-comms -- discover --timeout-ms 3000"
timeout_ms = 10000
depends_on_check = "announce"         # skip (not fail) if the named check didn't pass
```

Rust shape, added to `b00t-cli/src/datum_job.rs` alongside the other `JobTask` variants:

```rust
#[serde(rename = "repo_build_ci")]
RepoBuildCi {
    #[serde(default = "default_repo_path")]
    repo_path: String,
    #[serde(default)]
    r#ref: Option<String>,
    #[serde(default)]
    needs_gpu: bool,
    checks: Vec<RepoBuildCheck>,
},
```
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoBuildCheck {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub allow_fail: bool,
    #[serde(default)]
    pub depends_on_check: Option<String>,
}
```

## Components

1. **`RepoBuildCi` task variant** (`datum_job.rs`) — schema only, as above.
2. **Executor** (`job_executor.rs`, new `execute_repo_build_ci` alongside the existing
   `execute_bash`/`execute_provider_step` functions): if `ref` is set, `git fetch && git checkout
   <ref>` in `repo_path` first (fails the whole task step immediately if this fails — a bad ref isn't a
   "check", it's a setup precondition). Then runs each `RepoBuildCheck.command` via the same
   `duct::cmd` shell-out `Bash` already uses, in the order given, honoring `depends_on_check` (skip,
   don't run, if the named prior check didn't `Pass`) and `timeout_ms` per check independently. Collects
   one `CheckResult { name, status: Pass|Fail|Skipped|TimedOut, duration_ms, evidence: String }` per
   check — `evidence` is the last ~5 lines of combined stdout/stderr (mirrors the existing
   `output_contract` doc-comment's own "PASS|FAIL:<5lines>" convention, so this isn't inventing a new
   evidence-truncation idea).
3. **Step-level result rollup**: the `RepoBuildCi` task's own status (what `JobStep`'s existing
   pass/fail gate sees) is `Pass` only if every check with `allow_fail = false` is `Pass`; a step
   containing only `allow_fail = true` checks (or none) can't itself report a false "all good" — at
   least one `allow_fail = false` check is required by the schema (validated at job-load time, not
   silently defaulted).
4. **Output**: `Vec<CheckResult>` is written to the same per-job JSON log
   `build-and-test.job.toml` already uses (`.b00t/jobs/<job-name>/<run-id>.json`) as a `checks` array,
   and additionally serialized in the shape sibling designs need without new code: one row per check
   maps directly onto `JobUtilizationEvent`'s fields from
   `2026-08-30-dstack-multicloud-locality-cost-design.md` (`job_id` = `<job-name>/<check-name>`,
   `run_duration` = the check's `duration_ms`, `cold_start_duration` = 0 for local execution today,
   `estimated_cost` = 0 for local execution today — both become meaningful once a `RepoBuildCi` step is
   ever run via a dispatched `backend`, which this design doesn't build but doesn't preclude either).

## Data Flow

```
b00t job run verify-b00t-comms
  → JobExecutor walks steps sequentially (existing behavior)
  → step "verify-b00t-comms" has task.type = repo_build_ci
  → execute_repo_build_ci():
      optional git checkout(ref)
      for check in checks (in file order):
        if check.depends_on_check set and that check's status != Pass: record Skipped, continue
        run check.command with check.timeout_ms
        record Pass / Fail / TimedOut + evidence
  → roll up: step Pass iff all allow_fail=false checks are Pass
  → existing checkpoint/rollback machinery fires exactly as it does for any other step today
  → CheckResult[] written to job log
```

## Error Handling

- **Bad `ref`** (doesn't exist, checkout fails): whole step fails immediately, no checks run — this is
  a setup precondition, not a check outcome, so it does not produce a misleading "all checks skipped"
  result; it produces a clear "checkout failed" step error using the existing step-failure path.
- **A check times out**: recorded as `TimedOut` (distinct from `Fail`, so downstream tooling — e.g. a
  future `h00k` cost/time predictor — can tell "genuinely broken" apart from "took too long," which
  have different implications for retry/backoff decisions).
- **Zero checks with `allow_fail = false`**: job-load-time validation error (see Components §3) —
  fail fast at `b00t job plan`, not silently at runtime.
- **`depends_on_check` names a check that doesn't exist in the list**: job-load-time validation error,
  same reasoning.

## Testing / Validation

- Unit tests in `datum_job.rs`/`job_executor.rs` mirroring the existing `execute_provider_step_requires_
  batch_spec`-style tests: a `RepoBuildCi` task with one always-passing and one always-failing check
  (e.g. `command = "true"` / `command = "false"`) asserts the rollup logic; a `depends_on_check` chain
  asserts `Skipped` propagation; a bad `ref` asserts the step fails before any check runs.
- Manual validation: author `_b00t_/verify-b00t-comms.job.toml` using the exact example under
  Architecture above (the real, currently-blocked-on-NATS-auth scenario from this session) and run it —
  this both validates the design and finally produces the structured PASS/FAIL evidence issue #1299's
  own last comment asked for, which neither `pi` nor `opencode` fully delivered this session.

## Open Questions

- **`needs_gpu` is advisory-only in this design** — nothing reads or enforces it yet. It's included now
  so the field exists in the schema before any job author starts writing `RepoBuildCi` steps (avoiding a
  breaking schema change later), but wiring it to anything (e.g. a future `b00t hive plan`-style
  resource gate, or a `backend_hint` for dispatch) is deliberately deferred — whoever designs the
  dstack `CodeRunnerActionProcess` follow-on should decide whether this field or a new one drives that.
- **Dispatching a `RepoBuildCi` task via `backend = "dstack"`** doesn't work today because
  `BatchJobSpec` has no field for "run this list of commands" (it drives a container's own
  ENTRYPOINT/CMD, per its own doc comment at `provider.rs:97`) — bridging "checks list" into "one
  container invocation" (e.g. serializing checks to JSON and having a generic runner image execute them)
  is real design work, explicitly not attempted here per this design's Scope.
- **Whether `git fetch` inside `execute_repo_build_ci` should be skippable** (e.g. for a worktree that's
  already checked out to the right ref and shouldn't touch network) — no flag for this yet; add one if
  the first real usage (the manual validation job above) shows it's needed.
