#!/usr/bin/env bash
set -euo pipefail
KIRO_BIN="${KIRO_CLI_BINARY:-$HOME/.local/bin/kiro-cli}"
if [ ! -x "$KIRO_BIN" ]; then
  echo "⚠️  Kiro CLI binary not found: $KIRO_BIN" >&2
  echo "Set KIRO_CLI_BINARY to the executable path or install kiro-cli." >&2
  exit 1
fi
exec "$KIRO_BIN" chat --trusted-tools "${KIRO_TRUSTED_TOOLS:-tool1,tool2}" "$@"
