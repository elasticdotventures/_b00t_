#!/usr/bin/env bash
set -euo pipefail

# Simple line-based queue: append stdin to a file for downstream sm0l workers.
queue_file="${1:-/tmp/b00t-sm0l.queue}"
mkdir -p "$(dirname "$queue_file")"
touch "$queue_file"

while IFS= read -r line; do
  printf '%s\n' "$line" >> "$queue_file"
done
