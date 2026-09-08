# pyinfra inventory — the GCP dstack control node (plan gleaming-jingling-nygaard).
#
# The control node has NO static IP, and the idle-reaper stops/starts it so the
# ephemeral external IP changes. So this inventory RESOLVES the address live:
#
#   1. $CP_HOST if set        — explicit override (a tailnet MagicDNS name, an
#                               IAP ProxyCommand alias, whatever).
#   2. else `gcloud compute instances describe` — the current external IP.
#
#   pyinfra nats/pyinfra/inventory_cp.py nats/pyinfra/deploy_cp_node.py --data ...
#
# For a box with no external IP at all (network_mode=tailnet, or you prefer
# IAP), set CP_HOST to a Host alias whose ProxyCommand is
# `gcloud compute start-iap-tunnel b00t-dstack-control 22 --listen-on-stdin
# --zone <zone>` (or a tailnet name), and put ssh_user there.

import os
import shutil
import subprocess

CONTROL_INSTANCE = os.environ.get("CP_INSTANCE", "b00t-dstack-control")
CONTROL_ZONE = os.environ.get("CP_ZONE", "australia-southeast1-a")
SSH_USER = os.environ.get("CP_SSH_USER", "brianh")

_host = os.environ.get("CP_HOST", "").strip()

if not _host:
    if not shutil.which("gcloud"):
        raise SystemExit(
            "set CP_HOST=<ip or name>, or install gcloud so this inventory can "
            "resolve the control node's current (ephemeral) IP"
        )
    try:
        _host = subprocess.check_output(
            [
                "gcloud", "compute", "instances", "describe", CONTROL_INSTANCE,
                "--zone", CONTROL_ZONE,
                "--format=get(networkInterfaces[0].accessConfigs[0].natIP)",
            ],
            text=True,
        ).strip()
    except subprocess.CalledProcessError as e:
        raise SystemExit(f"gcloud could not describe {CONTROL_INSTANCE}: {e}")
    if not _host:
        raise SystemExit(
            f"{CONTROL_INSTANCE} has no external IP (tailnet mode?) — set CP_HOST "
            "to a tailnet name or an IAP ProxyCommand alias"
        )

cp_node = [
    (_host, {"ssh_user": SSH_USER}),
]
