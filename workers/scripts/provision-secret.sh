#!/usr/bin/env bash
# Provision (or reuse) a secret for a Cloudflare Worker under workers/*/.
#
# Idempotent by design: if the named var already exists in ~/.env, its
# existing value is reused rather than regenerated — this script never
# appends a duplicate line for a var that's already present.
#
# Reads ~/.env directly with the same quote-stripping logic as
# ~/.bash_profile's loader (PR #1126) — deliberately NOT by sourcing
# ~/.bash_profile itself: that file has interactive-shell-only setup
# (starship, ssh-agent, direnv, nvm, ...) that isn't guarded for
# non-interactive contexts and can `exit` the sourcing shell outright,
# which `|| true` cannot catch. Same parsing algorithm, safely inlined.
#
# Usage: provision-secret.sh <worker-dir-under-workers/> <SECRET_VAR_NAME> [--length BYTES]
# Example: provision-secret.sh ledgrrr-tenant-registry TOKEN_SIGNING_KEY

set -euo pipefail

get_env_var() {
  local var_name="$1" line value
  line=$(grep -m1 "^${var_name}=" "$ENV_FILE" 2>/dev/null || true)
  [[ -n "$line" ]] || return 1
  value="${line#*=}"
  if [[ "$value" =~ ^\"(.*)\"$ ]]; then
    value="${BASH_REMATCH[1]}"
  elif [[ "$value" =~ ^\'(.*)\'$ ]]; then
    value="${BASH_REMATCH[1]}"
  fi
  printf '%s' "$value"
}

usage() {
  echo "Usage: $0 <worker-dir-under-workers/> <SECRET_VAR_NAME> [--length BYTES]" >&2
  echo "Example: $0 ledgrrr-tenant-registry TOKEN_SIGNING_KEY" >&2
  exit 1
}

[[ $# -ge 2 ]] || usage

WORKER_DIR="$1"
VAR_NAME="$2"
LENGTH=32
if [[ "${3:-}" == "--length" && -n "${4:-}" ]]; then
  LENGTH="$4"
fi

ENV_FILE="$HOME/.env"

existing="$(get_env_var "$VAR_NAME" || true)"

if [[ -n "$existing" ]]; then
  echo "🔑 ${VAR_NAME} already present in ~/.env — reusing existing value." >&2
  VALUE="$existing"
else
  VALUE=$(openssl rand -hex "$LENGTH")
  printf '%s="%s"\n' "$VAR_NAME" "$VALUE" >> "$ENV_FILE"
  echo "🔑 Generated new ${VAR_NAME} (${LENGTH} bytes hex) and saved to ~/.env" >&2
fi

# Resolve the worker directory relative to this script's location, so the
# recipe works regardless of which directory `just` was invoked from.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKER_PATH="$SCRIPT_DIR/../${WORKER_DIR}"
[[ -d "$WORKER_PATH" ]] || { echo "❌ no such worker directory: workers/${WORKER_DIR}" >&2; exit 1; }

# Strip stray quote characters defensively, regardless of where the value
# came from (an inherited shell env can carry an unstripped value from
# before ~/.bash_profile's own quote-stripping fix, PR #1126).
strip_quotes() { printf '%s' "$1" | tr -d "\"'"; }

CLOUDFLARE_ACCOUNT_ID="$(strip_quotes "${CLOUDFLARE_ACCOUNT_ID:-f00c391669432ae2a423c04a001dab2d}")"
export CLOUDFLARE_ACCOUNT_ID

if [[ -z "${CLOUDFLARE_API_TOKEN:-}" ]]; then
  CLOUDFLARE_API_TOKEN="$(get_env_var CLOUDFLARE_API_TOKEN || true)"
fi
CLOUDFLARE_API_TOKEN="$(strip_quotes "${CLOUDFLARE_API_TOKEN:-}")"
export CLOUDFLARE_API_TOKEN
[[ -n "$CLOUDFLARE_API_TOKEN" ]] || {
  echo "❌ CLOUDFLARE_API_TOKEN not set and not found in ~/.env — wrangler needs it non-interactively." >&2
  exit 1
}

echo "🌀 Setting ${VAR_NAME} secret on Worker \"${WORKER_DIR}\"..." >&2
(
  cd "$WORKER_PATH"
  printf '%s' "$VALUE" | wrangler secret put "$VAR_NAME"
)
echo "✅ ${VAR_NAME} provisioned for ${WORKER_DIR}" >&2
