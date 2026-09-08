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

## 2. Control node (pyinfra)

```sh
CP_HOST=$(cd b00t-tf && tofu output -raw gcp_control_node_external_ip) \
  pyinfra --dry nats/pyinfra/inventory_cp.py nats/pyinfra/deploy_cp_node.py \
    --data gcp_project_id=promptexecution \
    --data gcp_region=australia-southeast1 \
    --data build_vm_sa_email=$(cd b00t-tf && tofu output -raw gcp_build_vm_sa_email) \
    --data vpc_name=$(cd b00t-tf && tofu output -raw gcp_build_plane_network)
# then drop --dry
```

Installs `dstack[all]==0.20.28`, renders `~/.dstack/server/config.yml` (GCP
backend, metadata ADC — no keys), the `dstack-server` systemd unit, and the
`dstack-idle-reaper` timer (powers the VM off when dstack is idle).

Verify: `ssh brianh@$CP_HOST 'systemctl is-active dstack-server'` → `active`.

## 3. Point the local dstack CLI at the waker

```sh
dstack project add --name b00t \
  --url $(cd b00t-tf && tofu output -raw gcp_control_node_endpoint) \
  --token <admin token from ~/.dstack/server/config.yml on the control node>
dstack project set-default b00t
```

First call wakes the VM (~30–60 s). Break-glass without the waker (SSH via IAP —
**no static or external IP needed**; the reaper churns the ephemeral one so
never cache an address):
```sh
gcloud compute ssh b00t-dstack-control --zone <zone> --tunnel-through-iap \
  -- -L 3000:127.0.0.1:3000
```
IAP needs `roles/iap.tunnelResourceAccessor` — set `ssh_iap_members` in
`b00t-tf/.env` / the module before apply, or grant it yourself:
`gcloud projects add-iam-policy-binding promptexecution --member=user:you@… --role=roles/iap.tunnelResourceAccessor`.

## 4. Secrets + provision

```sh
just remote-bootstrap ~/.ssh/b00t_build_deploy_key '<cache-password>'
just remote-provision      # fleet + dev-environment; populates the `b00t-build` ssh alias
```

## 5. Use it

```sh
just remote-push my-branch
just remote-test my-branch   # first run cold; every run after is warm (cache in GCS)
just remote-keepalive        # optional — hold the control plane awake for a long build
```

## 6. Acceptance proof

`just remote-stop`; wait > 30 min; `just remote-test my-branch` → **no full
rebuild**, only changed crates recompile; `ssh b00t-build 'mountpoint -q
/mnt/cache'` true. `gcloud compute instances describe b00t-dstack-control` →
`TERMINATED` (reaper fired); the next `remote-*` transparently wakes it.

## 7. Teardown

```sh
just remote-stop
dstack -p b00t delete --all ; dstack -p b00t fleet delete b00t-build-fleet
ssh brianh@$CP_HOST 'sudo systemctl disable --now dstack-server dstack-idle-reaper.timer'
gcloud storage rm -r gs://b00t-buildcache-promptexecution/**      # the cache — rebuilds
cd b00t-tf && tofu destroy -target=module.gcp_build_plane          # takes the waker + SAs + VPC
# state bucket last, after `tofu init -migrate-state` back to local
```

## Cost

~$18–30/mo (Spot build box): e2-small powered off when idle (~$2–3) + 10 GB
pd-standard (~$0.50) + waker (~$0) + **Spot** build box compute (~$6–10, ~2 h/day;
`spot_policy: auto` falls back to on-demand only when Spot has no capacity) +
GCS cache (~$1–4) + egress (~$2–8). No persistent cache PD, no reserved IP.
