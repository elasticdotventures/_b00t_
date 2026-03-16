#!/usr/bin/env bash
# b00t agent-hook: executive/agent.stop  (SubagentStop for executive role)
#
# Gates subagent completion. Validates output contract compliance before
# the executive context receives the result.
#
# exit 2 = re-queue the agent (block stop, Claude retries or prompts)
# exit 0 = accept output
#
# 🤓 last_assistant_message is the fast path — no transcript parse needed.
#    Keep validation FAST — this is on the hot path of every subagent.
#    Escalate to frontier review only on high-stakes agents (architect, security).

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
AGENT_TYPE="$(echo "$INPUT" | jq -r '.agent_type               // "unknown"' 2>/dev/null)"
LAST_MSG="$(  echo "$INPUT" | jq -r '.last_assistant_message   // ""'       2>/dev/null)"
AGENT_ID="$(  echo "$INPUT" | jq -r '.agent_id                 // "unknown"' 2>/dev/null)"

# ─── sm0l output contract validation ─────────────────────────────────────────
case "$AGENT_TYPE" in
    Explore|Grep|Glob|qa|lint|test-runner|sm0l*)
        # sm0l must produce PASS or FAIL + excerpt — not a multi-paragraph essay
        if [ ${#LAST_MSG} -gt 2000 ]; then
            echo "⚠️  b00t executive: sm0l agent ($AGENT_TYPE) output too long (${#LAST_MSG} chars)" >&2
            echo "   Contract: PASS | FAIL: <name> <≤5 lines>. Summarize." >&2
            # Warn but don't block — executive still gets result, just flags it
        fi
        ;;
    architect|security|Plan*)
        # frontier agents: must produce structured output, not single-line
        if [ ${#LAST_MSG} -lt 100 ]; then
            echo "⚠️  b00t executive: frontier agent ($AGENT_TYPE) output suspiciously short (${#LAST_MSG} chars)" >&2
            echo "   Expected structured decision/rationale." >&2
        fi
        ;;
esac

echo "b00t executive: agent.stop $AGENT_TYPE ($AGENT_ID) output_len=${#LAST_MSG}" >&2
exit 0
