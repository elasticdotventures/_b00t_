#!/usr/bin/env bash
# dispatch-hive.sh — parallel ralph-loop dispatcher for GH issue clusters
# Usage: ./dispatch-hive.sh [--dry-run] [--cluster <name>] [--max-iter N]
# Each cluster runs independently in background → parallel dispatch pattern

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RALPH_ISSUE="${SCRIPT_DIR}/ralph-issue.sh"
REPO="${REPO:-elasticdotventures/_b00t_}"
MAX_ITER="${MAX_ITER:-5}"
DRY_RUN=false
FILTER_CLUSTER=""
FILTER_ISSUES=""  # comma-separated issue numbers to run (empty = all)
LOG_DIR="${SCRIPT_DIR}/logs/$(date +%Y%m%d-%H%M%S)"

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=true ;;
        --cluster) FILTER_CLUSTER="$2"; shift ;;
        --issues) FILTER_ISSUES="$2"; shift ;;  # e.g. --issues "44,62,63"
        --max-iter) MAX_ITER="$2"; shift ;;
        *) echo "Unknown arg: $1"; exit 1 ;;
    esac
    shift
done

mkdir -p "${LOG_DIR}"
chmod +x "${RALPH_ISSUE}"

log() { echo "[dispatch-hive] $*" | tee -a "${LOG_DIR}/dispatch.log"; }

# Issue clusters — independent domains, safe to parallelize
# Format: "ISSUE_NUM|CLUSTER|TITLE"
declare -a ISSUES=(
    # alignment-needed — design/doc sign-offs
    "222|alignment|Define CLI personas & journeys for hive utility"
    "223|alignment|Clarify AI adapter latency + SLA budgets"
    "224|alignment|Document telemetry retention + cost guardrails"
    "225|alignment|I1 alignment brief sign-off"

    # datum — ontology/search features
    "89|datum|Datum Plans"
    "198|datum|Datum discovery: recursive scan + filename/pattern search"
    "199|datum|Datum query: dynamic filters using constraints + availability"
    "200|datum|Semantic datum search using local model"
    "201|datum|Datum ontology graph export + query"

    # skills/mcp — capability expansion
    "247|skills-mcp|ideation on 44 mcp tools"
    "248|skills-mcp|improve skill capability"

    # agent/crew — hive coordination
    "63|agent-crew|Cake economy integration and allocation tracking"
    "64|agent-crew|Crew coordination protocols for multi-agent task routing"
    "65|agent-crew|Captain-agent approval workflows for metric design"
    "66|agent-crew|Crew-level OODA decision framework"
    "77|agent-crew|b00t pair"
    "62|agent-crew|Role-based justfile templating system"

    # infra — k8s/platform
    "78|infra|anthropic desktop extension"
    "44|infra|uplift k8s beyond stub"
    "83|infra|b00t k8s Implementation Backlog - 11 Remaining Tasks"
    "91|infra|b00t.promptexecution.com"

    # tech — feature backlog
    "69|tech|redis capabilities in b00t"
    "71|tech|k0mmand3r"
    "72|tech|github-mcp-server deep dive"
    "76|tech|typescript features"
    "94|tech|mcp improvements"
    "96|tech|experiment with embedding stop sequences in files"
    "103|tech|b00t learn Official Microsoft Learn MCP server"
    "106|tech|enhance rust"
    "167|tech|Sync integration: Downstream __b00t__ requires test-gated dispatch notifications"
)

declare -a PIDS=()
declare -A PID_TO_ISSUE=()
# max concurrent codex workers — avoid OpenAI rate limits
MAX_CONCURRENT="${MAX_CONCURRENT:-8}"

dispatch_issue() {
    local spec="$1"
    local num cluster title
    IFS='|' read -r num cluster title <<< "${spec}"

    [[ -n "${FILTER_CLUSTER}" && "${cluster}" != "${FILTER_CLUSTER}" ]] && return 0
    if [[ -n "${FILTER_ISSUES}" ]]; then
        echo ",${FILTER_ISSUES}," | grep -q ",${num}," || return 0
    fi

    if [[ "${DRY_RUN}" == "true" ]]; then
        log "DRY-RUN: would dispatch #${num} [${cluster}] — ${title}"
        return 0
    fi

    # rate-limit: wait if too many concurrent workers
    while [[ $(jobs -r | wc -l) -ge ${MAX_CONCURRENT} ]]; do
        sleep 2
    done

    log "→ dispatching #${num} [${cluster}] — ${title}"
    REPO="${REPO}" MAX_ITER="${MAX_ITER}" \
        "${RALPH_ISSUE}" "${num}" "${cluster}" "${title}" \
        > "${LOG_DIR}/issue-${num}-${cluster}.log" 2>&1 &
    local pid=$!
    PIDS+=("${pid}")
    PID_TO_ISSUE["${pid}"]="${num}|${cluster}"
    log "  PID=${pid} → issue #${num}"
}

log "🚀 hive maintenance dispatch start"
log "  REPO=${REPO} MAX_ITER=${MAX_ITER} DRY_RUN=${DRY_RUN}"
log "  LOG_DIR=${LOG_DIR}"
[[ -n "${FILTER_CLUSTER}" ]] && log "  FILTER_CLUSTER=${FILTER_CLUSTER}"

# Dispatch all issues in parallel by cluster
for issue_spec in "${ISSUES[@]}"; do
    dispatch_issue "${issue_spec}"
done

[[ "${DRY_RUN}" == "true" ]] && { log "DRY-RUN complete."; exit 0; }

log "⏳ waiting for ${#PIDS[@]} parallel workers..."

# Wait and collect results
SUCCESS=0
FAILED=0
for pid in "${PIDS[@]}"; do
    issue_ref="${PID_TO_ISSUE[${pid}]}"
    if wait "${pid}"; then
        log "✅ DONE   PID=${pid} issue=${issue_ref}"
        SUCCESS=$((SUCCESS + 1))
    else
        log "❌ FAILED PID=${pid} issue=${issue_ref}"
        FAILED=$((FAILED + 1))
    fi
done

log "📊 dispatch complete: ${SUCCESS} succeeded, ${FAILED} failed"
log "📁 logs: ${LOG_DIR}/"

# Write summary for ralph-loop promise detection
if [[ ${FAILED} -eq 0 ]]; then
    echo "<promise>HIVE MAINTENANCE COMPLETE</promise>"
    exit 0
else
    echo "⚠️ ${FAILED} issues need human review — check ${LOG_DIR}/"
    exit 1
fi
