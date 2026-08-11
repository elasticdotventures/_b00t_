#!/usr/bin/env bash
set -euo pipefail

# Regenerates ~/.b00t/secrets/{nats-user,nats-password} from the b00t ACP
# NATS bus's own config (~/.b00t/nats/nats.conf, systemd unit
# nats-server.service). This is the shared LAN agent-coordination bus
# (fung1 + sm3lly) — app4dog is a promptexecution sub-project and owns no
# infrastructure of its own, so it authenticates against this bus rather
# than running its own NATS. Never hand-edit the secret files; re-run this
# after nats.conf's credentials change.

NATS_CONF="${HOME}/.b00t/nats/nats.conf"
SECRETS_DIR="${HOME}/.b00t/secrets"

if [ ! -f "$NATS_CONF" ]; then
  echo "sync-nats-secrets: $NATS_CONF not found — is the b00t NATS bus configured on this host?" >&2
  exit 1
fi

user=$(grep -oP '(?<=user:\s")[^"]+' "$NATS_CONF" | head -1)
password=$(grep -oP '(?<=password:\s")[^"]+' "$NATS_CONF" | head -1)

if [ -z "$user" ] || [ -z "$password" ]; then
  echo "sync-nats-secrets: could not parse user/password out of $NATS_CONF" >&2
  exit 1
fi

mkdir -p "$SECRETS_DIR"
printf '%s' "$user" > "$SECRETS_DIR/nats-user"
printf '%s' "$password" > "$SECRETS_DIR/nats-password"
chmod 600 "$SECRETS_DIR/nats-user" "$SECRETS_DIR/nats-password"

echo "sync-nats-secrets: wrote $SECRETS_DIR/nats-user, $SECRETS_DIR/nats-password"
