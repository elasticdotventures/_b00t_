# Runbook — dstack `kubernetes` backend on k0s + SOCI lazy pull

Plan gleaming-jingling-nygaard, k0s/SOCI follow-up. Turns the two eval docs
(`docs/superpowers/specs/2026-09-09-{lazy-pull-snapshotter-eval,dstack-runtime-podman-k0s}.md`)
into wired infra. **Authored, not applied** — every on-node step below is
operator-gated.

## Why

- **No Docker.** k0s runs containerd; the dstack `kubernetes` backend schedules
  Pods via the k8s API. No `dockerd` on b00t-node.
- **Lazy image pull.** With SOCI on the k0s node, `b00t-build` (multi-GB) starts
  before it is fully pulled — the payoff a snapshotter gives that the dockerd
  VM backend cannot.
- Same control plane / UI / OTLP export; only a backend entry is added.

## Pieces

| File | Runs on | Purpose |
|---|---|---|
| `nats/pyinfra/templates/dstack-server-config.yml.j2` | control node | adds a `kubernetes` backend block when `k0s_kubeconfig` is passed |
| `nats/pyinfra/deploy_cp_node.py` | control node | puts the kubeconfig, renders the block, `--data k0s_kubeconfig= k0s_proxy_jump_host=` |
| `nats/pyinfra/files/fetch-k0s-kubeconfig.sh` | b00t-node | emits a kubeconfig with the API host rewritten to the tailnet IP |
| `nats/pyinfra/deploy_k0s_soci.py` | b00t-node | installs `soci-snapshotter-grpc` + `soci`, systemd unit, k0s containerd drop-in |
| `nats/pyinfra/files/soci-snapshotter-grpc.service` | b00t-node | the snapshotter daemon unit |
| `nats/pyinfra/files/soci-snapshotter-config.toml` | b00t-node | prefetch-heavy snapshotter config |
| `nats/pyinfra/files/k0s-containerd-soci.toml` | b00t-node | `/etc/k0s/containerd.d/` drop-in — CRI `snapshotter = "soci"` |
| `.github/workflows/b00t-build-image.yml` (`soci-index` job) | CI | `soci create` + `soci push` the index to Artifact Registry |
| `dev-env/k0s-ci-test.task.yaml` | — | RUN-stage task pinned to `backends: [kubernetes]`; keyless GCS via a SPIRE SVID + `external_account` config |
| `dev-env/gcs-obj.sh` | build/test box (baked into the image) | credential order: GCE metadata → `external_account` (WIF/SVID, keyless STS exchange) → SA key |

## Apply order

### 1. b00t-node — SOCI

```sh
pyinfra --dry <inv-with-b00t-node> nats/pyinfra/deploy_k0s_soci.py \
  --data soci_version=0.15.0 --data k0s_role=controller   # single combined node
# review, then drop --dry
```

Verify on b00t-node:

```sh
systemctl is-active soci-snapshotter-grpc
k0s ctr plugin ls | grep soci                 # -> ok
crictl info | jq -r '.config.containerd.snapshotter'   # -> "soci"
```

### 2. Identity — keyless via the existing SPIRE plane (NOT here)

k0s on Vultr has no GCE metadata SA. GCS + Artifact Registry access for
build-plane pods is **keyless via `PromptExecution/infrastructure`'s live SPIRE
trust domain** (`spiffe://promptexecution.com`, issuer
`spire-oidc.promptexecution.com`). The delta needed there — a
`fleet/spire-agents.json` entry, the first real `objectViewer` +
`artifactregistry.reader` role grants on the bound SA, and a SPIRE
registration entry — is tracked in
`docs/superpowers/specs/2026-09-09-build-plane-identity.md`. Do **not** mint a
long-lived key or a parallel WIF pool here.

`dev-env/gcs-obj.sh` (baked into the `b00t-build` image) already consumes an
`external_account` config → SVID → STS exchange; the pod just needs the SVID +
config delivered by the SPIRE CSI driver / `spiffe-helper`.

### 3. Control node — add the kubernetes backend

```sh
# on b00t-node:
sudo nats/pyinfra/files/fetch-k0s-kubeconfig.sh > /tmp/b00t-node.kubeconfig
# copy it to the machine running pyinfra, then:
CP_HOST=$(cd b00t-tf && tofu output -raw gcp_control_node_external_ip) \
pyinfra --dry nats/pyinfra/inventory_cp.py nats/pyinfra/deploy_cp_node.py \
  --data build_vm_sa_email=$(cd b00t-tf && tofu output -raw gcp_build_vm_sa_email) \
  --data k0s_kubeconfig=/tmp/b00t-node.kubeconfig \
  --data k0s_proxy_jump_host=<b00t-node tailnet IP>
```

Verify: `dstack server` restarts, `dstack fleet` / `dstack apply` can target k8s.

### 4. Build a SOCI-indexed image

Run the `b00t-build-image` workflow (dispatch or push to `containers/b00t-build/**`).
The `soci-index` job pushes the index. Check b00t-node's
`journalctl -u soci-snapshotter-grpc` on the next k8s run for lazy mounts.

### 5. Smoke the k8s test task

```sh
dstack apply -f dev-env/k0s-ci-test.task.yaml -n k0s-smoke \
  -e CI_ARCHIVE_KEY=ci-archives/<a real prior build> \
  -e CI_REF=<sha> -e NEXTEST_PARTITION=1/4 --yes
```

## Open follow-ups

- **Identity is keyless via the existing SPIRE plane** — the delta lives in
  `PromptExecution/infrastructure` (`fleet/spire-agents.json` + first real role
  grants + a SPIRE registration entry). See
  `docs/superpowers/specs/2026-09-09-build-plane-identity.md`. `_b00t_` only
  consumes SVIDs; no WIF pool / long-lived key here.
- `proxy_jump.hostname` → tailnet IP once Phase 2.75 tailnet cutover lands.
- b00t-node sizing: too small for `--workspace` cold builds → k0s hosts the
  compile-free test fan-out; GCP Spot keeps `build_archive`.
- Dagster: add a `backend` knob to `DstackResource` so `ci_build_plane` can
  target k8s for the run ops.

## dstack kubernetes backend — bring-up notes (2026-09-09)

**dstack 0.20.28: `NO_OFFERS` for everything.** Bumped to **0.21.5** (b00t #189,
`deploy_cp_node.py DSTACK_VERSION`). 0.21's k8s backend actually enumerates the
cluster — but two gotchas:

1. **Default `disk: 100GB` exceeds vultr1's 72GB ephemeral-storage** → no node
   matches → `NO_OFFERS`. Give the task an explicit `disk:` **range** (e.g.
   `disk: 10GB..60GB`) — a bare value that is disjoint from the fleet's raises
   "Cannot combine fleet requirements" (see item 3).
2. **dstack creates an SSH jump-pod `Service` with `nodePort: <proxy_jump.port>`**
   → `nodePort: 22` is outside k8s's `30000-32767` range → `422 Unprocessable
   Entity`. Fix: `proxy_jump.port: 30022` (now the `deploy_cp_node.py` default)
   + open `tcp:30022` `tag:vultr1` in the tailnet ACL (done).

### 3. dstack 0.21.5 k8s backend — VERIFIED end-to-end (2026-09-09)

`dstack apply` runs a real Pod on the k0s node. Two more requirements beyond
the disk/nodePort fixes above:

3. **A `kubernetes` fleet must exist, and must NOT carry a `resources:` block.**
   0.21.5 assigns every run to a fleet; `get_fleet_requirements` intersects the
   fleet's `resources:` with the task's, and disjoint ranges (e.g. fleet
   `disk: 10GB`, task `disk: 5GB`) → `"Cannot combine fleet requirements"` →
   `FAILED_TO_START_DUE_TO_NO_CAPACITY`. An unconstrained fleet combines with
   anything:

   ```yaml
   # dev-env/k0s-ci-fleet.yaml
   type: fleet
   name: k0s
   nodes: 1
   backends: [kubernetes]
   ```

   `dstack apply -f dev-env/k0s-ci-fleet.yaml -y`. The fleet's own instance
   shows `terminated / no_offers` forever — the k8s backend has no
   `create_instance` (idle Pods are meaningless), so it can't provision fleet
   capacity. **That is cosmetic**: fleet `STATUS` stays `active`, and jobs
   schedule through the `run_job` Pod-creation path, not the fleet instance.

4. **Delete any stale `dstack-<project>-ssh-jump-pod`.** An orphan jump Pod from
   an earlier attempt (Service missing) makes `create_namespaced_pod` return
   `409 Conflict`. One-time cleanup:
   `kubectl -n default delete pod dstack-main-ssh-jump-pod --ignore-not-found`.
   dstack then recreates the jump Pod + `NodePort` Service (`nodePort: 30022`)
   itself.

Verified task (`alpine:3.20`, `cpu: 1 / memory: 512MB / disk: 10GB..60GB`,
`backends: [kubernetes]`) → offer `cpu=x86:1 mem=0.5GB disk=10GB vultr $0` →
Pod `dstack-main-<run>-0-0-<sfx>` scheduled on `vultr` → commands ran →
`Exited (0)`. Repeatable. `proxy_jump` (control node → 100.109.101.1:30022) and
the tailnet path both exercised.

### The SVID-carrying CI pod goes through kubectl/Dagster, NOT dstack

dstack 0.21.5's `kubernetes` backend does **not** surface pod-spec injection —
confirmed against source (`dstack/_internal/core/backends/kubernetes/compute.py`
`_create_job_pod`, 0.21.5). The only user-controlled pod-spec knobs are:
`resources` (cpu / memory / gpu / `shm_size`), `privileged: true`, and `volumes:`
mount points that each resolve to **either** a dstack-managed PVC
(`KubernetesVolumeConfiguration` → `persistentVolumeClaim`) **or** a `hostPath`
(`InstanceMountPoint`, `DirectoryOrCreate`). There is no init-container,
inline-CSI (`csi.spiffe.io`), configMap-volume, or `serviceAccountName`
support, and no raw pod-spec / strategic-merge-patch passthrough.

So `dev-env/k0s-ci-test.task.yaml`'s Pod — which needs a `csi.spiffe.io`
ephemeral volume + a `spiffe-helper` init container + the ADC ConfigMap + its
own `b00t-ci` ServiceAccount — **cannot** run through `dstack apply`. Apply
that one Pod via `kubectl` / `orchestration/dagster/` (full manifest, ref
`k0s/spire/agent.yaml` in PromptExecution/infrastructure). Plain compile-free
tasks with no identity requirement DO run through `dstack apply` (verified
above). Every other piece — SOCI image, SPIRE SVID delivery (`k0s/spire/`),
keyless GCS (`gcs-obj.sh` `external_account`) — is verified.

## (historical) dstack 0.20.28 kubernetes backend — `NO_OFFERS`

The backend is wired and the control node reaches the k8s API + `proxy_jump`
(both verified), but `dstack apply` (task or fleet) against `backends:
[kubernetes]` fails with **`NO_OFFERS (No offers found)`** — dstack 0.20.28's
k8s backend doesn't enumerate a self-managed single-node cluster's allocatable
resources as "offers".

- **Fix path:** bump dstack `0.20.28 → 0.21.x` (b00t task #189) — 0.21 rewrote
  the kubernetes backend. Retest after.
- **Interim:** drive the k8s build-plane pods directly (kubectl / the Dagster
  path in `orchestration/dagster/`) using the verified pieces — SOCI image,
  SPIRE SVID delivery (`k0s/spire/` + spiffe-helper), keyless GCS
  (`gcs-obj.sh` external_account). `dev-env/k0s-ci-test.task.yaml` documents the
  pod shape; apply it as a raw Pod for now, not via `dstack apply`.
