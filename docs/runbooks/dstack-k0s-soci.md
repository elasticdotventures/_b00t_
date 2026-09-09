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
| `dev-env/k0s-ci-test.task.yaml` | — | RUN-stage task pinned to `backends: [kubernetes]`, key-based GCS auth |
| `dev-env/gcs-obj.sh` | build/test box | now falls back to `GOOGLE_APPLICATION_CREDENTIALS` (openssl JWT) when GCE metadata is absent |

## Apply order

### 1. b00t-node — SOCI

```sh
pyinfra --dry <inv-with-b00t-node> nats/pyinfra/deploy_k0s_soci.py \
  --data soci_version=0.10.0 --data k0s_role=controller   # single combined node
# review, then drop --dry
```

Verify on b00t-node:

```sh
systemctl is-active soci-snapshotter-grpc
k0s ctr plugin ls | grep soci                 # -> ok
crictl info | jq -r '.config.containerd.snapshotter'   # -> "soci"
```

### 2. b00t-node — GCS read key for the k8s test tasks

k0s on Vultr has no GCE metadata SA. Create a **minimal** key:

```sh
gcloud iam service-accounts create b00t-k0s-gcsread --project promptexecution
gcloud storage buckets add-iam-policy-binding gs://b00t-buildcache-promptexecution \
  --member "serviceAccount:b00t-k0s-gcsread@promptexecution.iam.gserviceaccount.com" \
  --role roles/storage.objectViewer
gcloud iam service-accounts keys create b00t-k0s-gcsread.json \
  --iam-account b00t-k0s-gcsread@promptexecution.iam.gserviceaccount.com
dstack secret set gcs_sa_key "$(cat b00t-k0s-gcsread.json)" && shred -u b00t-k0s-gcsread.json
```

🚩 Long-lived key. Scope is objectViewer-on-one-bucket only. **Follow-up:**
replace with a short-lived-token refresher (CronJob) once the path is proven.

### 3. Artifact Registry pull secret for k0s

```sh
kubectl create secret docker-registry ar-pull \
  --docker-server=australia-southeast1-docker.pkg.dev \
  --docker-username=_json_key --docker-password="$(cat b00t-k0s-arpull.json)" \
  -n default
kubectl patch serviceaccount default -n default \
  -p '{"imagePullSecrets":[{"name":"ar-pull"}]}'
```

(same 🚩 — scoped `artifactregistry.reader`, rotation TODO.)

### 4. Control node — add the kubernetes backend

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

### 5. Build a SOCI-indexed image

Run the `b00t-build-image` workflow (dispatch or push to `containers/b00t-build/**`).
The `soci-index` job pushes the index. Check b00t-node's
`journalctl -u soci-snapshotter-grpc` on the next k8s run for lazy mounts.

### 6. Smoke the k8s test task

```sh
dstack apply -f dev-env/k0s-ci-test.task.yaml -n k0s-smoke \
  -e CI_ARCHIVE_KEY=ci-archives/<a real prior build> \
  -e CI_REF=<sha> -e NEXTEST_PARTITION=1/4 --yes
```

## Open follow-ups

- **Retire both 🚩 keys → keyless via Workload Identity Federation.** k0s's own
  OIDC issuer federates to GCP; a projected ServiceAccount token is exchanged at
  STS for a short-lived access token — no key file. Wiring:
  `b00t-tf/modules/gcp-build-plane/wif-k0s.tf` (gated) + `deploy/k0s/`
  (kustomize base: SA, `external_account` ConfigMap, AR-pull refresher CronJob)
  + `deploy/k0s/publish-oidc-discovery.sh`. `gcs-obj.sh` already handles the
  `external_account` config type. Design: `docs/superpowers/specs/2026-09-09-keyless-gcp-identity-k0s.md`.
- ⚠️ **The k0s-issuer WIF is a time-boxed bootstrap.** The multi-cloud end
  state (GCP + Azure + AWS) uses a **SPIRE trust domain** as the single OIDC
  issuer — build it before AWS onboards; do not add a second per-cluster
  issuer. Requirements + sequence:
  `docs/superpowers/specs/2026-09-09-identity-plane-minimum-requirements.md`.
- Entra ID provider is already in `wif-k0s.tf` (gated by `entra_tenant_id`);
  the reverse (GCP/GitHub → Azure) is in
  `modules/azure-control-plane/wif-reverse.tf`.
- `proxy_jump.hostname` → tailnet IP once Phase 2.75 tailnet cutover lands.
- b00t-node sizing: too small for `--workspace` cold builds → k0s hosts the
  compile-free test fan-out; GCP Spot keeps `build_archive`.
- Dagster: add a `backend` knob to `DstackResource` so `ci_build_plane` can
  target k8s for the run ops.
