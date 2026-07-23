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
*before* any task can be scheduled. A task submitted with no matching fleet
fails fast with `No fleets` / `FAILED_TO_START_DUE_TO_NO_CAPACITY` — the
job never reaches RunPod, so this failure mode is free, but it also means
**`DstackProvider::submit_batch_job` as currently implemented (b00t-cli/src/
commands/provider.rs) will always fail against real dstack 0.20.x** unless
a suitable fleet already happens to exist in the target project.

Minimal working fleet config used for this capture:

```yaml
type: fleet
name: b00t-fixture-fleet
nodes: 0..1
backends: [runpod]
regions: [EU-RO-1]
resources:
  gpu: 1
```

Second finding, possibly a dstack matching quirk worth re-testing later:
a CPU-only fleet (`resources: { gpu: false }`, no GPU) found **zero**
matching offers via `dstack offer --fleet <name>`, even though the
backend-agnostic `dstack offer -b runpod` command (no fleet filter) listed
real CPU-only RunPod instance types (e.g. `cpu5c-8-16` at $0.28/hr) that
should have matched `cpu=2.. mem=8GB.. disk=100GB.. gpu=0..`. Did not
chase this further since GPU-fleet offers matched cleanly and the goal was
fixture capture, not a full offer-matching audit — flagging in case Task 3
or a follow-up task hits the same wall for genuinely CPU-only workloads.

## Implication for Task 3 / Task 10 / this branch generally

**Fixed** in commit `bf1a6406`: `DstackProvider::ensure_fleet` (idempotent
`type: fleet`, `nodes: 0..1` apply, mirroring the existing `ensure_volume`
pattern) is now called from `submit_dstack_yaml` before every task/training
submission, against one shared fleet name (`SHARED_FLEET_NAME`). Not
re-verified against a second live RunPod run (the exact fleet YAML shape
was already proven working manually during this capture — see above); a
second live run would be redundant spend for the same already-confirmed
shape.

Task 3's parser still needs to handle the *shape* `dstack_ps_json.txt`
shows (`status: "done"`, `termination_reason: "all_jobs_done"`, nested
`run_spec.configuration`, etc.) — that part of this note stands regardless
of the fleet fix.

## Files

- `dstack_ps_json.txt` — full `dstack ps --json -a` output, single completed
  run, real project state.
- `dstack_logs_output.txt` — `dstack logs b00t-fixture-capture` output
  (just `PASS`, the echoed command output).
