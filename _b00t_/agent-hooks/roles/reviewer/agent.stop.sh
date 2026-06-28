#!/usr/bin/env bash
# b00t agent-hook — roles/reviewer/agent.stop.sh
#
# 🤓 Role-specific hook: fires when a reviewer sub-agent stops.
#    Extracts the verdict from reviewer output, normalizes to review.verdict event,
#    and routes through dispatch.sh for verdict processing.
#
# Input: stdin JSON from dispatch.sh (agent stop event with output/verdict)
# Output: emits review.verdict event via B00T_EVENT override
#
# Called by: dispatch.sh when B00T_ROLE="reviewer" and event is "agent.stop"

set -euo pipefail

INPUT="$(cat)"
HOOKS_DIR="${B00T_DIR:-$HOME/.b00t/_b00t_}/agent-hooks"

# ── Extract verdict from reviewer output ─────────────────────────────────────
# The reviewer agent (from _b00t_/skills/reviewer/) outputs a machine-parseable
# verdict line: VERDICT: APPROVE or VERDICT: REQUEST_CHANGES

VERDICT=""
FINDINGS=""

# Strategy 1: Parse structured JSON output
if echo "$INPUT" | jq -e '.verdict // empty' >/dev/null 2>&1; then
    VERDICT=$(echo "$INPUT" | jq -r '.verdict')
    FINDINGS=$(echo "$INPUT" | jq -r '.findings // []' 2>/dev/null || echo "[]")
fi

# Strategy 2: Parse text output for VERDICT line
if [ -z "$VERDICT" ]; then
    OUTPUT=$(echo "$INPUT" | jq -r '.output // .result // empty' 2>/dev/null || echo "$INPUT")
    VERDICT=$(echo "$OUTPUT" | grep -oP 'VERDICT:\s*\K\w+' | head -1 || echo "")
    if echo "$OUTPUT" | grep -q 'SCOPE WARNING:'; then
        SCOPE_WARN=$(echo "$OUTPUT" | grep -oP 'SCOPE WARNING:\s*\K.*' | head -1 || echo "")
    fi
fi

# ── No verdict found — neutral pass ──────────────────────────────────────────
if [ -z "$VERDICT" ]; then
    echo "{\"event\":\"agent.stop\",\"role\":\"reviewer\",\"verdict\":\"NONE\",\"note\":\"reviewer completed without machine-parseable verdict\"}"
    exit 0
fi

# ── Normalize to review.verdict event ────────────────────────────────────────
# Override B00T_EVENT so dispatch.sh routes to events/review.verdict.sh
export B00T_EVENT="review.verdict"

# Construct verdict event payload
PAYLOAD=$(jq -n \
    --arg verdict "$VERDICT" \
    --arg scope_warning "${SCOPE_WARN:-}" \
    --argjson findings "${FINDINGS:-[]}" \
    --arg timestamp "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg session_id "${B00T_SESSION_ID:-unknown}" \
    '{
        event: "review.verdict",
        verdict: $verdict,
        scope_warning: $scope_warning,
        findings: $findings,
        timestamp: $timestamp,
        session_id: $session_id,
        source_role: "reviewer"
    }')

# Route through the verdict event handler
VERDICT_HOOK="$HOOKS_DIR/events/review.verdict.sh"
if [ -x "$VERDICT_HOOK" ]; then
    echo "$PAYLOAD" | "$VERDICT_HOOK"
    exit $?
fi

# Fallback: verdict hook not found — log and allow
echo "{\"event\":\"agent.stop\",\"role\":\"reviewer\",\"verdict\":\"$VERDICT\",\"warning\":\"review.verdict.sh not found — verdict not processed\"}" >&2
exit 0
