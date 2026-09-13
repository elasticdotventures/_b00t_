# GKE Autopilot + ARC: Move Self-Hosted CI Off sm3lly

**Date:** 2026-09-12
**Status:** Proposed
**Motivated by:** sm3lly (`sm3llsl1k3s0ld3r`) is a single RTX 3090 dev box that also hosts the local
Qwen3.8-27B inference backend (GPU sits at ~22.3/24.6GB used, ~1.8GB headroom) plus two coding-agent
harnesses (`pi`, `opencode`) that delegate to it. Two things were building on this box and had to stop:
`actions-runner-app4dog` (a pm2-managed classic self-hosted runner for `app4dog/workspace`) and the ARC
`gpu-sm3lly` `AutoscalingRunnerSet` (registered against `elasticdotventures/_b00t_`, 0 current runners at
time of writing). Both are decommissioned as of this session (pm2 process stopped, `helm uninstall
gh-runner-gpu -n arc-runners` run against the local k0s cluster). This design replaces that local runner
capacity with a GKE Autopilot cluster, so cargo builds/tests never compete with local inference again.

**Builds on, does not duplicate:** `k8s/gh-runner-gpu/` — a reviewable-but-never-applied artifact already
in this repo (helm values, `create-secret-from-vault.sh`, README) targeting sm3lly's local k0s. Its auth
story (GitHub App `b00t-arc-runners`, app_id `4687493`, installed on `elasticdotventures`, key in Azure
Key Vault `kv-pe-agent-secrets`, fetched via SPIRE workload identity) and its cache/PoC-scoping decisions
are reused as-is; only the target cluster and node-selection mechanism change.

**Relationship to `2026-08-30-dstack-multicloud-locality-cost-design.md`:** that design defers a
dstack-native `CodeRunnerActionProcess` provider for GH-Actions-runner dispatch as an explicit follow-on,
and separately designs a `JobUtilizationEvent` (job_id, backend, cold_start_duration, run_duration,
estimated_cost) emission into ledgrrr's ledger-core. Decision for this session (see Open Questions): ship
GKE+ARC now as the fast, low-risk path; do not build `CodeRunnerActionProcess` yet. To avoid a second,
incompatible telemetry format later, ARC job-completion data emitted by this design (§ Telemetry) uses
the same event shape so a future migration to dstack-dispatched runners is a backend swap, not a schema
rewrite.

**Out of scope (this design):**
- `ci-gpu-systemd.yml`'s actual runtime GPU-presence-check refactor (tracked separately — this design
  only specifies *where* that workflow's runner comes from, not its internal test logic).
- Migrating `actions-runner-app4dog` (different GitHub org — `app4dog`, not `elasticdotventures` — the
  existing `b00t-arc-runners` App install doesn't cover it; needs its own App installation or an
  org-wide reinstall, noted as a fast-follow, not blocking this design).
- The `RepoBuildCI` structured `b00t job` type, the `h00k` cost/time-prediction primitive, and CRDT
  state-machine telemetry into ledgrrr — three sibling sub-projects from the same brainstorming session,
  each getting its own spec. This design's `JobUtilizationEvent`-shaped output (§ Telemetry) is the seam
  they'll attach to.
- dstack `CodeRunnerActionProcess` (see above — deferred, not this design).

---

## Architecture

GKE Autopilot cluster (single, free management-tier cluster in the existing `promptexecution` GCP
project — the management fee is waived; only pods that actually run are billed) replaces sm3lly's local
k0s cluster as the ARC target. GKE itself never runs a job: it is purely the control plane that decides,
per pending runner pod, which Autopilot **compute class** to provision from — CPU (default, near-zero
idle cost, scales 0→N) or GPU (`nvidia-l4` or similar, autoprovisioned only when a GPU-labeled job is
pending, scales back to 0 when idle). This mirrors the existing `gh-runner-gpu` PoC's `minRunners: 0`
posture, generalized across two node shapes instead of one fixed physical box.

```
GitHub (elasticdotventures/_b00t_)
   │  workflow queues a job against runs-on: <scale-set-name>
   ▼
ARC listener pod (arc-systems, GKE)  ──polls──▶  GitHub Actions API
   │ creates EphemeralRunner
   ▼
AutoscalingRunnerSet (arc-runners, GKE)
   │ pod spec requests either the default compute class or the GPU compute class
   ▼
GKE Autopilot control plane
   │ autoprovisions a node from the requested compute class (0→1), schedules the runner pod
   ▼
Runner pod executes the job, reports completion, pod is deleted
   │ (JobUtilizationEvent emitted — see Telemetry)
   ▼
GKE Autopilot scales the node back to 0 once idle
```

## Components

1. **Terraform** (`infrastructure/terraform/google/gke-arc-runners.tf`, new file): a
   `google_container_cluster` in Autopilot mode (`enable_autopilot = true`), one per project, region
   matching `local.c0ntext.region`. No node-pool resources — Autopilot manages those implicitly via
   compute classes requested in pod specs. Output the cluster's `endpoint`/`ca_certificate` for
   `gcloud container clusters get-credentials` scripting.

2. **ARC controller**: same chart/version already installed on sm3lly (`gha-runner-scale-set-controller`
   0.14.2), fresh `helm install` into the new cluster's `arc-systems` namespace. No config changes needed
   — it's cluster-agnostic.

3. **Two `AutoscalingRunnerSet` releases** (adapting `k8s/gh-runner-gpu/values.yaml`):
   - `cpu-builds` (new): `runnerScaleSetName: cpu-builds`, no `nodeSelector`/`runtimeClassName`/GPU
     resource requests — Autopilot's default compute class. `minRunners: 0`, `maxRunners` sized to
     whatever concurrency is actually observed (start at 2, per the existing PoC's "raise once real
     concurrency behavior has been observed" note). This is what `ci.yml`'s general `cargo
     build`/`test` jobs and `actions-runner-app4dog`'s former workload move to (app4dog migration itself
     is out of scope per above — this just makes the scale-set available).
   - `gpu-builds` (renamed from `gpu-sm3lly`, since it's no longer tied to that physical box):
     `nodeSelector`/`tolerations` swapped for whatever Autopilot's GPU compute-class selector syntax
     requires (`cloud.google.com/compute-class: <gpu-class-name>` + `nvidia.com/gpu` resource request —
     confirm exact key at implementation time against current Autopilot docs, since this is a
     fast-moving GKE feature). `maxRunners: 1` retained from the PoC.

4. **Caching**: Autopilot does not permit `hostPath` volumes (a hard restriction, unlike the single-node
   k0s PoC which relied on one). The existing warm-`$CARGO_HOME`-on-NVMe trick does not carry over
   directly. v1 accepts this regression and relies on the sccache work already landed (PR #1227) for
   cross-run cache reuse instead of a local volume. A `ReadWriteOnce` PVC-backed cache is a possible
   fast-follow if sccache alone proves insufficient — not designed here (YAGNI until measured).

5. **Auth**: reuse `k8s/gh-runner-gpu/create-secret-from-vault.sh` unmodified in logic, parameterized to
   target the new GKE cluster's kubeconfig context instead of the local k0s context (its SPIRE-based
   Key-Vault fetch runs from wherever it's invoked — sm3lly, since that's where the live SPIRE workload
   identity chain already exists — and simply `kubectl apply`s the resulting secret against
   `--context gke_<project>_<region>_<cluster>` instead of the local context). No new credential
   material, no change to the GitHub App itself.

## Telemetry

Each `EphemeralRunner` completion (success or failure) emits one event matching the field shape of
`2026-08-30-dstack-multicloud-locality-cost-design.md`'s `JobUtilizationEvent`: `job_id` (the GH Actions
`run_id`+`job_id` pair), `backend` (`"gke-arc-cpu"` or `"gke-arc-gpu"`), `cold_start_duration` (pod
scheduled → runner registered), `run_duration` (runner registered → job complete), `estimated_cost`
(compute-class hourly rate × wall time, GPU class priced separately from CPU). Emission mechanism:
smallest viable v1 is a `postJobHook`-style step appended to the runner pod template that POSTs this
event to ledgrrr's existing ingest surface (same ledger-entry-kind path the dstack design already
specifies) — no new ledgrrr code required if that ledger-entry kind lands first; if it hasn't yet, this
design's rollout blocks on it existing, not on building a parallel one.

## Migration Steps

1. `terraform apply` the new GKE Autopilot cluster (infrastructure/terraform/google).
2. `helm install` ARC controller into the new cluster's `arc-systems` namespace.
3. Run `create-secret-from-vault.sh` targeted at the new cluster's context to materialize
   `arc-runners/gh-runner-gpu-creds` (rename to `arc-runners/gh-runner-creds` since it now backs two
   scale sets, not one GPU-specific one).
4. `helm install cpu-builds` and `helm install gpu-builds` (new values files derived from
   `k8s/gh-runner-gpu/values.yaml`) into the new cluster.
5. Update `_b00t_/.github/workflows/ci.yml` (and any other CPU-bound workflow currently on
   `ubuntu-latest` that would benefit from a warm-cache/faster runner) and `ci-gpu-systemd.yml`'s
   `runs-on:` to the new scale-set names.
6. Verify one real PR triggers both scale sets correctly (a CPU-only PR, and a manual
   `workflow_dispatch` of `ci-gpu-systemd.yml`).
7. Decommission: confirm no workflow still references `[self-hosted, gpu, sm3lly]` or any sm3lly-local
   scale-set name; delete the now-unused `k8s/gh-runner-gpu/` PoC values (superseded by the new
   `cpu-builds`/`gpu-builds` values files) or fold it into a `k8s/gke-arc-runners/` directory that
   supersedes it — naming/cleanup decision left to implementation, not blocking this design.

## Error Handling

- **Secret creation fails** (SPIRE/Key-Vault chain unreachable from wherever the script runs): ARC
  controller logs `AutoscalingRunnerSet` reconcile errors, no listener pod starts, jobs queue
  indefinitely against the scale set — same failure mode as the current PoC's documented blocker, not a
  new risk.
- **GPU compute class unavailable/quota exhausted** in the target region: pod stays `Pending`,
  `ci-gpu-systemd.yml`'s job times out per its own `timeout-minutes` (assumed already set; verify at
  implementation time) rather than hanging forever — no new orchestration-level retry logic proposed for
  v1.
- **Telemetry POST fails**: best-effort, does not block or fail the CI job itself — logged, not retried,
  matching the dstack design's own "outbox" framing (fire-and-forget from the execution side).

## Testing / Validation

- `terraform plan` reviewed before apply (standard practice, not new).
- One CPU-only PR against `_b00t_` observed end-to-end: job queues, `cpu-builds` scale-set provisions a
  node, job runs, node scales back to 0 within a few minutes of idle.
- One manual `workflow_dispatch` of `ci-gpu-systemd.yml` observed end-to-end against `gpu-builds`,
  confirming the GPU compute class actually attaches a usable GPU (`nvidia-smi` inside the job).
- Confirm `nvidia.com/gpu.present` / `runtimeClassName: nvidia` node-selector references from the old
  PoC are fully replaced — a workflow accidentally still requesting the old sm3lly-specific selector
  should fail to schedule loudly (`Pending` pod, visible in `kubectl get pods -n arc-runners`), not
  silently fall back to sm3lly (sm3lly has no ARC agent registered anymore, so this failure mode is
  structurally impossible post-migration, but worth confirming once).

## Open Questions

- Exact Autopilot GPU compute-class selector syntax and available GPU types/regions — confirm against
  current GKE docs at implementation time (this is a fast-evolving Autopilot feature area).
- Whether `maxRunners: 2` for `cpu-builds` is enough — start there per the existing PoC's own guidance,
  raise based on observed queuing.
- Whether the ledgrrr ledger-entry-kind this design's Telemetry section depends on already exists or
  needs to be requested as a prerequisite from whoever owns the CRDT-telemetry sub-project spec.
