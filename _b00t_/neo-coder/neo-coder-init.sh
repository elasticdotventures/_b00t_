#!/usr/bin/env bash
# neo-coder-init.sh — banner-exchange initialization for NEO-CODER agents.
#
# Usage: ./neo-coder-init.sh <objective>
#
# Banner flow:
#   1. identity    → agent knows who it is
#   2. environment → agent discovers its sandbox
#   3. disposition → agent checks system-normal (gates)
#   4. task        → agent receives objective
#   5. execute     → agent does the work
#
# If banner3 (disposition) returns Violated, the sequence HALTS.

MODEL_ENDPOINT="${B00T_NEO_CODER_MODEL:-http://127.0.0.1:8001/v1}"
MODEL="${B00T_NEO_CODER_MODEL_NAME:-ch0nky}"
OBJECTIVE="${1:?Usage: neo-coder-init.sh <objective>}"

call_model() {
    local prompt="$1"
    local max_tokens="${2:-512}"
    local payload
    payload=$(jq -n \
        --arg model "$MODEL" \
        --arg content "$prompt" \
        --argjson max_tokens "$max_tokens" \
        '{model: $model, messages: [{role: "user", content: $content}], max_tokens: $max_tokens, temperature: 0.3}')
    curl -s "${MODEL_ENDPOINT}/chat/completions" \
        -H "Content-Type: application/json" \
        -d "$payload" | jq -r '.choices[0].message.content // "ERROR: no response"'
}

banner() { echo "═══ BANNER: ${1} ═══"; }

# ── Banner1: Identity ────────────────────────────────────────────────────────
banner "identity"
IDENTITY=$(call_model "You are NEO-CODER, a containerized coding agent. Model: ${MODEL}. Role: worker. Skills: rust, disposition-pipeline, b00t-cli. Respond ONLY with: NEO-CODER ONLINE | role=worker | skills=rust,disposition-pipeline" 128)
echo "$IDENTITY"

# ── Banner2: Environment ─────────────────────────────────────────────────────
banner "environment"
ENV_HOST=$(hostname 2>/dev/null || echo "container")
ENV_DATUMS=$(ls _b00t_/ 2>/dev/null | wc -l || echo "0")
ENV_GIT=$(git branch --show-current 2>/dev/null || echo "detached")
echo "ENV | host=${ENV_HOST} | branch=${ENV_GIT} | datums=${ENV_DATUMS}"

# ── Banner3: Disposition ─────────────────────────────────────────────────────
banner "disposition"
CHECKS_PASSED=0; CHECKS_FAILED=0
for cmd in git curl jq; do
    if command -v "$cmd" &>/dev/null; then CHECKS_PASSED=$((CHECKS_PASSED + 1))
    else CHECKS_FAILED=$((CHECKS_FAILED + 1)); echo "MISSING: $cmd"; fi
done
if [ "$CHECKS_FAILED" -gt 0 ]; then
    echo "DISPOSITION | Violated | missing ${CHECKS_FAILED} required tools"
    echo "HALT: disposition violated."
    exit 1
fi
echo "DISPOSITION | Satisfied | ${CHECKS_PASSED} tools present"

# ── Banner4: Task ────────────────────────────────────────────────────────────
banner "task"
TASK_ANALYSIS=$(call_model "Analyze this objective. Respond as: TASK | objective | subtasks=N | complexity=1-10. Objective: ${OBJECTIVE}" 256)
echo "$TASK_ANALYSIS"

# ── Banner5: Execute ─────────────────────────────────────────────────────────
banner "execute"
EXECUTION=$(call_model "Write Rust code for: ${OBJECTIVE}. Include a test. Show only code." 2048)
echo "$EXECUTION"

echo ""
echo "═══ NEO-CODER SESSION COMPLETE ═══"
