#!/usr/bin/env bash
# ch0nky-eval.sh — sequential benchmark of two local ch0nky models
# Usage: ./scripts/ch0nky-eval.sh [--port 8001]
# Output: results to .b00t/ralph/eval-<timestamp>.jsonl + console summary
set -euo pipefail

PORT="${1:-8001}"
BASE="http://127.0.0.1:${PORT}/v1"
TIMESTAMP=$(date +%Y%m%dT%H%M%S)
OUT_DIR=".b00t/ralph"
mkdir -p "${OUT_DIR}"
RESULTS_FILE="${OUT_DIR}/eval-${TIMESTAMP}.jsonl"

EVAL_PROMPTS=(
  "Write a one-line bash function to count lines in a file."
  "What is the output of: echo \$(( 2 ** 10 ))?"
  "In Rust, what does the ? operator do? One sentence."
  "Name 3 IETF RFC 2119 keywords and their meanings."
  "Write a b00t SCORE line for a passing datum test."
)

MODEL_ID=""
TOK_PER_SEC_SUM=0
PASS_COUNT=0
TOTAL=${#EVAL_PROMPTS[@]}

echo "=== ch0nky eval @ ${TIMESTAMP} port=${PORT} ===" | tee -a "${RESULTS_FILE}"

# Identify model
MODEL_RESP=$(curl -sf "${BASE}/models" 2>/dev/null) || { echo "FATAL: :${PORT} not responding"; exit 1; }
MODEL_ID=$(echo "${MODEL_RESP}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['data'][0].get('root', d['data'][0]['id']))" 2>/dev/null || echo "unknown")
echo "model: ${MODEL_ID}" | tee -a "${RESULTS_FILE}"

for i in "${!EVAL_PROMPTS[@]}"; do
    PROMPT="${EVAL_PROMPTS[$i]}"
    NUM=$((i+1))
    T_START=$(date +%s%3N)

    RESP=$(curl -sf "${BASE}/chat/completions" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer local-b00t" \
        -d "$(python3 -c "import json,sys; print(json.dumps({'model':'ch0nky','messages':[{'role':'user','content':sys.argv[1]}],'max_tokens':128,'temperature':0.1}))" "${PROMPT}")" 2>/dev/null) || RESP=""

    T_END=$(date +%s%3N)
    ELAPSED_MS=$(( T_END - T_START ))

    if [[ -z "${RESP}" ]]; then
        echo "  [${NUM}/${TOTAL}] FAIL (no response) | prompt: ${PROMPT:0:50}..." | tee -a "${RESULTS_FILE}"
        continue
    fi

    CONTENT=$(echo "${RESP}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['choices'][0]['message']['content'])" 2>/dev/null || echo "")
    COMPLETION_TOKENS=$(echo "${RESP}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('usage',{}).get('completion_tokens',0))" 2>/dev/null || echo "0")

    if [[ "${COMPLETION_TOKENS}" -gt 0 && "${ELAPSED_MS}" -gt 0 ]]; then
        TPS=$(python3 -c "print(round(${COMPLETION_TOKENS} * 1000 / ${ELAPSED_MS}, 1))")
        TOK_PER_SEC_SUM=$(python3 -c "print(${TOK_PER_SEC_SUM} + ${TPS})")
    else
        TPS="?"
    fi

    PASS_COUNT=$(( PASS_COUNT + 1 ))
    SHORT_RESP="${CONTENT:0:80}"
    echo "  [${NUM}/${TOTAL}] OK | ${COMPLETION_TOKENS}tok ${ELAPSED_MS}ms ${TPS}tok/s | ${SHORT_RESP}..." | tee -a "${RESULTS_FILE}"

    # jsonl record
    python3 -c "import json; print(json.dumps({'n':${NUM},'prompt':$(python3 -c "import json,sys; print(json.dumps(sys.argv[1]))" "${PROMPT}"),'tokens':${COMPLETION_TOKENS},'ms':${ELAPSED_MS},'tps':\"${TPS}\",'ok':True,'response':$(python3 -c "import json,sys; print(json.dumps(sys.argv[1]))" "${SHORT_RESP}")}))" >> "${RESULTS_FILE}"
done

AVG_TPS=$(python3 -c "print(round(${TOK_PER_SEC_SUM} / max(${PASS_COUNT},1), 1))")
echo "" | tee -a "${RESULTS_FILE}"
echo "=== SUMMARY model=${MODEL_ID} pass=${PASS_COUNT}/${TOTAL} avg_tps=${AVG_TPS} ===" | tee -a "${RESULTS_FILE}"
echo "results: ${RESULTS_FILE}"
