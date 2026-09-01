#!/usr/bin/env bash
set -euo pipefail

# Fetches the b00t ACP NATS bus's (fung1 + sm3lly hive-b00t-relay) LAN
# credential from Azure Key Vault (kv-pe-agent-secrets,
# config-global-hive-nats-{user,password}) and writes it to
# ~/.b00t/secrets/{nats-user,nats-password} (bare-value files, existing
# consumer contract) and ~/.b00t/secrets/hive-nats.env (KEY=VALUE, for
# systemd EnvironmentFile= consumers). app4dog is a promptexecution
# sub-project and owns no infrastructure of its own, so it authenticates
# against this bus rather than running its own NATS.
#
# hive-nats.env includes a precomposed NATS_URL (with the credential
# embedded) specifically so consumers like b00t_historian.py never need
# the credential passed as a CLI argument - args are visible to any local
# user via `ps aux`/`/proc/<pid>/cmdline`, unlike process environment
# (only visible via /proc/<pid>/environ, same-user/root only). Confirmed
# the hard way 2026-09-01: an earlier version of this rotation passed
# --nats-url on b00t-historian's command line, which leaked the new
# credential right back into `ps`/systemd status output immediately after
# rotating it specifically to stop that class of exposure.
#
# Previously parsed the credential OUT of nats.conf, which is why nats.conf
# used to hold it in plaintext (infra#191 - that file is tracked in the
# public elasticdotventures/_b00t_ repo). Inverted 2026-09-01: Key Vault is
# now the source of truth, nats.conf just references $HIVE_NATS_USER/
# $HIVE_NATS_PASSWORD. Requires `az login`.

VAULT="kv-pe-agent-secrets"
SECRETS_DIR="${HOME}/.b00t/secrets"

user=$(az keyvault secret show --vault-name "$VAULT" --name config-global-hive-nats-user --query value -o tsv)
password=$(az keyvault secret show --vault-name "$VAULT" --name config-global-hive-nats-password --query value -o tsv)

if [ -z "$user" ] || [ -z "$password" ]; then
  echo "sync-nats-secrets: failed to fetch user/password from Key Vault $VAULT" >&2
  exit 1
fi

mkdir -p "$SECRETS_DIR"
printf '%s' "$user" > "$SECRETS_DIR/nats-user"
printf '%s' "$password" > "$SECRETS_DIR/nats-password"
{
  printf 'HIVE_NATS_USER=%s\n' "$user"
  printf 'HIVE_NATS_PASSWORD=%s\n' "$password"
  printf 'NATS_URL=nats://%s:%s@127.0.0.1:4222\n' "$user" "$password"
} > "$SECRETS_DIR/hive-nats.env"
chmod 600 "$SECRETS_DIR/nats-user" "$SECRETS_DIR/nats-password" "$SECRETS_DIR/hive-nats.env"

echo "sync-nats-secrets: wrote $SECRETS_DIR/nats-user, $SECRETS_DIR/nats-password, $SECRETS_DIR/hive-nats.env"
