#!/usr/bin/env bash
# Minimal Ralph loop: fresh-context iterations, disk state = prd.json + progress.txt + git.
# Harness: opencode `run` against local ch0nky. One story per iteration.
set -u
cd "$(dirname "$0")" || exit 1
WT="$(pwd)"
MAX="${1:-8}"
MODEL="${OPENCODE_MODEL:-qwen36-local/ch0nky}"
export OPENCODE_CONFIG="${OPENCODE_CONFIG:-/home/brianh/.config/opencode/opencode.json}"
LOG="$WT/ralph-loop.log"

remaining() {
  python3 -c 'import json;d=json.load(open("prd.json"));print(sum(1 for s in d["userStories"] if not s.get("passes")))'
}

echo "===== RALPH LOOP START $(date -Is)  wt=$WT model=$MODEL max=$MAX =====" | tee -a "$LOG"
for i in $(seq 1 "$MAX"); do
  rem="$(remaining 2>/dev/null || echo '?')"
  echo "----- iteration $i/$MAX  $(date -Is)  stories_remaining=$rem -----" | tee -a "$LOG"
  if [ "$rem" = "0" ]; then
    echo "ALL_STORIES_PASS" | tee -a "$LOG"
    exit 0
  fi
  set +e
  out="$(opencode run --auto --model "$MODEL" --dir "$WT" "$(cat prompt.md)" 2>&1)"
  rc=$?
  set -e 2>/dev/null || true
  printf '%s\n' "$out" >> "$LOG"
  printf '%s\n' "$out" | tail -25
  echo "[iteration $i exit rc=$rc]" | tee -a "$LOG"
  if printf '%s' "$out" | grep -q '<promise>COMPLETE</promise>'; then
    echo "COMPLETE_PROMISE" | tee -a "$LOG"
    exit 0
  fi
  sleep 2
done
echo "===== RALPH LOOP END (max iterations) $(date -Is) rem=$(remaining 2>/dev/null || echo '?') =====" | tee -a "$LOG"
