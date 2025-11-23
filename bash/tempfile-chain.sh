#!/usr/bin/env bash
# Tempfile-based queue with flock retry/fail chaining (fallback for systems without FIFO/sockets)
# Usage: tempfile-chain.sh /tmp/sm0l.queue "handler_cmd"

set -euo pipefail

queue="${1:-/tmp/sm0l.queue}"
handler="${2:-cat}"
max_retries="${MAX_RETRIES:-5}"
sleep_secs="${RETRY_SLEEP:-0.5}"

touch "$queue"

try=0
while true; do
  if ! flock -n 9; then
    (( try++ ))
    if [ "$try" -gt "$max_retries" ]; then
      echo "error: unable to lock $queue after $max_retries attempts" >&2
      exit 1
    fi
    sleep "$sleep_secs"
    continue
  fi

  try=0
  while IFS= read -r line; do
    [ -z "$line" ] && continue
    printf '%s\n' "$line" | eval "$handler" || echo "warn: handler failed" >&2
  done <"$queue"

  # truncate after processing
  : >"$queue"
  flock -u 9
  sleep "$sleep_secs"
done 9<"$queue"
