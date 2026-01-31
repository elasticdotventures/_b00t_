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

exec "$KIRO_BIN" chat --agent "$AGENT_NAME" --trust-all-tools "$@"
