#!/usr/bin/env bash
set -euo pipefail

queue_dir="${1:-/tmp/b00t-sm0l-chain}"
payload="${2:-}"

mkdir -p "$queue_dir"
file="$queue_dir/$(date +%s%N)-$$.msg"
printf '%s\n' "$payload" > "$file"
echo "$file"
