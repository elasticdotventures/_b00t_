#!/usr/bin/env bash
# pr-validate.sh — blocking reviewer gate for staged changes
# Usage: bash _b00t_/scripts/pr-validate.sh --goal="<issue>" [--scope="<files>"]
# Exit: 0 on APPROVE, 1 on REQUEST_CHANGES, 2 on error
set -euo pipefail

# ── Parse args ──────────────────────────────────────────────────────────────
GOAL=""
SCOPE=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --goal=*)   GOAL="${1#--goal=}" ;;
        --goal)     GOAL="$2"; shift ;;
        --scope=*)  SCOPE="${1#--scope=}" ;;
        --scope)    SCOPE="$2"; shift ;;
        *)          echo "Unknown arg: $1"; exit 2 ;;
    esac
    shift
done

if [[ -z "$GOAL" ]]; then
    GOAL="staged changes"
fi

# ── Resolve repo root ───────────────────────────────────────────────────────
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$(cd "$(dirname "$0")/../.." && pwd)")"
cd "$REPO_ROOT"

# ── Get staged diff ─────────────────────────────────────────────────────────
STAGED_DIFF="$(git diff --cached 2>/dev/null || true)"
STAGED_FILES="$(git diff --cached --name-only 2>/dev/null || true)"

if [[ -z "$STAGED_DIFF" ]]; then
    echo "✅ No staged changes — auto-approving"
    echo "VERDICT: APPROVE"
    exit 0
fi

# ── Scope drift check (pre-flight, no LLM needed) ──────────────────────────
SCOPE_WARNING=""
if [[ -n "$SCOPE" ]]; then
    while IFS= read -r file; do
        [[ -z "$file" ]] && continue
        # Check if file matches any scope pattern
        matched=false
        for pattern in $SCOPE; do
            if [[ "$file" == $pattern* ]]; then
                matched=true
                break
            fi
        done
        if [[ "$matched" == "false" ]]; then
            SCOPE_WARNING="${SCOPE_WARNING}SCOPE WARNING: $file outside declared scope $SCOPE"$'\n'
        fi
    done <<< "$STAGED_FILES"
fi

# ── Compile reviewer agent ──────────────────────────────────────────────────
AGENT_FILE="/tmp/reviewer-agent-$$.md"

if command -v just >/dev/null 2>&1; then
    echo "🔧 Compiling reviewer agent..."
    JUST_UNSTABLE=1 just compile-agent reviewer 3 "$AGENT_FILE" 2>&1 || {
        echo "⚠️  compile-agent failed — using fallback review"
        rm -f "$AGENT_FILE"
        # Fallback: check only guard patterns locally
        if echo "$STAGED_DIFF" | grep -qE '(pip install|pip3 install|npm install -g|rm -rf /|docker run --privileged)'; then
            echo "❌ Guard violation detected in staged diff"
            echo "VERDICT: REQUEST_CHANGES"
            exit 1
        fi
        echo "VERDICT: APPROVE"
        exit 0
    }
else
    echo "⚠️  'just' not found — cannot compile reviewer agent"
    echo "VERDICT: APPROVE"
    exit 0
fi

# ── Build review prompt ─────────────────────────────────────────────────────
REVIEW_PROMPT="## GOAL
${GOAL}

## SCOPE
${SCOPE:-"(not declared — all files are in scope)"}

## STAGED DIFF
\`\`\`diff
${STAGED_DIFF}
\`\`\`

## STAGED FILES
${STAGED_FILES}

Review the staged changes against the GOAL above. Check for guard violations, DRY violations, scope drift, and goal alignment. Output your verdict as the LAST line: VERDICT: APPROVE or VERDICT: REQUEST_CHANGES."

# ── Run reviewer via claude ─────────────────────────────────────────────────
echo "🔍 Running reviewer gate..."
echo "   goal: $GOAL"
echo "   scope: ${SCOPE:-any}"
echo "   staged files: $(echo "$STAGED_FILES" | wc -l) files, $(echo "$STAGED_DIFF" | wc -l) lines diff"
echo ""

REVIEW_OUTPUT=""
if command -v claude >/dev/null 2>&1; then
    REVIEW_OUTPUT="$(echo "$REVIEW_PROMPT" | claude --print --agent "$AGENT_FILE" --permission-mode bypassPermissions --max-budget-usd 0.50 2>/dev/null || true)"
else
    echo "⚠️  'claude' not found — running local guard check only"
    rm -f "$AGENT_FILE"
    # Local guard check fallback
    if echo "$STAGED_DIFF" | grep -qE '(pip install|pip3 install|npm install -g|rm -rf /|docker run --privileged)'; then
        echo "❌ Guard violation detected in staged diff"
        echo "VERDICT: REQUEST_CHANGES"
        exit 1
    fi
    echo "VERDICT: APPROVE"
    exit 0
fi

rm -f "$AGENT_FILE"

# ── Parse verdict ──────────────────────────────────────────────────────────
echo "$REVIEW_OUTPUT"
echo ""

# Output scope warnings if any were detected pre-flight
if [[ -n "$SCOPE_WARNING" ]]; then
    echo "$SCOPE_WARNING"
fi

# Extract the VERDICT line
VERDICT_LINE="$(echo "$REVIEW_OUTPUT" | grep -i '^VERDICT:' | tail -1 || true)"

if [[ -z "$VERDICT_LINE" ]]; then
    echo "⚠️  No VERDICT line found in reviewer output — treating as APPROVE"
    echo "VERDICT: APPROVE"
    exit 0
fi

VERDICT="$(echo "$VERDICT_LINE" | sed 's/VERDICT: *//i' | tr -d '[:space:]')"

case "$VERDICT" in
    APPROVE|APPROVED)
        echo ""
        echo "✅ Gate passed: APPROVE"
        exit 0
        ;;
    REQUEST_CHANGES|REQUEST_CHANGE|CHANGES_REQUESTED|REJECT|REJECTED)
        echo ""
        echo "❌ Gate blocked: REQUEST_CHANGES"
        exit 1
        ;;
    *)
        echo "⚠️  Unknown verdict '$VERDICT' — treating as APPROVE"
        exit 0
        ;;
esac
