#!/usr/bin/env bash
# 🤓: kiro agent runner - launches kiro-cli with specific agent context
set -euo pipefail

AGENT_NAME="${1:-}"
shift || true

if [ -z "$AGENT_NAME" ]; then
  echo "Usage: $0 <agent-name> [additional-args]" >&2
  echo "Available agents:" >&2
  kiro-cli agent list >&2
  exit 1
fi

KIRO_BIN="${KIRO_CLI_BINARY:-$HOME/.local/bin/kiro-cli}"
if [ ! -x "$KIRO_BIN" ]; then
  echo "⚠️  Kiro CLI binary not found: $KIRO_BIN" >&2
  exit 1
fi

# Support trusted tools allowlist via environment variable or argument
TRUSTED_TOOLS="${TRUSTED_TOOLS:-}"
if [ -n "$TRUSTED_TOOLS" ]; then
  exec "$KIRO_BIN" chat --agent "$AGENT_NAME" --trusted-tools "$TRUSTED_TOOLS" "$@"
else
  echo "⚠️  No trusted tools specified. Only safe default tools will be available."
  exec "$KIRO_BIN" chat --agent "$AGENT_NAME" "$@"
fi
