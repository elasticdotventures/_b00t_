#!/usr/bin/env bash
# b00t hook adapter — opencode
#
# 🤓 opencode hook naming — maps opencode event names to b00t canonical events.
#    OpenCode fork (PromptExecution/opencode-b00t) uses snake_case events.
#    Track upstream spec at: https://github.com/sst/opencode
#
# Sourced by dispatch.sh when OPENCODE_PROJECT_DIR is set.

_OC_EVENT="${HOOK_EVENT_NAME:-$(echo "${B00T_HOOK_INPUT:-}" | jq -r '.event // .hook_event_name // empty' 2>/dev/null || echo "")}"

# 🦨 opencode event mapping — canonical b00t events
case "$_OC_EVENT" in
    session_start|SessionStart)         B00T_EVENT="session.start" ;;
    session_end|SessionEnd)             B00T_EVENT="session.end" ;;
    pre_tool_call|PreToolUse)           B00T_EVENT="tool.pre" ;;
    post_tool_call|PostToolUse)         B00T_EVENT="tool.post" ;;
    tool_error|PostToolUseFailure)      B00T_EVENT="tool.fail" ;;
    agent_start|SubagentStart)          B00T_EVENT="agent.start" ;;
    agent_stop|SubagentStop)            B00T_EVENT="agent.stop" ;;
    stop|Stop)                          B00T_EVENT="stop" ;;

    # ── Reviewer verdict events (b00t extension) ─────────────────────────────
    # 🤓 These events are emitted by the b00t reviewer sub-agent when it
    #    produces a machine-parseable VERDICT: APPROVE|REQUEST_CHANGES.
    #    They route to agent-hooks/events/review.verdict.sh which triggers
    #    harness actions (continue/block/warn) based on the verdict.
    review_verdict|ReviewVerdict)       B00T_EVENT="review.verdict" ;;
    review_start|ReviewStart)           B00T_EVENT="review.start" ;;
    review_complete|ReviewComplete)     B00T_EVENT="review.complete" ;;

    # ── b00t-specific events ─────────────────────────────────────────────────
    b00t_learn|B00tLearn)               B00T_EVENT="b00t.learn" ;;
    b00t_task|B00tTask)                 B00T_EVENT="b00t.task" ;;
    b00t_agent|B00tAgent)               B00T_EVENT="b00t.agent" ;;

    *)                                  B00T_EVENT="$_OC_EVENT" ;;
esac
export B00T_EVENT

export B00T_PROJECT_DIR="${OPENCODE_PROJECT_DIR:-}"
