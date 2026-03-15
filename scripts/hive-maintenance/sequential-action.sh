#!/usr/bin/env bash
# sequential-action.sh — process issues ONE AT A TIME with OODA/ralph retry
# No parallelism. Each issue fully resolved (PR open) before next.
# Usage: ./sequential-action.sh [--start-from <issue_num>]

set -euo pipefail

REPO="${REPO:-elasticdotventures/_b00t_}"
MAX_OODA="${MAX_OODA:-5}"
WORKTREE_ROOT="${WORKTREE_ROOT:-/tmp/b00t-wt}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OODA="${SCRIPT_DIR}/ooda-issue.sh"
START_FROM="${1:-0}"
[[ "${1:-}" == "--start-from" ]] && START_FROM="${2:-0}"

RESULTS_LOG="${SCRIPT_DIR}/logs/sequential-$(date +%Y%m%d-%H%M%S).log"
mkdir -p "$(dirname "${RESULTS_LOG}")"
chmod +x "${OODA}"

log() { echo "[sequential] $*" | tee -a "${RESULTS_LOG}"; }

# DELEGATE_CODEX issues only — ordered simple→complex
declare -a ISSUES=(
    "222|alignment|Define CLI personas and journeys for hive utility"
    "223|alignment|Clarify AI adapter latency and SLA budgets"
    "247|skills-mcp|ideation on 44 mcp tools"
    "248|skills-mcp|improve skill capability"
    "103|tech|b00t learn Microsoft Learn MCP server"
    "96|tech|experiment with embedding stop sequences in files"
    "94|tech|mcp improvements"
    "198|datum|Datum discovery recursive scan and filename pattern search"
    "199|datum|Datum query dynamic filters using constraints and availability"
    "200|datum|Semantic datum search using local model"
    "201|datum|Datum ontology graph export and query"
    "89|datum|Datum Plans"
    "76|tech|typescript features"
    "106|tech|enhance rust"
    "69|tech|redis capabilities in b00t"
    "72|tech|github-mcp-server deep dive"
    "167|tech|Sync integration test-gated dispatch notifications"
    "63|agent-crew|Cake economy integration and allocation tracking"
    "64|agent-crew|Crew coordination protocols for multi-agent task routing"
    "65|agent-crew|Captain-agent approval workflows for metric design"
    "66|agent-crew|Crew-level OODA decision framework"
    "77|agent-crew|b00t pair"
    "78|infra|anthropic desktop extension"
    "91|infra|b00t promptexecution com"
    "44|infra|uplift k8s beyond stub"
    "83|infra|b00t k8s Implementation Backlog"
)

DONE=0
SKIPPED=0
FAILED=0

for issue_spec in "${ISSUES[@]}"; do
    IFS='|' read -r num cluster title <<< "${issue_spec}"

    # Skip issues before START_FROM
    [[ "${START_FROM}" -gt 0 && "${num}" -lt "${START_FROM}" ]] && continue

    # Check if PR already exists (from parallel agents or prior run)
    EXISTING=$(gh pr list --repo "${REPO}" --head "issue-${num}-*" \
        --json url -q '.[0].url' 2>/dev/null || true)
    if [[ -n "${EXISTING}" ]]; then
        log "⏭️  #${num} — PR exists: ${EXISTING} (skipping)"
        DONE=$((DONE + 1))
        continue
    fi

    log "▶️  processing #${num} [${cluster}]: ${title}"

    OODA_OUTPUT=$(REPO="${REPO}" MAX_OODA="${MAX_OODA}" WORKTREE_ROOT="${WORKTREE_ROOT}" \
        "${OODA}" "${num}" "${cluster}" "${title}" 2>&1) || {
        log "❌ #${num} failed OODA loop"
        FAILED=$((FAILED + 1))
        continue
    }

    printf '%s\n' "${OODA_OUTPUT}" | tee -a "${RESULTS_LOG}" >/dev/null

    PR_URL=$(printf '%s\n' "${OODA_OUTPUT}" | grep -oE 'https://github.com[^[:space:]]*' | head -n1 || true)

    if echo "${PR_URL}" | grep -q "github.com"; then
        log "✅ #${num} → ${PR_URL}"
        DONE=$((DONE + 1))
    else
        log "⚠️  #${num} — no PR URL (skip/error)"
        SKIPPED=$((SKIPPED + 1))
    fi

    log "── pause 3s before next issue ──"
    sleep 3
done

log "═══════════════════════════════════════"
log "📊 sequential complete: ${DONE} done, ${SKIPPED} skipped, ${FAILED} failed"
log "📁 log: ${RESULTS_LOG}"

if [[ ${FAILED} -eq 0 ]]; then
    echo "<promise>SEQUENTIAL ACTION COMPLETE</promise>"
fi
