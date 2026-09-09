# dstack container runtime: Podman? and k0s integration

**Date:** 2026-09-09 · plan gleaming-jingling-nygaard · follow-up to the
build-plane leverage pass and the snapshotter eval.
**Questions (operator):** (1) can dstack run under Podman instead of Docker?
(2) we already run **k0s** on b00t-node in Vultr — can dstack connect to it?

## TL;DR

- **Podman on VM/SSH fleets: unsupported, and only *maybe* works for CPU
  workloads.** `dstack-shim` talks the **Docker Engine API** via
  `docker.NewClientWithOpts(docker.FromEnv, …)` — it honours `DOCKER_HOST`, so
  pointing it at `podman system service`'s Docker-compatible socket is
  *plausible* for plain CPU tasks (our Rust build plane). GPU is the risk:
  the shim drives Docker's `container.DeviceRequest` API, whose Podman
  Docker-compat translation is historically incomplete. Zero references to
  podman in the dstack source, no tracking issue, docs say *"Hosts must be
  Linux-based and have Docker pre-installed."* → treat as an unsupported spike.
- **k0s: yes, cleanly — via dstack's first-class `kubernetes` backend.** Give
  the dstack server b00t-node's kubeconfig context + one node as SSH jump host.
  dstack then schedules **Pods** through the k8s API; the node runtime is k0s's
  bundled **containerd** — **Docker never enters the picture.** This is also the
  path that unlocks **SOCI** lazy pull from the snapshotter eval (we'd own
  containerd's snapshotter config on the k0s nodes).

**Recommendation:** don't chase Podman-on-VMs. If "no Docker" is a hard
requirement, move the plane (or a second backend) to the **`kubernetes`
backend against k0s**. Keep the GCP VM backend as-is for burst capacity where
Docker-on-a-throwaway-Spot-VM is acceptable.

## Podman — detail

| Fact | Source |
|---|---|
| `dstack-shim` init: `docker.NewClientWithOpts(docker.FromEnv, docker.WithAPIVersionNegotiation())` | `runner/internal/shim/docker.go` |
| Uses the Docker Go client API (`ContainerCreate`, `ImagePull`, …), never the `docker` CLI | same |
| GPU via `container.DeviceRequest` (NVIDIA/AMD/Tenstorrent/Habana device mapping) | same |
| No podman references anywhere in the repo; no open issue | `repo:dstackai/dstack podman` → 0 |
| SSH-fleet host requirement: *"have Docker pre-installed"* | dstack docs, Fleets |

**What a Podman spike would look like** (if pursued, CPU-only build plane):

```sh
# on the fleet host
systemctl --user enable --now podman.socket           # or rootful /run/podman/podman.sock
export DOCKER_HOST=unix://$XDG_RUNTIME_DIR/podman/podman.sock
# install podman-docker (provides the `docker` shim some probes call)
```

Then register the host as an SSH fleet and run a **CPU** task. Likely failure
points: image-pull auth against Artifact Registry, `HostConfig` fields the
Podman API rejects, and anything GPU. Not worth it unless "no Docker" is
non-negotiable *and* k8s is off the table.

## k0s — detail

dstack's `kubernetes` backend (GA-ish; "orchestrating GPUs on Kubernetes"):

- One backend entry can manage one or many clusters; each selected by a
  **kubeconfig context**.
- dstack picks **one node as an SSH jump host** to proxy SSH into Pods —
  auto-configured, no manual proxy setup. Needs that node's IP reachable from
  the dstack server.
- Runs on **pre-provisioned nodes** only (cluster autoscale "coming soon") —
  fine: b00t-node is a fixed box.
- Fleets with `placement: cluster` → distributed tasks / dev-envs / services.

Server config sketch (`~/.dstack/server/config.yml` on the control node):

```yaml
projects:
  - name: main
    backends:
      - type: kubernetes
        kubeconfig:
          filename: /root/.dstack/kube/b00t-node.kubeconfig   # k0s admin kubeconfig
        proxy_jump:
          hostname: <b00t-node public IP>
          port: 22
```

Then `dstack apply` with a normal task/dev-env config schedules onto k0s.
`b00t-build:latest` in Artifact Registry stays the image (k0s containerd pulls
it; add an imagePullSecret or Workload Identity equivalent for AR).

### Why this is the better "no Docker" answer

- k0s bundles **containerd** — no Docker daemon on the node at all.
- We control the node → we can later add the **SOCI snapshotter** (eval doc
  recommendation) and get lazy image pull, which is impossible on the
  dockerd-based VM backend.
- Same control plane, UI, OTLP export, `just remote-*` ergonomics — only the
  backend entry changes.

### Open items for a k0s integration

- AR pull auth from Vultr k0s (no GCE metadata SA there): a static AR
  reader key as an `imagePullSecret`, or `gcr-cleaner`-style short-lived token
  refresher. 🚩 don't ship a long-lived JSON key — scope it to
  `artifactregistry.reader` on the one repo.
- Network path: dstack server (GCP control node) → b00t-node jump host SSH.
  Tailnet already planned (Phase 2.75) — use the tailnet IP as `proxy_jump.hostname`.
- k0s node sizing vs the Rust workspace cold build (needs ~8 vCPU / 32 GB /
  200 GB scratch for `--workspace`); b00t-node may be too small → k0s is better
  suited to the **compile-free test fan-out** (leverage #1) and smaller jobs,
  with GCP Spot still doing the heavy `build_archive`.

## Sources

- https://raw.githubusercontent.com/dstackai/dstack/master/runner/internal/shim/docker.go
- https://dstack.ai/docs/concepts/fleets/
- https://dstack.ai/docs/concepts/backends/
- https://dstack.ai/docs/guides/kubernetes/
- https://dstack.ai/blog/kubernetes-beta/
- https://www.golinuxcloud.com/podman-docker-socket/ — DOCKER_HOST / podman.sock
