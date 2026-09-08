"""
Provision the GCP dstack control node. Plan gleaming-jingling-nygaard, Phase 3.

Idempotent — safe to re-run. Deliberately minimal: this box runs ONE thing,
`dstack server`, plus a systemd timer that powers the box off when dstack is
idle (Phase 2.5). No NATS, no k0s, no Rust toolchain.

Auth: the dstack GCP backend authenticates as the VM's attached service
account via the metadata server (`creds: type: default` in the rendered
config) — no key file, no WIF for the server. RunPod is DEFERRED.

Prereqs (OpenTofu, Phase 2): `just gcp-apply` has created the control node,
the b00t-build-vm SA, the buildcache bucket and the VPC.

Usage:
    CP_HOST=$(cd b00t-tf && tofu output -raw gcp_control_node_external_ip) \\
      pyinfra nats/pyinfra/inventory_cp.py nats/pyinfra/deploy_cp_node.py \\
        --data gcp_project_id=promptexecution \\
        --data gcp_region=australia-southeast1 \\
        --data build_vm_sa_email=$(cd b00t-tf && tofu output -raw gcp_build_vm_sa_email) \\
        --data vpc_name=$(cd b00t-tf && tofu output -raw gcp_build_plane_network 2>/dev/null || echo '')

    # dry run first:
    pyinfra --dry nats/pyinfra/inventory_cp.py nats/pyinfra/deploy_cp_node.py --data ...
"""

from pyinfra import host
from pyinfra.facts.files import File
from pyinfra.operations import apt, files, server, systemd

DSTACK_VERSION = "0.20.28"
# "main" = dstack's default project name (the server auto-creates it and writes
# the CLI's ~/.dstack/config.yml against it). Standardize on it everywhere —
# the reaper, the post-check, and the operator's `dstack project add main ...`.
DSTACK_PROJECT = "main"
SSH_USER = "brianh"
HOME = f"/home/{SSH_USER}"

gcp_project_id = host.data.get("gcp_project_id", "promptexecution")
gcp_region = host.data.get("gcp_region", "australia-southeast1")
build_vm_sa_email = host.data.get("build_vm_sa_email")
vpc_name = host.data.get("vpc_name", "")
idle_grace_min = str(host.data.get("idle_grace_min", 45))

if not build_vm_sa_email:
    raise ValueError(
        "pass --data build_vm_sa_email=<the b00t-build-vm SA email> "
        "(cd b00t-tf && tofu output -raw gcp_build_vm_sa_email)"
    )

# ─── 1. Base packages ──────────────────────────────────────────────────────
# git: dstack's CLI + server import GitPython, which hard-fails at import time
# without a `git` binary on PATH ("Bad git executable"). iproute2: the reaper's
# `ss`. curl: uv installer + health probes.
apt.packages(
    name="Install git + curl + python3 + iproute2 (reaper needs `ss`)",
    packages=["git", "curl", "python3", "iproute2", "ca-certificates"],
    update=True,
    _sudo=True,
)

# ─── 2. uv + dstack (as the login user, not root) ─────────────────────────
if not host.get_fact(File, path=f"{HOME}/.local/bin/uv"):
    server.shell(
        name="Install uv",
        commands=["curl -LsSf https://astral.sh/uv/install.sh | sh"],
    )

server.shell(
    name=f"Install dstack[all]=={DSTACK_VERSION}",
    commands=[
        f"{HOME}/.local/bin/uv tool install --force 'dstack[all]=={DSTACK_VERSION}'"
    ],
)

# ─── 3. dstack server config (GCP backend only, metadata ADC) ─────────────
files.directory(
    name="Create ~/.dstack/server",
    path=f"{HOME}/.dstack/server",
    user=SSH_USER,
    group=SSH_USER,
)
files.template(
    name="Render ~/.dstack/server/config.yml (0600)",
    src="templates/dstack-server-config.yml.j2",
    dest=f"{HOME}/.dstack/server/config.yml",
    mode="600",
    user=SSH_USER,
    group=SSH_USER,
    dstack_project=DSTACK_PROJECT,
    gcp_project_id=gcp_project_id,
    gcp_region=gcp_region,
    build_vm_sa_email=build_vm_sa_email,
    vpc_name=vpc_name,
)

# ─── 4. dstack-server.service ────────────────────────────────────────────
files.put(
    name="Install dstack-server.service",
    src="files/dstack-server.service",
    dest="/etc/systemd/system/dstack-server.service",
    _sudo=True,
)
systemd.service(
    name="Enable + start dstack-server",
    service="dstack-server.service",
    running=True,
    enabled=True,
    restarted=True,  # pick up a re-rendered config on re-run
    daemon_reload=True,
    _sudo=True,
)

# ─── 5. Local dstack CLI project (so the reaper can introspect) ──────────
files.put(
    name="Install configure-local-dstack.sh",
    src="files/configure-local-dstack.sh",
    dest="/usr/local/bin/configure-local-dstack.sh",
    mode="755",
    _sudo=True,
)
server.shell(
    name="Configure local dstack CLI project",
    commands=[
        f"DSTACK_BIN={HOME}/.local/bin/dstack DSTACK_PROJECT={DSTACK_PROJECT} "
        f"HOME={HOME} bash /usr/local/bin/configure-local-dstack.sh"
    ],
)

# ─── 6. Idle reaper (powers the box off when dstack is idle) ─────────────
# The script reads its tunables from the environment; the unit (below)
# supplies them, so the script itself is a plain put with no templating.
files.put(
    name="Install b00t-cp-idle-reaper.sh",
    src="files/b00t-cp-idle-reaper.sh",
    dest="/usr/local/bin/b00t-cp-idle-reaper.sh",
    mode="755",
    _sudo=True,
)
files.template(
    name="Render dstack-idle-reaper.service (0644)",
    src="templates/dstack-idle-reaper.service.j2",
    dest="/etc/systemd/system/dstack-idle-reaper.service",
    home=HOME,
    idle_grace_min=idle_grace_min,
    dstack_project=DSTACK_PROJECT,
    dstack_bin=f"{HOME}/.local/bin/dstack",
    _sudo=True,
)
files.put(
    name="Install dstack-idle-reaper.timer",
    src="files/dstack-idle-reaper.timer",
    dest="/etc/systemd/system/dstack-idle-reaper.timer",
    _sudo=True,
)
systemd.service(
    name="Enable + start dstack-idle-reaper.timer",
    service="dstack-idle-reaper.timer",
    running=True,
    enabled=True,
    daemon_reload=True,
    _sudo=True,
)

# ─── 7. Post-checks ───────────────────────────────────────────────────────
server.shell(
    name="Post-check: dstack server up + GCP backend healthy",
    commands=[
        f"{HOME}/.local/bin/dstack --version",
        "curl -sf http://127.0.0.1:3000/ >/dev/null && echo 'dstack server: responding'",
        f"{HOME}/.local/bin/dstack fleet --project {DSTACK_PROJECT} 2>&1 | head -5 || true",
    ],
)
