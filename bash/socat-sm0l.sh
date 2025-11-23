#!/usr/bin/env bash
# socat-based UNIX socket front-end with retry/fail chaining
# Usage: socat-sm0l.sh /tmp/sm0l.sock "handler_cmd"

set -euo pipefail

sock="${1:-/tmp/sm0l.sock}"
handler="${2:-cat}"
max_retries="${MAX_RETRIES:-5}"
sleep_secs="${RETRY_SLEEP:-0.5}"

try=0
while true; do
  # ensure old socket is gone
  [ -S "$sock" ] && rm -f "$sock"

  socat UNIX-LISTEN:"$sock",fork,reuseaddr SYSTEM:"$handler" &
  pid=$!

  # wait a bit; if socat dies immediately, retry
  sleep "$sleep_secs"
  if ! kill -0 "$pid" 2>/dev/null; then
    (( try++ ))
    if [ "$try" -gt "$max_retries" ]; then
      echo "error: socat failed to start after $max_retries attempts" >&2
      exit 1
    fi
    continue
  fi

  try=0
  wait "$pid" || true
done
