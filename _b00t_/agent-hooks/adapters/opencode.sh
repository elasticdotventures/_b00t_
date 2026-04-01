#!/usr/bin/env bash
# b00t hook adapter — opencode
#
# 🤓 opencode hook naming TBD — format not yet documented.
#    Placeholder: assumes snake_case events matching b00t canonical where possible.
#    Update as opencode hook spec stabilises; track at:
#    https://github.com/sst/opencode
#
# Sourced by dispatch.sh when OPENCODE_PROJECT_DIR is set.

_OC_EVENT="${HOOK_EVENT_NAME:-$(echo "${B00T_HOOK_INPUT:-}" | jq -r '.event // .hook_event_name // empty' 2>/dev/null || echo "")}"

# 🦨 opencode event mapping — speculative, needs validation when docs available
case "$_OC_EVENT" in
    session_start|SessionStart)     B00T_EVENT="session.start" ;;
    session_end|SessionEnd)         B00T_EVENT="session.end" ;;
    pre_tool_call|PreToolUse)       B00T_EVENT="tool.pre" ;;
    post_tool_call|PostToolUse)     B00T_EVENT="tool.post" ;;
    tool_error|PostToolUseFailure)  B00T_EVENT="tool.fail" ;;
    agent_start|SubagentStart)      B00T_EVENT="agent.start" ;;
    agent_stop|SubagentStop)        B00T_EVENT="agent.stop" ;;
    stop|Stop)                      B00T_EVENT="stop" ;;
    *)                              B00T_EVENT="$_OC_EVENT" ;;
esac
export B00T_EVENT

export B00T_PROJECT_DIR="${OPENCODE_PROJECT_DIR:-}"
