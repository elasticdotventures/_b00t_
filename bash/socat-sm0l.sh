#!/usr/bin/env bash
set -euo pipefail

if ! command -v socat >/dev/null 2>&1; then
  echo "socat not installed" >&2
  exit 1
fi

target="${1:-TCP:127.0.0.1:4222}"
mode="${2:-send}"

if [[ "$mode" == "listen" ]]; then
  socat -u "$target" -
else
  socat -u - "$target"
fi
