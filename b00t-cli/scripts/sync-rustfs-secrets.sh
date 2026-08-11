#!/usr/bin/env bash
set -euo pipefail

# Regenerates ~/.b00t/secrets/{rustfs-access-key,rustfs-secret-key} from the
# RustFS b00t datum's own config (~/.b00t/_b00t_/rustfs.docker.toml). Never
# hand-edit the secret files; re-run this after rustfs.docker.toml's
# credentials change. SecretSource::File can only extract a whole file as
# one opaque string, so RustFS's two credentials (one TOML file, two keys)
# need two separate small files rather than one SecretRef.

RUSTFS_TOML="${HOME}/.b00t/_b00t_/rustfs.docker.toml"
SECRETS_DIR="${HOME}/.b00t/secrets"

if [ ! -f "$RUSTFS_TOML" ]; then
  echo "sync-rustfs-secrets: $RUSTFS_TOML not found — is the rustfs b00t datum installed?" >&2
  exit 1
fi

creds="$(python3 -c "
import tomllib
with open('$RUSTFS_TOML', 'rb') as f:
    data = tomllib.load(f)
env = data['b00t']['env']
print(env['RUSTFS_ACCESS_KEY'])
print(env['RUSTFS_SECRET_KEY'])
")"
access_key="$(echo "$creds" | sed -n '1p')"
secret_key="$(echo "$creds" | sed -n '2p')"

if [ -z "$access_key" ] || [ -z "$secret_key" ]; then
  echo "sync-rustfs-secrets: could not parse RUSTFS_ACCESS_KEY/RUSTFS_SECRET_KEY out of $RUSTFS_TOML" >&2
  exit 1
fi

mkdir -p "$SECRETS_DIR"
printf '%s' "$access_key" > "$SECRETS_DIR/rustfs-access-key"
printf '%s' "$secret_key" > "$SECRETS_DIR/rustfs-secret-key"
chmod 600 "$SECRETS_DIR/rustfs-access-key" "$SECRETS_DIR/rustfs-secret-key"

echo "sync-rustfs-secrets: wrote $SECRETS_DIR/rustfs-access-key, $SECRETS_DIR/rustfs-secret-key"
