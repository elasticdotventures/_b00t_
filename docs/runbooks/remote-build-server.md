# Runbook — remote build server (GCP dstack build plane)

End-to-end: stand up the plane, use it, tear it down. Datum:
`_b00t_/build-plane.tomllm`. Plan: `gleaming-jingling-nygaard`. Spec:
`docs/superpowers/specs/2026-08-10-cloud-build-server-design.md`.

> Status 2026-09-08: IaC + pyinfra + dev-env authored & validated; **not yet
> applied**. Steps 2+ are the intended sequence, not a record of a completed run.

## 0. Operator inputs (one-time)

1. Confirm the ~$25–40/mo GCP spend on project `promptexecution`.
2. Your egress CIDR for the tcp/22 allowlist during the build phase (end state
   is tailnet-only — no rule). Put it in `b00t-tf/.env` as
   `GCP_ALLOWED_CIDRS=1.2.3.4/32`.
3. Add your workstation SSH key to `_b00t_/keyring.tomllm`: paste
   `~/.ssh/id_ed25519.pub`, set `machine` to your `hostname`, fill
   `fingerprint` (`ssh-keygen -lf ~/.ssh/id_ed25519.pub`), flip
   `status = "active"`.
4. A **read-only** GitHub deploy key for `elasticdotventures/_b00t_` — new
   keypair; public half → repo Settings → Deploy keys; keep the private half
   for step 4 (`remote-bootstrap`). This is for the in-box `git clone` only,
   unrelated to VM login (#3).
5. A cache encryption password (any strong string) — for step 4.
6. A Tailscale auth key (reusable, tagged) — for the tailnet cutover, and the
   cleanest reach path before then.

`gcloud auth application-default login` && `gcloud config set project promptexecution`.

## 1. Infra (OpenTofu)

```sh
cd b00t-tf
just gcp-bootstrap        # once — GCS state bucket + base APIs (Phase 1, already applied)
just tf-init-migrate      # once — move root state local -> gcs
just gcp-plan             # review: e2-small VM, 3 SAs, 2 roles, 2 firewalls, buildcache bucket, Cloud Run waker, WIF
just gcp-apply
```

Capture outputs:
```sh
tofu output -raw gcp_control_node_endpoint     # the pingap waker URL
tofu output -raw gcp_control_node_external_ip
tofu output -raw gcp_control_zone
tofu output -raw gcp_build_vm_sa_email
tofu output -raw gcp_build_plane_network
```

🤓 `tofu` needs `TMPDIR` on real disk (`export TMPDIR=$HOME/.cache/tofu-tmp`) —
the `/tmp` tmpfs can't hold the ~200 MB google provider.

## 2. Control node (pyinfra, over an IAP tunnel)

The control node has no public SSH — reach it via IAP. `inventory_cp.py`
resolves the address; run pyinfra through a local IAP tunnel:

```sh
# background: IAP tunnel :2222 -> the control node's :22
gcloud compute start-iap-tunnel b00t-dstack-control 22 \
  --local-host-port=localhost:2222 --zone australia-southeast1-a &

CP_HOST=localhost pyinfra --ssh-port 2222 --ssh-user brianh \
  --ssh-key ~/.ssh/google_compute_engine --dry \
  nats/pyinfra/inventory_cp.py nats/pyinfra/deploy_cp_node.py \
    --data build_vm_sa_email=$(cd b00t-tf && tofu output -raw gcp_build_vm_sa_email) \
    --data vpc_name=$(cd b00t-tf && tofu output -raw gcp_build_plane_network)
# then drop --dry
```

(`~/.ssh/google_compute_engine` is auto-created by a first
`gcloud compute ssh b00t-dstack-control --zone australia-southeast1-a --tunnel-through-iap`.)

Installs `dstack[all]==0.20.28`, renders `~/.dstack/server/config.yml` (GCP
backend, metadata ADC — no keys), the `dstack-server` systemd unit, and the
`dstack-idle-reaper` timer (powers the VM off when dstack is idle).

Verify: `ssh brianh@$CP_HOST 'systemctl is-active dstack-server'` → `active`.

## 3. Point the local dstack CLI at the waker

The server-side project is **`main`** (dstack's default; the control node
auto-creates it). Grab its admin token off the box and register it locally:

```sh
TOKEN=$(gcloud compute ssh b00t-dstack-control --zone australia-southeast1-a \
  --tunnel-through-iap --command \
  "python3 -c \"import yaml;print(yaml.safe_load(open('/home/brianh/.dstack/server/config.yml')).get('token',''))\"")
dstack project add main \
  --url $(cd b00t-tf && tofu output -raw gcp_control_node_endpoint) --token "$TOKEN"
```

The `just remote-*` recipes export `DSTACK_PROJECT=main`.

First call wakes the VM (~30–60 s). Break-glass without the waker (SSH via IAP —
**no static or external IP needed**; the reaper churns the ephemeral one so
never cache an address):
```sh
gcloud compute ssh b00t-dstack-control --zone <zone> --tunnel-through-iap \
  -- -L 3000:127.0.0.1:3000
```
IAP access is TF-managed: add your email to `[gcp].access_accounts` in
`b00t-tf/_b00t_.toml` (a list of bare `@elastic.ventures` emails) and `tofu
apply`. Each entry gets `roles/iap.tunnelResourceAccessor` conditioned to the
control instance.

## 4. Secrets + provision

The read-only **deploy key** for `elasticdotventures/_b00t_` (the box clones
that private repo; the `vendor/*` submodules are public, no key needed):

```sh
# generate + register the private half:
ssh-keygen -t ed25519 -N '' -f ~/.ssh/b00t_build_deploy_key -C b00t-build-plane
dstack secret set b00t_build_deploy_key "$(cat ~/.ssh/b00t_build_deploy_key)"
dstack secret set cache_password "$(openssl rand -hex 24)"   # zerofs only; harmless otherwise
# then paste ~/.ssh/b00t_build_deploy_key.pub into
#   github.com/elasticdotventures/_b00t_ -> Settings -> Deploy keys (read-only)

just remote-provision      # fleet (Spot e2-standard-8) + dev-environment
```

⚠️ `init:` installs the whole toolchain — **`ci-occt` is a minimal image**
(only `git`), not "OCCT/cmake/protoc baked". First provision ≈ 6–10 min
(apt + rustup + `cargo install` sccache/nextest); the source clone + submodules
add ~1 min. The cache mount is `gcsfuse` by default (`CACHE_BACKEND` in the
dev-env `env:`); if it can't mount, `cache-up.sh` binds a local dir and warns —
the build still runs, it just isn't warm across an idle-stop.

## 5. Use it

```sh
just remote-push my-branch
just remote-test my-branch   # first run cold; every run after is warm (cache in GCS)
just remote-keepalive        # optional — hold the control plane awake for a long build
```

**Measured (e2-standard-8, 8 vCPU @ 2.2 GHz, 2026-09-08):**
`cargo build -p b00t-cli` cold, empty cache ≈ **9 min** · warm (1-file change) ≈
**1.5 min** · no-op ≈ **1.4 min** (b00t-cli's compile-time build.rs).

## 6. Acceptance proof

`just remote-stop`; wait > 30 min; `just remote-test my-branch` → **no full
rebuild**, only changed crates recompile; `ssh b00t-build 'mountpoint -q
/mnt/cache'` true. `gcloud compute instances describe b00t-dstack-control` →
`TERMINATED` (reaper fired); the next `remote-*` transparently wakes it.
⚠️ This proof requires the `gcsfuse`/`zerofs` mount actually working — with the
local fallback the cache does NOT survive the stop.

## 7. Teardown

```sh
just remote-stop
dstack delete --all ; dstack fleet delete b00t-build-fleet   # DSTACK_PROJECT=main
ssh brianh@$CP_HOST 'sudo systemctl disable --now dstack-server dstack-idle-reaper.timer'
gcloud storage rm -r gs://b00t-buildcache-promptexecution/**      # the cache — rebuilds
cd b00t-tf && tofu destroy -target=module.gcp_build_plane          # takes the waker + SAs + VPC
# state bucket last, after `tofu init -migrate-state` back to local
```

## Cost & budget

**~$18–30/mo** (Spot build box): e2-small powered off when idle (~$2–3) + 10 GB
pd-standard (~$0.50) + waker (~$0) + **Spot** build box compute (~$6–10, ~2 h/day)
+ GCS cache (~$1–4) + egress (~$2–8). No persistent cache PD, no reserved IP.

Cost attribution: **ledgrrr FOCUS experiment `gcp-build-plane`** (billing account
`010CF0-14B757-AF79DC`, GCP project `promptexecution`, budget $30/mo, projected
$24/mo). Record actuals with
`mcp__ledgrrr__ledgerr_focus append_focus_record --experiment_id gcp-build-plane`
and reconcile against the GCP billing export. Standing the plane up +
Phase-8 acceptance on 2026-09-08 cost ≈ **$1** (build box on-demand ~1.6 h).
