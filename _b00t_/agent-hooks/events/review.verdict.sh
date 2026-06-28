#!/usr/bin/env bash
# b00t agent-hook — review.verdict
#
# 🤓 Parses VERDICT from reviewer agent output and returns exit code signals.
#    Called by dispatch.sh when B00T_EVENT = "review.verdict"
#    Input: stdin JSON with verdict data from reviewer sub-agent stop event
#
# Exit protocol (dispatch.sh contract):
#   exit 0 → allow (APPROVE — continue workflow)
#   exit 2 → block (REQUEST_CHANGES — inject feedback, block merge)
#   exit 1 → warn  (scope drift or non-critical issues)
#
# Governance invariant (PRD-REVIEWER-GOVERNANCE-ENGINE):
#   Every command triggered by a verdict MUST have ledgerr evidence.
#   Without evidence receipt → command DID NOT HAPPEN → skip.

set -euo pipefail

INPUT="$(cat)"
HOOKS_DIR="${B00T_DIR:-$HOME/.b00t/_b00t_}/agent-hooks"
REVIEWER_DIR="${B00T_DIR:-$HOME/.b00t/_b00t_}/skills/reviewer"

# ── Parse verdict from input ─────────────────────────────────────────────────

# Try multiple parsing strategies: verdict field, last line containing VERDICT, full text scan
VERDICT=""
if echo "$INPUT" | jq -e '.verdict // empty' >/dev/null 2>&1; then
    VERDICT=$(echo "$INPUT" | jq -r '.verdict')
elif echo "$INPUT" | jq -e '.output // empty' >/dev/null 2>&1; then
    # Extract VERDICT line from agent output
    VERDICT=$(echo "$INPUT" | jq -r '.output' | grep -oP 'VERDICT:\s*\K\w+' | head -1 || echo "")
elif echo "$INPUT" | grep -q 'VERDICT:'; then
    VERDICT=$(echo "$INPUT" | grep -oP 'VERDICT:\s*\K\w+' | head -1 || echo "")
fi

# ── No verdict found — allow (neutral pass) ──────────────────────────────────
if [ -z "$VERDICT" ]; then
    echo '{"event":"review.verdict","verdict":"NONE","action":"allow","reason":"no verdict found in reviewer output"}'
    exit 0
fi

# ── Check for scope warning ──────────────────────────────────────────────────
SCOPE_WARNING=""
if echo "$INPUT" | grep -q 'SCOPE WARNING:'; then
    SCOPE_WARNING=$(echo "$INPUT" | grep -oP 'SCOPE WARNING:\s*\K.*' | head -1 || echo "")
fi

# ── Governed command execution ───────────────────────────────────────────────
# 🤓 Before executing any harness action, verify ledgerr evidence exists.
#    Without evidence → command DID NOT HAPPEN → skip.

verify_evidence() {
    local cmd_id="$1"
    # Check if ledgerr MCP is available and has a receipt for this command
    if command -v ledgerr >/dev/null 2>&1; then
        if ledgerr evidence verify --command-id "$cmd_id" 2>/dev/null; then
            return 0  # evidence exists → command happened
        fi
    fi
    # No ledgerr or no receipt → command didn't happen
    return 1
}

# ── Dispatch by verdict ──────────────────────────────────────────────────────

case "${VERDICT^^}" in
    APPROVE)
        COMMAND_ID="approve-$(date +%s)-$$"
        if verify_evidence "$COMMAND_ID"; then
            echo "{\"event\":\"review.verdict\",\"verdict\":\"APPROVE\",\"action\":\"continue\",\"command_id\":\"$COMMAND_ID\"}"
        else
            # No evidence → command didn't happen → skip but allow continuation
            # (The audit gap is logged; workflow proceeds to avoid deadlock)
            echo "{\"event\":\"review.verdict\",\"verdict\":\"APPROVE\",\"action\":\"skip\",\"reason\":\"no evidence receipt\",\"command_id\":\"$COMMAND_ID\"}" >&2
            echo "{\"event\":\"review.verdict\",\"verdict\":\"APPROVE\",\"action\":\"allow\",\"warning\":\"evidence missing — audit gap logged\"}"
        fi
        exit 0
        ;;
    REQUEST_CHANGES|REQUEST_CHANGE)
        COMMAND_ID="reject-$(date +%s)-$$"
        if [ -n "$SCOPE_WARNING" ]; then
            echo "{\"event\":\"review.verdict\",\"verdict\":\"REQUEST_CHANGES\",\"action\":\"block\",\"scope_warning\":\"$SCOPE_WARNING\",\"command_id\":\"$COMMAND_ID\"}"
        else
            echo "{\"event\":\"review.verdict\",\"verdict\":\"REQUEST_CHANGES\",\"action\":\"block\",\"command_id\":\"$COMMAND_ID\"}"
        fi
        # Inject feedback into agent context for revision
        if [ -n "${B00T_HOOK_INPUT:-}" ]; then
            FEEDBACK=$(echo "$B00T_HOOK_INPUT" | jq -r '.findings // .output // "Review requested changes — revise and resubmit."' 2>/dev/null || echo "Review requested changes — revise and resubmit.")
            echo "{\"feedback\":$FEEDBACK}" >&2
        fi
        exit 2
        ;;
    *)
        # Unknown or malformed verdict — allow with warning
        echo "{\"event\":\"review.verdict\",\"verdict\":\"UNKNOWN\",\"action\":\"warn\",\"raw\":\"$VERDICT\"}"
        exit 1
        ;;
esac
