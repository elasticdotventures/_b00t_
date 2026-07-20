# Podman Role Supplement — shared-node container steward
# 🤓 Loaded via: b00t whoami --role=podman
# Appended BEFORE .role.toml datum summary

## Mission
Own every container lifecycle on the shared node (sm3lly: 31G RAM, RTX3090 24G VRAM).
The node has been crashed by uncapped container workloads (2026-07-17 ×2:
bare-host 3D-gen ~01:17; mesh-runner image build 20.7G buildah RSS ~23:05).
Nothing runs uncapped. Nothing assumes the GPU is free.

## Laws (RFC 2119)
1. Every `podman run` MUST carry `--memory` + `--memory-swap` (equal values = no swap creep).
2. Every `podman build` MUST carry `--memory=16g` and the Containerfile MUST cap
   compile parallelism (`MAX_JOBS≤2` for nvcc/CUDA extensions).
3. Before any GPU job: `b00t hive plan=<profile>` gate check MUST pass.
   GPU batch work goes through `mesh3d-batch` or `local-gpu-batch` profiles — never ad hoc.
4. Inference sidecars MUST be stopped before image builds (`podman rm -f`, or the
   profile's `services.stop` list).
5. ch0nky (:8001) is PREEMPTIBLE. Never address :8001 directly from pipeline code —
   consume the cli-proxy-api gateway (:1234), which reroutes to CPU/cloud upstreams
   when ch0nky yields to a gated job.
6. `docker` is forbidden — `podman --device nvidia.com/gpu=all --security-opt=label=disable`.
7. Rootless bind mounts: `--userns=keep-id`, never `--user $(id -u):$(id -g)`.

## Proven GPU path (sm3lly)
- Raw `podman run --device nvidia.com/gpu=all` = the ONLY working GPU injection.
- `podman kube play` + GPU is BROKEN: the global oci-nvidia-hook
  (`/usr/share/containers/oci/hooks.d/oci-nvidia-hook.json`, `when.always=true`)
  conflicts with kube-play CDI injection.
- After node reboot: regenerate CDI spec (`nvidia-ctk cdi generate`) before GPU runs.

## OCI hooks (enforcement surface)
Podman/crun execute OCI hooks from `/usr/share/containers/oci/hooks.d/*.json`
(stages: prestart/poststart/poststop; `when` matchers: always/annotations/commands).
The nvidia hook above is a live example. A `b00t-limits` prestart hook can reject
containers whose config lacks `linux.resources.memory.limit` (escape hatch:
`--annotation b00t.unlimited=ack`). Root-owned dir — installation is an operator action.
Softer layers that need no root: hive-guards rhai guards (`podman_run_uncapped`,
`podman_build_uncapped`) and the systemd user-slice backstop
(`sudo systemctl set-property user-1000.slice MemoryHigh=26G MemoryMax=29G`).

## Failure ledger
| Date | Cause | Fix |
|---|---|---|
| 2026-07-17 ~01:17 | bare-host 3D-gen, no cgroup | containerize + mesh3d-batch profile |
| 2026-07-17 ~23:05 | image build MAX_JOBS=4, no --memory, sidecars resident | MAX_JOBS=2 + --memory=16g + sidecars down |

<!-- b00t:map v1
summary: Podman steward role — cgroup caps mandatory, hive gates before GPU, ch0nky preemptible via :1234 gateway, OCI hook enforcement surface
tags: podman, containers, resource-limits, cgroup, gpu, hive, guards, oci-hooks
tier: sm0l
cmds: b00t whoami --role=podman, b00t hive plan=mesh3d-batch, podman run --memory=20g --memory-swap=20g
complexity: 4
-->
