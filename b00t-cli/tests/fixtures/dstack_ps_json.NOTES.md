# dstack_ps_json.txt / dstack_logs_output.txt — capture notes

Captured 2026-07-23 against a real dstack 0.20.28 server + real RunPod backend
(project "b00t"), submitting a trivial `commands: [echo "PASS"]` task with
`resources: { gpu: 1 }` in region `EU-RO-1` (cheapest available offer:
NVIDIA RTX 2000 Ada Generation, $0.24/hr, run duration ~2 minutes — actual
spend was a few cents). Fleet deleted and run stopped immediately after
capture; verified no secrets present in either file before committing.

## Critical finding: fleet-first requirement (not in the original design)

The plan's Task 1/Task 2 design assumed `dstack apply -f <task.yml> -y -d`
would dynamically provision a pod per task, the way older dstack / RunPod's
own API works. **This is no longer true as of the installed 0.20.28.**

First attempt (`dstack apply` on a bare task config, no fleet) failed
immediately, no RunPod API call made at all:

```
Job status changed SUBMITTED -> TERMINATING. Termination reason:
FAILED_TO_START_DUE_TO_NO_CAPACITY (No matching fleet found. Possible
reasons: https://dstack.ai/docs/guides/troubleshooting/#no-fleets)
```

dstack now requires an explicit `type: fleet` resource (with `nodes: 0..N`
for autoscaling, so it costs nothing while idle) to exist in the project
*before* any task can be scheduled.

**Fixed** in commit `bf1a6406`: `DstackProvider::ensure_fleet` (idempotent
`type: fleet`, `nodes: 0..1` apply, mirroring the existing `ensure_volume`
pattern) is now called from `submit_dstack_yaml` before every task/training
submission, against one shared fleet name (`SHARED_FLEET_NAME`).

Second finding, possibly a dstack matching quirk worth re-testing later:
a CPU-only fleet (`resources: { gpu: false }`, no GPU) found **zero**
matching offers via `dstack offer --fleet <name>`, even though the
backend-agnostic `dstack offer -b runpod` command (no fleet filter) listed
real CPU-only RunPod instance types (e.g. `cpu5c-8-16` at $0.28/hr) that
should have matched `cpu=2.. mem=8GB.. disk=100GB.. gpu=0..`. Did not
chase this further since GPU-fleet offers matched cleanly and the goal was
fixture capture, not a full offer-matching audit — flagging in case a
follow-up task hits the same wall for genuinely CPU-only workloads.

## Task 10 (e2e smoke test): three more real bugs found, all fixed in commit `116bfefe`

Running `b00t-cli provider job submit-batch --provider dstack` for real
(not hand-typed `dstack` commands — the actual Rust code path, for the
first time) surfaced three more unconditional bugs:

1. **Scratch config files written outside CWD's subtree.** `ensure_volume`/
   `ensure_fleet`/`submit_dstack_yaml` wrote their generated YAML to
   `std::env::temp_dir()` (`/tmp`). dstack's own `apply` command computes
   `configuration_path.absolute().relative_to(Path.cwd())` and errors
   — before any network call — if the file isn't inside CWD's subtree:
   `ValueError: '/tmp/...' is not in the subpath of '<cwd>'`. Fixed via a
   new `dstack_scratch_config_path()` helper, rooted at CWD.

2. **Job/training names exceeded dstack's name regex.**
   `format!("b00t-job-{}", uuid::Uuid::new_v4())` produces a 45-character
   name; dstack requires `^[a-z][a-z0-9-]{1,40}$` (max 41 chars) and
   rejects anything longer with `Resource name should match regex ...`.
   This made every single `submit_batch_job`/`submit_training_job` call
   fail unconditionally. Fixed via `dstack_short_id()` (12 hex chars
   instead of a full UUID).

3. **`commands: [echo starting]` was hardcoded on every task**, silently
   discarding every real image's actual ENTRYPOINT/CMD (e.g. `mesh-runner:
   v6` would never run its own logic). dstack's own `TaskConfiguration`
   model requires "either `commands` or `image` must be set", not both —
   confirmed by inspecting the installed 0.20.28 source directly. Fixed by
   omitting `commands:` entirely, letting the image run its own default.

## Task 10 evidence (trace-or-filler, per b00t's own law)

Real, live run through `b00t-cli` (not raw `dstack` CLI) after all fixes
above, against dstack 0.20.28 + live RunPod:

```
$ b00t-cli provider job submit-batch --provider dstack --image ubuntu:24.04 --config /dev/null --flavor cpu
{
  "id": "b00t-job-2dd5da33120d",
  "provider": "dstack"
}

$ b00t-cli provider job status --provider dstack b00t-job-2dd5da33120d
run=b00t-job-2dd5da33120d status=done
```

Polled 10 times over ~80 seconds; status was already `done` by the first
poll and stayed `done` — no 10-second-timeout regression, the exact
failure mode (RunPod's flaky short poll cutoff) this whole branch exists
to fix. Fleet deleted and dstack server process stopped immediately after.

**Not exercised**: the job.toml + `output_contract` PASS/FAIL path
end-to-end with a real dstack submission. `BatchJobSpec` has no
command-override field (by design — job images are meant to bake in their
own PASS/FAIL-emitting entrypoint, e.g. `mesh-runner:v6`), so there's no
way to inject a literal `echo "PASS"` through the real dstack path without
either building a throwaway test image or adding a command-override field
to `BatchJobSpec` — both are real scope decisions, not bugs, and are
flagged here as a follow-up rather than done unsupervised. Task 7's
`evaluate_output_contract` itself already has dedicated unit test coverage
(4 tests, reviewed and merged) independent of this gap.

## Files

- `dstack_ps_json.txt` — full `dstack ps --json -a` output, single completed
  run, real project state.
- `dstack_logs_output.txt` — `dstack logs b00t-fixture-capture` output
  (just `PASS`, the echoed command output).
