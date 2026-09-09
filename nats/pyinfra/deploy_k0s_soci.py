"""
Install soci-snapshotter on b00t-node and wire it into k0s's containerd, so the
kubernetes dstack backend gets lazy image pull. Plan gleaming-jingling-nygaard —
k0s/SOCI follow-up to the build-plane leverage pass.

Idempotent. Targets b00t-node (the k0s box in Vultr), NOT the GCP control node.

  pyinfra --dry <inventory-with-b00t-node> nats/pyinfra/deploy_k0s_soci.py \
      --data soci_version=0.15.0 \
      --data k0s_role=worker            # or "controller" for a single combined node

Verify after apply (on b00t-node):
  systemctl status soci-snapshotter-grpc
  k0s ctr plugin ls | grep soci
  k0s ctr --namespace k8s.io snapshots --snapshotter soci list

References:
  https://github.com/awslabs/soci-snapshotter
  docs/superpowers/specs/2026-09-09-lazy-pull-snapshotter-eval.md
  docs/superpowers/specs/2026-09-09-dstack-runtime-podman-k0s.md
"""

from pyinfra import host
from pyinfra.facts.files import File
from pyinfra.operations import files, server, systemd

SOCI_VERSION = str(host.data.get("soci_version", "0.15.0"))
K0S_ROLE = host.data.get("k0s_role", "worker")  # worker | controller
K0S_SERVICE = "k0scontroller" if K0S_ROLE == "controller" else "k0sworker"

TARBALL = f"soci-snapshotter-{SOCI_VERSION}-linux-amd64.tar.gz"
URL = (
    "https://github.com/awslabs/soci-snapshotter/releases/download/"
    f"v{SOCI_VERSION}/{TARBALL}"
)

# ── 1. fuse3 (soci mounts FUSE filesystems for lazy layers) ──────────────
server.packages(
    name="Install fuse3",
    packages=["fuse3"],
    _sudo=True,
)

# ── 2. soci + soci-snapshotter-grpc binaries ────────────────────────────
if not host.get_fact(File, path="/usr/local/bin/soci-snapshotter-grpc"):
    server.shell(
        name=f"Download + install soci-snapshotter {SOCI_VERSION}",
        commands=[
            f"curl -fsSL -o /tmp/{TARBALL} {URL}",
            f"tar -C /usr/local/bin -xzf /tmp/{TARBALL} soci soci-snapshotter-grpc",
            "chmod 0755 /usr/local/bin/soci /usr/local/bin/soci-snapshotter-grpc",
            f"rm -f /tmp/{TARBALL}",
            "/usr/local/bin/soci-snapshotter-grpc --version",
        ],
        _sudo=True,
    )

files.directory(
    name="Create soci state + config dirs",
    path="/var/lib/soci-snapshotter-grpc",
    _sudo=True,
)
files.directory(name="Create /etc/soci-snapshotter-grpc", path="/etc/soci-snapshotter-grpc", _sudo=True)

files.put(
    name="Install soci-snapshotter-grpc config",
    src="files/soci-snapshotter-config.toml",
    dest="/etc/soci-snapshotter-grpc/config.toml",
    _sudo=True,
)

# ── 3. systemd unit for the snapshotter daemon ─────────────────────────
snap_unit = files.put(
    name="Install soci-snapshotter-grpc.service",
    src="files/soci-snapshotter-grpc.service",
    dest="/etc/systemd/system/soci-snapshotter-grpc.service",
    _sudo=True,
)
systemd.service(
    name="Enable + start soci-snapshotter-grpc",
    service="soci-snapshotter-grpc.service",
    running=True,
    enabled=True,
    daemon_reload=True,
    restarted=snap_unit.changed,
    _sudo=True,
)

# ── 4. k0s containerd drop-in + restart k0s ────────────────────────────
files.directory(name="Create /etc/k0s/containerd.d", path="/etc/k0s/containerd.d", _sudo=True)
cri_dropin = files.put(
    name="Install k0s containerd SOCI drop-in",
    src="files/k0s-containerd-soci.toml",
    dest="/etc/k0s/containerd.d/soci.toml",
    _sudo=True,
)
systemd.service(
    name=f"Restart {K0S_SERVICE} to pick up the SOCI snapshotter",
    service=f"{K0S_SERVICE}.service",
    running=True,
    enabled=True,
    restarted=cri_dropin.changed,
    _sudo=True,
)

# ── 5. post-check ─────────────────────────────────────────────────────
server.shell(
    name="Post-check: soci registered in containerd",
    commands=[
        "systemctl is-active soci-snapshotter-grpc",
        "sleep 3",
        "k0s ctr plugin ls 2>/dev/null | grep -E 'soci|snapshot' || true",
    ],
    _sudo=True,
)
