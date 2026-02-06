#!/usr/bin/env bash
set -euo pipefail

QWEN_CODE_BIN="${QWEN_CODE_BINARY:-qwen-code}"
if ! command -v "$QWEN_CODE_BIN" &> /dev/null; then
  echo "⚠️  Qwen Code binary not found: $QWEN_CODE_BIN" >&2
  echo "Install the Qwen Code CLI or set QWEN_CODE_BINARY to the executable path." >&2
  exit 1
fi

exec "$QWEN_CODE_BIN" mcp-server