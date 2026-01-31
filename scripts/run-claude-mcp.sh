#!/usr/bin/env bash
set -euo pipefail
CLAUDE_BIN="${CLAUDE_DESKTOP_BINARY:-$HOME/.claude/claude-desktop}"
if [ ! -x "$CLAUDE_BIN" ]; then
  echo "⚠️  Claude desktop binary not found: $CLAUDE_BIN" >&2
  echo "Set CLAUDE_DESKTOP_BINARY to the executable path or install the Claude desktop app." >&2
  exit 1
fi
exec "$CLAUDE_BIN" --stdio
