#!/usr/bin/env bash
# serena-smoke.sh — MCP stdio handshake smoke test for a serena server launch command.
#
# Usage: scripts/serena-smoke.sh <launch-cmd...>
# Examples:
#   scripts/serena-smoke.sh uvx --from serena-agent serena start-mcp-server --context ide-assistant --project /home/brianh/.b00t
#   scripts/serena-smoke.sh podman run -i --rm --userns=keep-id \
#       -v /home/brianh/.b00t:/workspace:rw \
#       -v /home/brianh/.b00t/_b00t_/k8s/serena-config:/workspaces/serena/config \
#       serena:latest serena start-mcp-server --context ide-assistant --project /workspace
#   scripts/serena-smoke.sh kubectl exec -i -n b00t-serena deploy/serena -- \
#       serena start-mcp-server --context ide-assistant --project /workspace
#
# Sends JSON-RPC initialize -> notifications/initialized -> tools/list over stdio,
# waits for responses, asserts serena tool names appear. Exit 0 = PASS.
# Env: SMOKE_TIMEOUT (seconds per response, default 120)
set -uo pipefail

if [ $# -lt 1 ]; then
  echo "usage: $0 <launch-cmd...>" >&2
  exit 2
fi

TIMEOUT="${SMOKE_TIMEOUT:-120}"
EXPECT_TOOLS=(find_symbol find_referencing_symbols get_symbols_overview)

workdir=$(mktemp -d)
fifo="$workdir/stdin.fifo"
out="$workdir/stdout.log"
errf="$workdir/stderr.log"
mkfifo "$fifo"
srv_pid=""

cleanup() {
  exec 3>&- 2>/dev/null || true
  if [ -n "$srv_pid" ] && kill -0 "$srv_pid" 2>/dev/null; then
    kill "$srv_pid" 2>/dev/null
    sleep 1
    kill -9 "$srv_pid" 2>/dev/null || true
  fi
  rm -rf "$workdir"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $*" >&2
  echo "--- last stdout ---" >&2
  tail -c 2000 "$out" >&2 2>/dev/null || true
  echo "" >&2
  echo "--- last stderr ---" >&2
  tail -n 15 "$errf" >&2 2>/dev/null || true
  exit 1
}

wait_for() { # wait_for <pattern> <label>
  local deadline=$((SECONDS + TIMEOUT))
  until grep -q "$1" "$out" 2>/dev/null; do
    kill -0 "$srv_pid" 2>/dev/null || fail "server exited before $2 response"
    [ "$SECONDS" -ge "$deadline" ] && fail "timeout (${TIMEOUT}s) waiting for $2 response"
    sleep 1
  done
}

# Launch server with stdin held open on a fifo.
"$@" < "$fifo" > "$out" 2> "$errf" &
srv_pid=$!
exec 3> "$fifo"

printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"b00t-serena-smoke","version":"0.1"}}}' >&3
wait_for '"id":1' "initialize"

printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}' >&3
printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' >&3
wait_for '"id":2' "tools/list"

# Assert expected serena tool names are present.
missing=()
for t in "${EXPECT_TOOLS[@]}"; do
  grep -q "\"$t\"" "$out" || missing+=("$t")
done
[ ${#missing[@]} -eq 0 ] || fail "missing expected tools: ${missing[*]}"

# Count tools if python3 is available; otherwise crude count.
tool_count=$(python3 - "$out" <<'PY' 2>/dev/null || echo "?"
import json, sys
for line in open(sys.argv[1]):
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if msg.get("id") == 2 and "result" in msg:
        print(len(msg["result"].get("tools", [])))
        break
PY
)

echo "PASS: serena MCP handshake OK — ${tool_count} tools, includes: ${EXPECT_TOOLS[*]}"
exit 0
