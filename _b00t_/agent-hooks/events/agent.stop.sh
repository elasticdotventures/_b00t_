#!/usr/bin/env bash
# b00t hook: agent.stop  (SubagentStop)
#
# Fires when a subagent finishes. CAN block (exit 2) to prevent the subagent
# from completing — useful for requiring a retry or injecting follow-up work.
#
# stdin extra fields (claude-code):
#   .agent_type            — agent name (Explore, security-reviewer, etc.)
#   .agent_transcript_path — path to subagent's transcript .jsonl
#   .last_assistant_message — final response text (no transcript parse needed)
#
# 🤓 last_assistant_message is the fast path — avoid parsing transcript for simple checks.

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
AGENT_TYPE="$(echo "$INPUT" | jq -r '.agent_type          // "unknown"' 2>/dev/null || echo "unknown")"
LAST_MSG="$(  echo "$INPUT" | jq -r '.last_assistant_message // ""'    2>/dev/null || echo "")"

# Load role-specific agent.stop hook if defined
ROLE_HOOK="${B00T_DIR:-$HOME/.b00t/_b00t_}/hooks/roles/$AGENT_TYPE/agent.stop.sh"
if [ -x "$ROLE_HOOK" ]; then
    echo "$INPUT" | "$ROLE_HOOK"
    exit $?
fi

# Default: allow
exit 0
