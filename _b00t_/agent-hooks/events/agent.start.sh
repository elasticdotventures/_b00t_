#!/usr/bin/env bash
# b00t hook: agent.start  (SubagentStart)
#
# Fires when a subagent is spawned. agent_type in stdin identifies the agent.
# Non-blocking — use this for logging/observability, not gating.
#
# 🤓 Use SubagentStop (agent.stop) to gate on subagent results.
#    agent.start is the right place to set up per-agent context or log spawns.

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
AGENT_TYPE="$(echo "$INPUT" | jq -r '.agent_type // "unknown"' 2>/dev/null || echo "unknown")"
AGENT_ID="$(echo "$INPUT"   | jq -r '.agent_id   // "unknown"' 2>/dev/null || echo "unknown")"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id  // "unknown"' 2>/dev/null || echo "unknown")"

# Load role-specific agent.start hook if defined
ROLE_HOOK="${B00T_DIR:-$HOME/.b00t/_b00t_}/hooks/roles/$AGENT_TYPE/agent.start.sh"
if [ -x "$ROLE_HOOK" ]; then
    echo "$INPUT" | "$ROLE_HOOK"
    exit $?
fi

# Default: log spawn (stderr so it doesn't interfere with JSON stdout)
echo "b00t agent.start: $AGENT_TYPE ($AGENT_ID) session=$SESSION_ID" >&2

exit 0
