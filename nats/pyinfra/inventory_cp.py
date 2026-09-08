# pyinfra inventory — the GCP dstack control node (plan gleaming-jingling-nygaard).
#
# Not a fixed IP: the control node is OpenTofu-managed and its external IP is
# ephemeral (network_mode="public") or absent (network_mode="tailnet"). Pass
# the target explicitly, sourced from `tofu output`:
#
#   CP_HOST=$(cd b00t-tf && tofu output -raw gcp_control_node_external_ip) \
#     pyinfra nats/pyinfra/inventory_cp.py nats/pyinfra/deploy_cp_node.py
#
# tailnet mode: CP_HOST=b00t-dstack-control (MagicDNS name), ssh via Tailscale.

import os

_host = os.environ.get("CP_HOST", "").strip()
if not _host:
    raise SystemExit(
        "set CP_HOST=<control-node ip or magicdns name> "
        "(cd b00t-tf && tofu output -raw gcp_control_node_external_ip)"
    )

cp_node = [
    (_host, {"ssh_user": os.environ.get("CP_SSH_USER", "brianh")}),
]
