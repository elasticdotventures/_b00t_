#!/usr/bin/env bash
# action-issue.sh — implement a GH issue: branch → codex → test → commit → push → PR
# Usage: REPO=elasticdotventures/_b00t_ ./action-issue.sh <issue_num> <cluster> "<title>"
# Each run is isolated — safe to run in parallel via worktrees

set -euo pipefail

ISSUE_NUM="${1:?issue_num required}"
CLUSTER="${2:?cluster required}"
ISSUE_TITLE="${3:?issue_title required}"
REPO="${REPO:-elasticdotventures/_b00t_}"
MAX_CODEX_ITER="${MAX_CODEX_ITER:-3}"
WORKTREE_ROOT="${WORKTREE_ROOT:-/tmp/b00t-worktrees}"

SLUG=$(echo "${ISSUE_TITLE}" | tr '[:upper:]' '[:lower:]' | sed 's/[^a-z0-9]/-/g' | sed 's/--*/-/g' | cut -c1-40 | sed 's/-$//')
BRANCH="issue-${ISSUE_NUM}-${SLUG}"
WORKTREE="${WORKTREE_ROOT}/${BRANCH}"
LOG_PREFIX="[action-issue #${ISSUE_NUM}]"

log() { echo "${LOG_PREFIX} $*" >&2; }

cleanup() {
    local exit_code=$?
    if [[ ${exit_code} -ne 0 && -d "${WORKTREE}" ]]; then
        log "⚠️ cleanup worktree on error"
        git worktree remove --force "${WORKTREE}" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# ── 1. Fetch investigation report from GH ────────────────────────────────────
log "fetching investigation report"
INVESTIGATION=$(gh issue view "${ISSUE_NUM}" --repo "${REPO}" --json comments \
    -q '.comments[] | select(.body | contains("Hive Maintenance")) | .body' \
    | tail -1 2>/dev/null || echo "(no investigation found)")

ISSUE_BODY=$(gh issue view "${ISSUE_NUM}" --repo "${REPO}" --json body -q '.body' 2>/dev/null || echo "")

# ── 2. Create worktree on feature branch ─────────────────────────────────────
mkdir -p "${WORKTREE_ROOT}"
REPO_ROOT=$(git -C /home/brianh/.b00t rev-parse --show-toplevel)

# Check if branch already exists
if git -C "${REPO_ROOT}" rev-parse --verify "${BRANCH}" >/dev/null 2>&1; then
    log "branch ${BRANCH} exists — resuming"
    git -C "${REPO_ROOT}" worktree add "${WORKTREE}" "${BRANCH}" 2>/dev/null || {
        log "worktree already exists, using it"
        WORKTREE=$(git -C "${REPO_ROOT}" worktree list | grep "${BRANCH}" | awk '{print $1}' || echo "${WORKTREE}")
    }
else
    log "creating branch ${BRANCH}"
    git -C "${REPO_ROOT}" worktree add -b "${BRANCH}" "${WORKTREE}" main
fi

# ── 3. Write implementation prompt ───────────────────────────────────────────
IMPL_PROMPT="${WORKTREE}/.codex-impl-prompt.md"
cat > "${IMPL_PROMPT}" <<PROMPT
# Implement: Issue #${ISSUE_NUM} — ${ISSUE_TITLE}
## Cluster: ${CLUSTER}
## Repo: ${REPO}
## Branch: ${BRANCH}
## Working directory: ${WORKTREE}

## Original Issue
${ISSUE_BODY}

## Investigation Report
${INVESTIGATION}

## Your Mission
Implement the proposed changes from the investigation report above.
Working directory is already on branch ${BRANCH}.

Guidelines:
- b00t idioms: DRY, KISS, use existing OSS libs over writing new code
- Add tests if implementing new functionality (TDD)
- Update justfile if adding new commands
- Be laconic — minimal code, maximum impact
- Do NOT modify submodules (integrations/, plantuml-server)
- Commit message: "feat/fix/chore(#${ISSUE_NUM}): <short description>"

After implementing:
1. Run any relevant tests: \`just test\` or \`cargo test\` or \`npm test\`
2. Verify changes compile/lint: \`cargo check\` or \`tsc --noEmit\` as appropriate
3. Report: what you changed, what tests pass/fail

IMPORTANT: If the issue is already implemented or requires fundamental design
decisions beyond your scope, output exactly: SKIP: <reason>
PROMPT

# ── 4. Run codex exec in worktree ────────────────────────────────────────────
CODEX_OUT="${WORKTREE}/.codex-output.md"
ITER=0
IMPLEMENTED=false

while [[ ${ITER} -lt ${MAX_CODEX_ITER} ]]; do
    ITER=$((ITER + 1))
    log "codex impl iter ${ITER}/${MAX_CODEX_ITER}"

    timeout 600 codex exec \
        --config 'sandbox_permissions=["disk-full-read-access","disk-write-access"]' \
        - < "${IMPL_PROMPT}" \
        > "${CODEX_OUT}" 2>&1 || {
            log "⚠️ codex exec failed iter ${ITER} (exit $?)"
            continue
        }

    # Check if codex decided to skip
    if grep -q "^SKIP:" "${CODEX_OUT}" 2>/dev/null; then
        SKIP_REASON=$(grep "^SKIP:" "${CODEX_OUT}" | head -1)
        log "codex SKIP: ${SKIP_REASON}"
        gh issue comment "${ISSUE_NUM}" --repo "${REPO}" \
            --body "## 🤖 Action Agent: Skipped\n\n${SKIP_REASON}\n\n*action-issue.sh*" 2>/dev/null || true
        # Clean up worktree
        git -C "${REPO_ROOT}" worktree remove --force "${WORKTREE}" 2>/dev/null || true
        exit 0
    fi

    IMPLEMENTED=true
    break
done

if [[ "${IMPLEMENTED}" != "true" ]]; then
    log "❌ failed to implement after ${MAX_CODEX_ITER} iters"
    exit 1
fi

# ── 5. Check if any files changed ────────────────────────────────────────────
CHANGED=$(git -C "${WORKTREE}" status --porcelain | grep -v "^\?\? \.codex" | wc -l | tr -d ' ')
if [[ "${CHANGED}" -eq 0 ]]; then
    log "no changes made — skipping commit"
    git -C "${REPO_ROOT}" worktree remove --force "${WORKTREE}" 2>/dev/null || true
    exit 0
fi

# ── 6. Stage + commit ────────────────────────────────────────────────────────
log "committing ${CHANGED} changed files"
# Stage everything except codex temp files
git -C "${WORKTREE}" add -A
git -C "${WORKTREE}" reset -- .codex-impl-prompt.md .codex-output.md 2>/dev/null || true
git -C "${WORKTREE}" rm --cached .codex-impl-prompt.md .codex-output.md 2>/dev/null || true

# Extract short summary from codex output for commit message
SUMMARY=$(tail -20 "${CODEX_OUT}" | grep -v "^tokens\|^session\|^codex\|^---\|^user\|^exec\|^mcp" | \
    grep -v "^$" | head -2 | tr '\n' ' ' | cut -c1-60 || echo "implement issue #${ISSUE_NUM}")

git -C "${WORKTREE}" commit -m "$(cat <<EOF
feat(#${ISSUE_NUM}): ${ISSUE_TITLE:0:50}

${SUMMARY}

Closes #${ISSUE_NUM}

Co-Authored-By: codex <noreply@openai.com>
Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>
EOF
)" 2>&1

# ── 7. Push ──────────────────────────────────────────────────────────────────
log "pushing ${BRANCH}"
git -C "${WORKTREE}" push -u origin "${BRANCH}" 2>&1

# ── 8. Create PR ─────────────────────────────────────────────────────────────
log "creating PR"
CODEX_SUMMARY=$(tail -30 "${CODEX_OUT}" | grep -v "^tokens\|^session\|^---\|^mcp startup" | \
    grep -v "^$" | head -10 | sed 's/^/> /' || echo "> see codex output")

PR_URL=$(gh pr create \
    --repo "${REPO}" \
    --title "feat(#${ISSUE_NUM}): ${ISSUE_TITLE:0:60}" \
    --base main \
    --head "${BRANCH}" \
    --body "$(cat <<PR_BODY
## Issue
Closes #${ISSUE_NUM}

## Cluster
${CLUSTER}

## Changes
${CODEX_SUMMARY}

## Test Plan
- [ ] codex exec verified no errors
- [ ] relevant tests pass

🤖 Generated by scripts/hive-maintenance/action-issue.sh
PR_BODY
)" 2>&1)

log "✅ PR created: ${PR_URL}"
echo "${PR_URL}"

# ── 9. Comment on issue ───────────────────────────────────────────────────────
gh issue comment "${ISSUE_NUM}" --repo "${REPO}" \
    --body "## 🚀 Implementation PR Opened

**Branch:** \`${BRANCH}\`
**PR:** ${PR_URL}

Implementation dispatched via action-issue.sh (codex worker, iter ${ITER}/${MAX_CODEX_ITER})" 2>/dev/null || true

log "✅ done — issue #${ISSUE_NUM}"
