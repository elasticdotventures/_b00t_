#!/usr/bin/env bash
# b00t hook adapter — claude-code
#
# Normalizes claude-code PascalCase events → b00t canonical B00T_EVENT.
# Sourced by dispatch.sh when CLAUDE_PROJECT_DIR is set.
#
# 🤓 Claude Code event names are already descriptive; b00t aliases map 1:1.
#    The adapter's job is to surface tool-specific env vars as b00t vars.

# Map claude-code PascalCase → b00t canonical (snake_case)
# 🤓 b00t canonical is snake_case; adapters for other tools convert to match.
_CC_EVENT="${HOOK_EVENT_NAME:-$(echo "${B00T_HOOK_INPUT:-}" | jq -r '.hook_event_name // empty' 2>/dev/null || echo "")}"

case "$_CC_EVENT" in
    SessionStart)         B00T_EVENT="session.start" ;;
    SessionEnd)           B00T_EVENT="session.end" ;;
    UserPromptSubmit)     B00T_EVENT="prompt.pre" ;;
    PreToolUse)           B00T_EVENT="tool.pre" ;;
    PostToolUse)          B00T_EVENT="tool.post" ;;
    PostToolUseFailure)   B00T_EVENT="tool.fail" ;;
    PermissionRequest)    B00T_EVENT="permission.request" ;;
    SubagentStart)        B00T_EVENT="agent.start" ;;
    SubagentStop)         B00T_EVENT="agent.stop" ;;
    Stop)                 B00T_EVENT="stop" ;;
    PreCompact)           B00T_EVENT="compact.pre" ;;
    PostCompact)          B00T_EVENT="compact.post" ;;
    InstructionsLoaded)   B00T_EVENT="instructions.loaded" ;;
    ConfigChange)         B00T_EVENT="config.change" ;;
    WorktreeCreate)       B00T_EVENT="worktree.create" ;;
    WorktreeRemove)       B00T_EVENT="worktree.remove" ;;
    Elicitation)          B00T_EVENT="elicitation" ;;
    ElicitationResult)    B00T_EVENT="elicitation.result" ;;
    TeammateIdle)         B00T_EVENT="teammate.idle" ;;
    TaskCompleted)        B00T_EVENT="task.completed" ;;
    Notification)         B00T_EVENT="notification" ;;
    *)                    B00T_EVENT="$_CC_EVENT" ;;  # pass through unknown
esac
export B00T_EVENT

# Expose claude-code specific context as b00t vars
export B00T_SESSION_ID="${CLAUDE_SESSION_ID:-}"
export B00T_PROJECT_DIR="${CLAUDE_PROJECT_DIR:-}"
export B00T_ENV_FILE="${CLAUDE_ENV_FILE:-}"       # SessionStart only
export B00T_REMOTE="${CLAUDE_CODE_REMOTE:-false}"
