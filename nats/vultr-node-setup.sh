#!/bin/bash
# Configure a fresh Vultr VPS as a NATS leaf-accept peer for the b00t hive.
#
# Unlike fung1 (home NAT, no public IP), the Vultr node has a public IP —
# but per the pre-existing "b00t-node" firewall-group convention (only
# 22/80/443 public, reserved for SSH/pingap; NATS/Dapr stay localhost-only),
# the leaf listener binds to 127.0.0.1 only, NOT 0.0.0.0. The LAN hub reaches
# it via an SSH tunnel over port 22 (already open), not direct public
# exposure — so no TLS is needed here, the SSH tunnel already encrypts the
# transport. This node uses its own distinct credential (not the LAN-only
# b00t-hive-lan secret in ~/.dotfiles/nats/nats.conf). The LAN hub then dials
# OUT to it (via the tunnel) as a leafnodes.remotes entry (separate step —
# see ~/.dotfiles/secrets/nats-leaf-<host>.conf convention).
#
# Usage: ./vultr-node-setup.sh <ssh-host-alias> [leaf-port]
set -euo pipefail

HOST="${1:?usage: vultr-node-setup.sh <ssh-host-alias> [leaf-port]}"
LEAF_PORT="${2:-443}"
LEAF_USER="b00t-leaf"
LEAF_PASSWORD="$(openssl rand -hex 24)"

echo "== Installing nats-server on $HOST =="
LATEST_TAG=$(ssh "$HOST" "curl -sL https://api.github.com/repos/nats-io/nats-server/releases/latest | grep -m1 '\"tag_name\"' | cut -d'\"' -f4")
echo "latest nats-server release: $LATEST_TAG"
ssh "$HOST" "set -e
  curl -sL https://github.com/nats-io/nats-server/releases/download/${LATEST_TAG}/nats-server-${LATEST_TAG}-linux-amd64.tar.gz -o /tmp/nats-server.tar.gz
  tar -xzf /tmp/nats-server.tar.gz -C /tmp
  install -m 755 /tmp/nats-server-${LATEST_TAG}-linux-amd64/nats-server /usr/local/bin/nats-server
  nats-server --version
  mkdir -p /etc/nats"

echo "== Writing /etc/nats/nats.conf (leaf-accept mode, localhost-only, no TLS) =="
ssh "$HOST" "cat > /etc/nats/nats.conf" <<EOF
# b00t ACP NATS server — Vultr leaf-accept node. Per the pre-existing
# "b00t-node" firewall-group convention (only 22/80/443 are meant to be
# public, reserved for SSH/pingap; NATS/Dapr stay localhost-only), the
# leafnode listener binds to 127.0.0.1 only. The LAN hub reaches it via an
# SSH tunnel over port 22 (already open), not direct public exposure — no
# TLS needed here since the SSH tunnel already encrypts the transport.
port: 4222
http_port: 8222

authorization {
  users = [
    { user: "${LEAF_USER}", password: "${LEAF_PASSWORD}" }
  ]
}

leafnodes {
  host: "127.0.0.1"
  port: ${LEAF_PORT}
}

debug: false
trace: false
EOF

echo "== Installing systemd unit =="
ssh "$HOST" "cat > /etc/systemd/system/nats-server.service" <<'EOF'
[Unit]
Description=b00t NATS — leaf-accept node
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/nats-server --config /etc/nats/nats.conf
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

ssh "$HOST" "systemctl daemon-reload && systemctl enable --now nats-server && systemctl status nats-server --no-pager -l | head -10"

PUBLIC_IP=$(ssh "$HOST" "curl -s https://api.ipify.org")

echo ""
echo "== Done =="
echo "Public IP:      ${PUBLIC_IP} (leaf listener is NOT exposed here — 127.0.0.1 only)"
echo "Leaf port:      ${LEAF_PORT} (localhost-only, no TLS)"
echo "Leaf user:      ${LEAF_USER}"
echo "Leaf password:  ${LEAF_PASSWORD}"
echo ""
echo "Next steps:"
echo "  1. Save the leaf credential (e.g. into kv-pe-foundry) — it is not written anywhere else."
echo "  2. On the LAN hub, write ~/.dotfiles/secrets/nats-leaf-${HOST}.conf:"
echo "       leafnodes { remotes = [ { url: \"nats://${LEAF_USER}:${LEAF_PASSWORD}@127.0.0.1:<local-tunnel-port>\" } ] }"
echo "     and include it from nats.conf (NATS \$VAR substitution does not work"
echo "     inside quoted compound strings like this URL — use 'include', not env vars)."
echo "  3. Set up a persistent SSH tunnel from the LAN hub to ${HOST}:${LEAF_PORT}"
echo "     (systemd unit, ExecStart=ssh -N -L <lan-ip>:<local-tunnel-port>:127.0.0.1:${LEAF_PORT} ${HOST})."
