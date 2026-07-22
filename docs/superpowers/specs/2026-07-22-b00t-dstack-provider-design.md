# b00t dstack Provider — Job Orchestration Reliability Design

**Date:** 2026-07-22
**Status:** Proposed
**Motivated by:** RunPod-backed cloud AI jobs (mesh3d/Pixal3D today, Sapiens2 pose estimation next) burn most of their iteration time on orchestration failure, not the AI work itself. Root cause: `job_executor.rs::poll_until_terminal` gives up after 10s (`PROVIDER_MAX_POLLS=50 × PROVIDER_POLL_INTERVAL=200ms`), documented in its own comment as an MVP stopgap that can't distinguish "still pulling the image" from "gave up." `cloud_mesh.sh` bypasses that poller entirely and relies on the human manually re-running `b00t provider job status`. RunPod itself is not a requirement — reliable, fast, provider-agnostic AI job iteration is. `dstack` is already documented in `_b00t_/learn/dstack.md` and referenced as `ORCHESTRATOR=dstack` in `ai-finetune.just` / the provider datums, but has zero implementation: `get_provider()` only recognizes `runpod | hf | local`.

---

## Summary

1. Add a `DstackProvider: ComputeProvider` backed by the `dstack` CLI, giving b00t a real multi-cloud job backend (RunPod becomes one of dstack's backends, not a hard dependency).
2. Replace status-guessing with dstack's native run-state machine (submitted → provisioning → pulling → running → done/failed), which finally distinguishes "still pulling" from "stuck" from "dead" — RunPod's bare pod API cannot do this (its `desired_status` enum is only `Running | Exited | Terminated`).
3. Define a PASS/FAIL evidence contract at the provider layer so `b00t provider job status` reports the job's actual test outcome, not just container lifecycle state — this is the "reliably classifying tests as correct or incorrect" requirement, and it's a gap today regardless of provider.
4. Route `cloud_mesh.sh` (and the future Sapiens2 container job) through the new provider instead of one-shot-submit-then-manual-poll.

---

## Scope

**In scope:**
- `DstackProvider` implementing the existing `ComputeProvider` trait (`b00t-cli/src/commands/provider.rs`): `submit_batch_job`, `submit_training_job`, `job_status`, `cancel_job`, `list_jobs`
- `get_provider()` routing for `"dstack"`
- `PROVIDER-DSTACK.provider.tomllmd` datum, following the existing `PROVIDER-RUNPOD` / `PROVIDER-HF` convention
- A job-status contract that surfaces a real `PASS` / `FAIL: <detail>` / `RUNNING` / `PULLING` state, not just raw provider lifecycle strings — extends the `is_terminal_status`/`is_failure_status` helpers in `job_executor.rs`
- `cloud_mesh.sh` updated to submit via the new provider and either watch synchronously or print a real status command
- `just ai-finetune::*` recipes gain a working `ORCHESTRATOR=dstack` path (currently a documented no-op)

**Out of scope (deferred, tracked as later phases):**
- Automatic crash/hang retry policy (Phase 2)
- Preflight image warm/validation before submission (Phase 3)
- Building the actual Sapiens2 container / pose-estimation runner (separate workstream — audited, not designed here)
- Per-dog re-identification + per-dog persistent score records (separate workstream, to be brainstormed next)
- Removing `RunpodProvider`/`HfProvider` — they stay; dstack may use RunPod as a backend under the hood, and HF Jobs is explicitly documented as not dstack-covered

---

## Architecture

### Component placement (`~/.b00t`, consumed by `app4dog`)

```
b00t-cli/src/commands/provider.rs
  DstackProvider                  ← new, mirrors RunpodProvider/HfProvider shape
  get_provider()                  ← add "dstack" branch

b00t-cli/src/job_executor.rs
  is_terminal_status / is_failure_status
                                   ← extended with a distinct "pending/pulling" classification
                                     so poll_until_terminal doesn't conflate slow-pull with hang

_b00t_/datums/PROVIDER-DSTACK.provider.tomllmd
                                   ← new datum: auth, CLI install, usages (same shape as
                                     PROVIDER-RUNPOD.provider.tomllmd)

app4dog/game-play/pipelines/photo-critter/cloud_mesh.sh
                                   ← submit via `b00t provider job submit-batch --provider dstack`,
                                     replace "monitor manually" footer with a real watch/status call
```

### Status model

dstack's run states map onto three buckets our poller needs:

| dstack state | our bucket |
|---|---|
| `submitted`, `provisioning`, `pulling` | **pending** — not terminal, not stuck by itself |
| `running` | **running** |
| `done` | **terminal / success** (still needs PASS/FAIL check, see below) |
| `failed`, `terminated` | **terminal / failure** |

`job_status()` returns a structured status (not the current ad-hoc string) so `poll_until_terminal` can apply a *state-aware* timeout: generous while `pending` (cold starts are provider-dependent and can legitimately take minutes), tight once `running` (a job that's actually running should be making progress or emitting the PASS/FAIL contract below).

### PASS/FAIL evidence contract

Container exit code 0 means "the process didn't crash," not "the test passed." Job entrypoints write a terminal line b00t already standardizes on (`whoami`'s Cognitive Tiers table: `PASS` or `FAIL: <5-line excerpt>`) to stdout or a `result.json` in the job's workspace mount. `DstackProvider::job_status` reads that once the run reaches `done`/`failed` and folds it into the returned status, so a job that exits 0 but never asserted PASS is reported as indeterminate rather than silently treated as success (closing the gap `is_failure_status`'s doc comment already flags: "none of the providers expose an exit code... a bare 'exited' is treated as success rather than guessed at").

**Why not just route to MLflow?** b00t already runs MLflow for local training (`_b00t_/learn/hf-jobs-mlflow.md`: tracking server at `http://192.168.1.137:30803`, a LAN NodePort on the local k8s box), but that note documents it as **unreachable from any cloud GPU worker** — HF Jobs cloud runs already hit `ConnectTimeoutError` against it and fall back to `report_to: "none"`. The same LAN-reachability constraint applies to RunPod and any dstack-orchestrated cloud backend. So cloud jobs today have zero run-tracking of any kind, MLflow included — that's a direct contributor to "can't reliably classify pass/fail," independent of the polling bug. Exposing the tracking server publicly is a separate security/ops decision, out of scope here; the stdout/`result.json` contract above is deliberately push-free (the provider reads it back directly) so it works the same whether the job ran on the LAN or in someone else's cloud.

---

## Testing Strategy

- Unit tests for `DstackProvider` status parsing against fixture CLI output (dstack's exact JSON/status shape needs a docs pass at implementation time — flagged as an open question below, not assumed here)
- Extend the existing `job_executor.rs` poll tests (`with_statuses` fake provider) to cover a `pending → running → done+PASS` sequence and a `pending → running → done+FAIL` sequence
- One real end-to-end smoke job (`echo PASS`) submitted through `dstack` against an actual configured backend, run manually before calling this done — this is the trace-or-filler evidence line for the feature itself

---

## Open Questions (resolve during planning/implementation, not blocking this design)

1. Exact `dstack` CLI JSON/status output shape — shell out and parse, or wrap the REST/Python API directly? **No official Rust SDK exists for `dstackai/dstack`** (project is 75.7% Python, CLI + Python SDK + REST API only — verified against github.com/dstackai/dstack). `⚠️ crates.io/crates/dstack-sdk is an unrelated project (different "dstack" — do not depend on it.)` `DstackProvider` will shell out to the `dstack` CLI (matching the pattern `RunpodProvider`/`HfProvider` already established for `hf jobs`) unless the REST API proves clearly better once inspected.
2. Does `dstack` need its own fleet/compute config (`.dstack.yml`) committed to `~/.b00t` or `app4dog`, and which repo owns it?
3. Confirm dstack can actually target the clouds implied by "we have all the major clouds" — verify configured backends match expectation before assuming full multi-cloud coverage.

---

## Dependencies to Add

- `dstack` CLI: `uv tool install 'dstack[all]'` — not yet installed on this machine (`which dstack` → not found), so first implementation step is install + `dstack server` / cloud-backend config, before any Rust code lands. No new Rust crate needed (see Open Question 1).

---

# b00t:map v1
# summary: DstackProvider ComputeProvider backend — replaces RunPod-only bespoke polling with dstack's native multi-cloud run-state machine + a PASS/FAIL job-status contract
# tags: provider, dstack, orchestration, runpod, gpu, multi-cloud, reliability, job-status
# tier: frontier
# cmds: b00t-cli provider job submit-batch --provider dstack
# complexity: 5
