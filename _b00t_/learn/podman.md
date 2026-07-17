# podman — Rootless Container Engine (b00t Gospel)

**podman** (v6.0.1+) provides rootless containers, pod management, and `podman kube play`
for deploying Kubernetes-style pod specs without a cluster.

## References
- [Repo: containers/podman](https://github.com/containers/podman)
- [Docs: docs.podman.io](https://docs.podman.io/)
- datum: `b00t install podman`

## Install

```bash
b00t install podman     # downloads v6.0.1 static binary to /usr/local/bin
b00t up podman          # checks for newer version, upgrades if needed
```

Install places `podman`, `podman-remote` in `/usr/local/bin`. The static binary
bundles crun, conmon, netavark, and aardvark-dns — no separate runtime packages needed.

### System upgrade (24.04 → 25.04)

Ubuntu 24.04 LTS ships kernel 6.8. Ubuntu 25.04 ships kernel 6.14 with:
- Improved io_uring (faster container I/O, async operations)
- eBPF verifier enhancements (better seccomp/syscall filtering)
- cgroup v2 improvements (more precise resource accounting)
- WSL3 kernel interfaces (GPU passthrough, nested virt, virtio-fs perf)

These kernel features directly benefit podman 6.0's new capabilities.
Upgrade when the operator is ready:

```bash
sudo do-release-upgrade -d    # to 24.10, then 25.04
# OR fresh install Ubuntu 25.04
```

## Usage

```bash
# Version and info
podman version
podman info

# Rootless operation (default — no sudo needed)
podman run --rm hello-world

# Pod management (Kubernetes-style)
podman pod ps
podman pod logs <pod-name>

# Kube play (deploy from YAML)
podman kube play deployment.yaml
podman kube down deployment.yaml

# Image management
podman pull ghcr.io/actions/actions-runner:latest
podman image ls

# Cleanup
podman system prune -af
```

## Podman 6.0 Features

Released July 2026. Key improvements over 4.x:

- `podman kube play` supports `secretKeyRef` for env var injection (used by gh-runner)
- Improved `--secret` handling for podman secrets
- Better `hostPath` volume permission handling
- Rootless networking improvements (pasta default)
- Faster pod startup (parallel container initialization)
- Quadlet support (systemd-generator for podman)

## Secret Management

```bash
# Create secret (from stdin)
echo "my-token" | podman secret create mysecret -

# List secrets
podman secret ls

# Use with kube play
podman kube play --secret mysecret pod.yaml

# Remove
podman secret rm mysecret
```

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| `podman: not found` | `/usr/local/bin` not in PATH with sudo; use full path |
| Rootless fails | `podman system migrate` after kernel upgrade |
| `kube play` permission denied | Check SELinux labels on hostPath volumes |
| Image pull timeout | `podman pull --retry 3 <image>` |
| Secret not mounting | `podman secret ls` — verify secret exists before `kube play` |
| OBS package conflict | `sudo apt remove --purge podman` before static install |

## Kernel Requirements

Minimum kernel 5.13 for cgroup v2. Recommended kernel 6.8+ for io_uring.
Optimal kernel 6.14+ (Ubuntu 25.04) for full podman 6.0 feature set.

```bash
uname -r          # check kernel version
podman info --format '{{.Host.Kernel}}'  # podman's view of kernel
```

---
CDI GPU injection breaks after reboot when /dev/dri renumbers (card1→card0): /etc/cdi/nvidia.yaml goes stale and needs root. Rootless fix: nvidia-ctk cdi generate --output=~/.config/containers/cdi/nvidia.yaml — containers.conf already sets cdi_spec_dirs to that path. Regenerate after every reboot; symptom is 'failed to stat CDI host device /dev/dri/cardN'
