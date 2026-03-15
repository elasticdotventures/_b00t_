#!/usr/bin/env bash
# ooda-issue.sh — OODA loop (ralph style) per issue until resolved
# Observe → Orient → Decide → Act → check → repeat until PR open or skip
# Usage: REPO=... ./ooda-issue.sh <issue_num> <cluster> "<title>"

set -euo pipefail

ISSUE_NUM="${1:?issue_num required}"
CLUSTER="${2:?cluster required}"
ISSUE_TITLE="${3:?issue_title required}"
REPO="${REPO:-elasticdotventures/_b00t_}"
MAX_OODA="${MAX_OODA:-5}"
WORKTREE_ROOT="${WORKTREE_ROOT:-/tmp/b00t-wt}"

SLUG=$(echo "${ISSUE_TITLE}" | tr '[:upper:]' '[:lower:]' \
    | sed 's/[^a-z0-9]/-/g' | sed 's/--*/-/g' | cut -c1-40 | sed 's/-$//')
BRANCH="issue-${ISSUE_NUM}-${SLUG}"
WORKTREE="${WORKTREE_ROOT}/${BRANCH}"
REPO_ROOT=$(git -C /home/brianh/.b00t rev-parse --show-toplevel)
LOG="${WORKTREE_ROOT}/ooda-${ISSUE_NUM}.log"

mkdir -p "${WORKTREE_ROOT}"

log() { echo "[ooda #${ISSUE_NUM}] $*" | tee -a "${LOG}" >&2; }

# ── OBSERVE: check current state ─────────────────────────────────────────────
observe() {
    # Is there already an open PR for this issue?
    EXISTING_PR=$(gh pr list --repo "${REPO}" --search "Closes #${ISSUE_NUM}" \
        --json url,state -q '.[0].url' 2>/dev/null || true)
    [[ -z "${EXISTING_PR}" ]] && EXISTING_PR=$(gh pr list --repo "${REPO}" \
        --head "${BRANCH}" --json url,state -q '.[0].url' 2>/dev/null || true)
    echo "${EXISTING_PR}"
}

# ── ORIENT: read investigation + any prior codex attempts ────────────────────
orient() {
    local iter=$1
    INVESTIGATION=$(gh issue view "${ISSUE_NUM}" --repo "${REPO}" --json comments \
        -q '.comments[] | select(.body | contains("Hive Maintenance")) | .body' \
        2>/dev/null | tail -1 || echo "(no investigation)")
    ISSUE_BODY=$(gh issue view "${ISSUE_NUM}" --repo "${REPO}" \
        --json body -q '.body' 2>/dev/null || echo "")
    PRIOR_LOG=""
    [[ -f "${LOG}" ]] && PRIOR_LOG=$(tail -20 "${LOG}")
}

# ── DECIDE + ACT: codex implements in worktree ────────────────────────────────
decide_act() {
    local iter=$1

    # Setup worktree if needed
    if [[ ! -d "${WORKTREE}" ]]; then
        if git -C "${REPO_ROOT}" rev-parse --verify "${BRANCH}" >/dev/null 2>&1; then
            git -C "${REPO_ROOT}" worktree add "${WORKTREE}" "${BRANCH}" 2>/dev/null || true
        else
            git -C "${REPO_ROOT}" worktree add -b "${BRANCH}" "${WORKTREE}" main 2>/dev/null
        fi
    fi

    # Write OODA-enriched prompt
    cat > "${WORKTREE}/.ooda-prompt.md" <<PROMPT
# OODA Loop iter ${iter}/${MAX_OODA} — Issue #${ISSUE_NUM}
## Title: ${ISSUE_TITLE}
## Cluster: ${CLUSTER}
## Branch: ${BRANCH}
## Working directory: ${WORKTREE}

## Original Issue
${ISSUE_BODY}

## Investigation Report
${INVESTIGATION}

## Prior Attempts (this session)
${PRIOR_LOG}

## OODA Act
Implement the proposed changes. You are on branch ${BRANCH} in ${WORKTREE}.

Rules:
- DRY, KISS, use existing OSS libs
- Do NOT touch submodules (integrations/, plantuml-server)
- Run tests after implementing: \`cargo test\` or \`just test\` as appropriate
- If already implemented or needs fundamental human design: output SKIP: <reason>
- Commit with: git add -A && git commit -m "feat(#${ISSUE_NUM}): <description>"
- After committing: git push -u origin ${BRANCH}

Check if branch ${BRANCH} already has commits beyond main — if so, build on them.
PROMPT

    timeout 600 codex exec \
        --config 'sandbox_permissions=["disk-full-read-access","disk-write-access"]' \
        - < "${WORKTREE}/.ooda-prompt.md" \
        >> "${LOG}" 2>&1 || {
            log "⚠️ codex act failed iter ${iter} (exit $?)"
            return 1
        }
    return 0
}

# ── CHECK: did the act produce a pushed branch? ───────────────────────────────
check_pushed() {
    git -C "${WORKTREE}" log "origin/main..HEAD" --oneline 2>/dev/null | grep -q "." || return 1
    # Verify push succeeded
    git -C "${WORKTREE}" push -u origin "${BRANCH}" 2>/dev/null || true
    git ls-remote origin "${BRANCH}" 2>/dev/null | grep -q "${BRANCH}"
}

# ── CREATE PR ─────────────────────────────────────────────────────────────────
create_pr() {
    local pr_url
    pr_url=$(gh pr create \
        --repo "${REPO}" \
        --title "feat(#${ISSUE_NUM}): ${ISSUE_TITLE:0:60}" \
        --base main \
        --head "${BRANCH}" \
        --body "## Summary
Closes #${ISSUE_NUM}

## Cluster
${CLUSTER}

## OODA Loop
Resolved via ooda-issue.sh sequential loop.

🤖 codex + claude executive dispatch" 2>&1) || true
    echo "${pr_url}"
}

# ══════════════════════════════════════════════════════════════════════════════
# MAIN OODA LOOP
# ══════════════════════════════════════════════════════════════════════════════
log "🔄 OODA start — issue #${ISSUE_NUM}: ${ISSUE_TITLE}"

for iter in $(seq 1 "${MAX_OODA}"); do
    log "── iter ${iter}/${MAX_OODA} ──"

    # OBSERVE
    log "O: observe"
    EXISTING_PR=$(observe)
    if [[ -n "${EXISTING_PR}" ]]; then
        log "✅ PR already exists: ${EXISTING_PR}"
        echo "${EXISTING_PR}"
        exit 0
    fi

    # ORIENT
    log "O: orient"
    orient "${iter}"

    # DECIDE + ACT
    log "D/A: decide+act (codex)"
    decide_act "${iter}" || { log "act failed, looping"; continue; }

    # Check for SKIP
    if grep -q "^SKIP:" "${LOG}" 2>/dev/null; then
        SKIP=$(grep "^SKIP:" "${LOG}" | tail -1)
        log "SKIP detected: ${SKIP}"
        gh issue comment "${ISSUE_NUM}" --repo "${REPO}" \
            --body "## 🤖 OODA Agent: ${SKIP}\n\n*ooda-issue.sh iter ${iter}*" 2>/dev/null || true
        git -C "${REPO_ROOT}" worktree remove --force "${WORKTREE}" 2>/dev/null || true
        exit 0
    fi

    # CHECK
    log "check: branch pushed?"
    if check_pushed; then
        log "branch pushed — creating PR"
        PR_URL=$(create_pr)
        log "✅ PR: ${PR_URL}"
        gh issue comment "${ISSUE_NUM}" --repo "${REPO}" \
            --body "## 🚀 PR Opened (OODA iter ${iter})\n\n${PR_URL}" 2>/dev/null || true
        echo "${PR_URL}"
        exit 0
    fi

    log "not pushed yet — OODA loop again"
    sleep 2
done

log "❌ OODA max iter ${MAX_OODA} reached for #${ISSUE_NUM} — needs human"
gh issue comment "${ISSUE_NUM}" --repo "${REPO}" \
    --body "## ⚠️ OODA Loop Exhausted (${MAX_OODA} iters)\n\nNeeds human review.\nSee: ${LOG}" 2>/dev/null || true
exit 1
