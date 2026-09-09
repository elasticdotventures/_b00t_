#!/usr/bin/env bash
# Run ON b00t-node. Emits a kubeconfig for the local k0s cluster with the API
# server address rewritten to an address the dstack control node can reach
# (tailnet IP by default), so `dstack` on the GCP control node can schedule
# onto k0s. Plan gleaming-jingling-nygaard — k0s/SOCI follow-up.
#
#   sudo fetch-k0s-kubeconfig.sh [<reachable-api-host>] > b00t-node.kubeconfig
#
# <reachable-api-host> defaults to the node's tailscale0 IP. The k0s API server
# must be listening on / reachable at that address (k0s `api.sans` /
# `api.address`), and 6443 open to the control node.
set -euo pipefail

host="${1:-$(tailscale ip -4 2>/dev/null | head -1 || true)}"
[ -n "$host" ] || { echo "no reachable API host given and no tailscale0 IP found" >&2; exit 1; }

# k0s ships its own kubeconfig generator; controller+worker single node uses
# `k0s kubeconfig admin`.
raw="$(k0s kubeconfig admin 2>/dev/null || sudo k0s kubeconfig admin)"

printf '%s\n' "$raw" | python3 -c '
import sys, re
cfg = sys.stdin.read()
host = sys.argv[1]
# swap only the server: host:port, keep scheme + port
cfg = re.sub(r"(server:\s*https://)[^:/\s]+(:\d+)", r"\g<1>" + host + r"\g<2>", cfg)
sys.stdout.write(cfg)
' "$host"
