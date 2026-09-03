#!/usr/bin/env bash
# gpu-heater — keep the RTX3090 (and the office) warm by never letting the local
# qwen ch0nky slot go idle. Pulls prompts from a rotating backlog and streams
# generations straight at :8001 (no MCP, no opencode server — cheap driver).
#
# It's a space heater with a side effect of useful work: each cycle's output is
# appended to .b00t/gpu-heater.out for the other Ralphs to harvest.
#
# Env: HEATER_MAX_TOKENS (default 512), HEATER_IDLE_ONLY=1 (only fire when GPU <5%),
#      HEATER_ONESHOT=1, HEATER_SLEEP_SECS (default 3)
set -u
API="http://127.0.0.1:8001/v1/chat/completions"
REPO="${B00T_REPO_ROOT:-/home/brianh/.b00t}"
OUT="${REPO}/.b00t/gpu-heater.out"
MAXTOK="${HEATER_MAX_TOKENS:-512}"
SLEEP_SECS="${HEATER_SLEEP_SECS:-3}"
BACKLOG="${HEATER_BACKLOG:-${REPO}/.b00t/gpu-heater-backlog.txt}"
mkdir -p "${REPO}/.b00t" 2>/dev/null || true
. "$(dirname "$0")/lib/agent-progress.sh" 2>/dev/null || true

# Seed a backlog of self-refilling work if none exists. These are real, useful
# local-qwen jobs — summarise datums, draft tests, explain code — not busywork.
if [ ! -s "$BACKLOG" ]; then
  cat > "$BACKLOG" <<'EOF'
Summarise one b00t hive resilience risk and propose a one-line mitigation.
Draft a bash `bats` test case for scripts/b00t-hive-watchdog.sh crash-loop trip.
Explain what a SHACL shape is and give a minimal example for a "Document" node.
Write a DuckDB SQL snippet that does cosine-similarity top-k over a FLOAT[] column.
Propose 3 acceptance criteria for a pgwire dynamic-embedding query provider.
Draft a progress.txt "Codebase Patterns" bullet about NATS best-effort publishing.
Explain the tradeoff between llama.cpp prompt-cache and LMCache for agent workloads.
Write a python one-liner that tails .b00t/hive-watchdog.jsonl and prints crashloop events.
EOF
fi

gpu_util() { nvidia-smi --query-gpu=utilization.gpu --format=csv,noheader,nounits 2>/dev/null | tr -dc '0-9'; }

fire() {
  local prompt="$1" u
  if [ "${HEATER_IDLE_ONLY:-0}" = "1" ]; then
    u="$(gpu_util)"; u="${u:-0}"
    [ "$u" -ge 5 ] && { echo "$(date -Is) skip (gpu ${u}% busy)" >> "$OUT"; return 0; }
  fi
  local body
  body="$(printf '{"messages":[{"role":"user","content":%s}],"max_tokens":%s,"temperature":0.7}' \
    "$(printf '%s' "$prompt" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')" "$MAXTOK")"
  {
    echo "=== $(date -Is) :: $prompt"
    curl -s -m 300 "$API" -H 'Content-Type: application/json' -d "$body" \
      | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin); t=d["choices"][0]["message"]["content"]; u=d.get("usage",{})
    print(t.strip()[:1200]); print("[tokens:", u.get("completion_tokens","?"), "]")
except Exception as e:
    sys.stdin.seek(0) if hasattr(sys.stdin,"seek") else None
    print("[heater: bad response]", e)'
    echo
  } >> "$OUT"
  command -v pr_progress >/dev/null && PR_AGENT=gpu-heater pr_progress "gpu.heater" "" "" "job=${prompt:0:60}"
}

run_once() {
  # take the top line, rotate it to the bottom (self-refilling backlog)
  local line
  line="$(head -1 "$BACKLOG")"
  [ -z "$line" ] && return 0
  sed -i '1d' "$BACKLOG"; printf '%s\n' "$line" >> "$BACKLOG"
  fire "$line"
}

[ "${HEATER_ONESHOT:-0}" = "1" ] && { run_once; exit 0; }
echo "gpu-heater: feeding :8001, backlog=$BACKLOG, out=$OUT" >&2
while true; do
  curl -sf -m3 http://127.0.0.1:8001/health >/dev/null 2>&1 && run_once || echo "$(date -Is) :8001 down, wait" >> "$OUT"
  sleep "$SLEEP_SECS"
done
