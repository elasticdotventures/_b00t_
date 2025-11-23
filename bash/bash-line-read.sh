#!/usr/bin/env bash
# Simple FIFO queue reader with retry/fail chaining.
# Usage: bash-line-read.sh /tmp/sm0l.q "handler_cmd"

set -euo pipefail

fifo="${1:-/tmp/sm0l.q}"
handler="${2:-cat}"
max_retries="${MAX_RETRIES:-5}"
sleep_secs="${RETRY_SLEEP:-0.5}"

try=0
while true; do
  if [ ! -p "$fifo" ]; then
    (( try++ ))
    if [ "$try" -gt "$max_retries" ]; then
      echo "error: fifo $fifo not available after $max_retries attempts" >&2
      exit 1
    fi
    mkfifo "$fifo" 2>/dev/null || true
    sleep "$sleep_secs"
    continue
  fi

  # reset retry counter once fifo is ready
  try=0

  # read line by line and hand off to handler; if handler fails, keep going
  while IFS= read -r line; do
    if [ -z "$line" ]; then
      continue
    fi
    printf '%s\n' "$line" | $handler || echo "warn: handler failed" >&2
  done <"$fifo"
done
