#!/bin/bash
# Deploy b00t-forge-kv to a Vultr node as a Redis/Valkey replacement,
# matching vultr-node-setup.sh's security model exactly: binds 127.0.0.1
# only, reached over the same SSH tunnel already in place for NATS — never
# exposed publicly, and agents never connect to it directly (only whatever
# local service needs a KV/pub-sub backing store does).
#
# Builds a static (musl) binary locally rather than building on the remote
# node or shipping a glibc-linked one: musl sidesteps glibc-version skew
# between this build host and the target Debian release entirely, and Vultr
# nodes in this hive are provisioned lean (no Rust toolchain) per the
# existing nats-server deploy pattern (prebuilt binary, not built in place).
#
# Usage: ./vultr-forge-kv-deploy.sh <ssh-host-alias> [port]
set -euo pipefail

HOST="${1:?usage: vultr-forge-kv-deploy.sh <ssh-host-alias> [port]}"
PORT="${2:-6379}"

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="x86_64-unknown-linux-musl"

echo "== Building b00t-forge-kv (release, static musl) =="
if ! rustup target list --installed | grep -q "$TARGET"; then
  rustup target add "$TARGET"
fi
( cd "$REPO_ROOT" && cargo build --release --target "$TARGET" -p b00t-forge-kv )
BINARY="$REPO_ROOT/target/$TARGET/release/b00t-forge-kv"
[[ -x "$BINARY" ]] || { echo "❌ build did not produce $BINARY" >&2; exit 1; }

echo "== Checking what's currently on :$PORT on $HOST =="
if ssh "$HOST" "systemctl is-active redis-server 2>/dev/null || systemctl is-active valkey-server 2>/dev/null" | grep -q active; then
  echo "⚠️  An active redis-server/valkey-server unit was found on $HOST."
  echo "    This script does not stop/uninstall it for you — confirm nothing"
  echo "    else depends on it, then stop it manually before (or after)"
  echo "    enabling the b00t-forge-kv unit below, so only one process ever"
  echo "    binds 127.0.0.1:$PORT."
fi

echo "== Copying binary to $HOST =="
scp "$BINARY" "$HOST:/tmp/b00t-forge-kv"
ssh "$HOST" "sudo install -m 755 /tmp/b00t-forge-kv /usr/local/bin/b00t-forge-kv && rm -f /tmp/b00t-forge-kv"

echo "== Installing systemd unit (127.0.0.1:$PORT only — never public) =="
ssh "$HOST" "sudo tee /etc/systemd/system/b00t-forge-kv.service > /dev/null" <<EOF
[Unit]
Description=b00t-forge-kv — RESP2 KV server (Redis/Valkey replacement)
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/usr/local/bin/b00t-forge-kv --host 127.0.0.1 --port ${PORT}
Restart=always
RestartSec=5
DynamicUser=yes

[Install]
WantedBy=multi-user.target
EOF

ssh "$HOST" "sudo systemctl daemon-reload && sudo systemctl enable --now b00t-forge-kv && sudo systemctl status b00t-forge-kv --no-pager -l | head -10"

echo ""
echo "== Smoke test (from $HOST itself — the port is not reachable from here) =="
ssh "$HOST" "printf '*1\r\n\$4\r\nPING\r\n' | timeout 2 bash -c 'cat > /dev/tcp/127.0.0.1/${PORT}; cat < /dev/tcp/127.0.0.1/${PORT}' 2>&1 | head -c 20 || echo 'FAILED — inspect with: ssh $HOST journalctl -u b00t-forge-kv -n 50'"

echo ""
echo "== Done =="
echo "b00t-forge-kv is listening on ${HOST}:127.0.0.1:${PORT} (localhost-only, same"
echo "reachability model as the existing NATS leaf: over whatever SSH tunnel"
echo "already reaches this host, never public)."
echo ""
echo "Once confirmed healthy and nothing depends on the old redis-server unit:"
echo "  ssh $HOST 'sudo systemctl disable --now redis-server'  # or valkey-server"
