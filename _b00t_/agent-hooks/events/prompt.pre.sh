#!/usr/bin/env bash
# b00t agent-hook: prompt.pre  (UserPromptSubmit)
#
# Fires before user prompt is processed. Can block (exit 2) or inject context.
# Primary use: inject useful context the model doesn't have (hive state, role,
# active tools) rather than gating — keep gates in tool.pre.sh.
#
# 🤓 Context here becomes part of the model's thinking before it responds.
#    Keep injections SHORT and HIGH-SIGNAL — context window is precious.
#    Skip injection if B00T_ROLE not set (bare session, no overhead).

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
PROMPT="$(echo "$INPUT" | jq -r '.prompt // ""' 2>/dev/null | head -c 200)"

# Skip if no role configured — bare session, no b00t overhead
[ -z "${B00T_ROLE:-}" ] && exit 0

CONTEXT_PARTS=()

# Append active role
CONTEXT_PARTS+=("b00t role: $B00T_ROLE")

# Append hive status if prompt mentions resources/models/GPU/RAM
if echo "$PROMPT" | grep -qiE 'vllm|gpu|ram|memory|model|download|hive'; then
    HIVE="$(b00t-cli hive status 2>/dev/null | head -3 || true)"
    [ -n "$HIVE" ] && CONTEXT_PARTS+=("hive: $HIVE")
fi

# Append active tool/framework context from b00t session
if command -v b00t-cli >/dev/null 2>&1; then
    SKILLS="$(b00t-cli session status --field=skills 2>/dev/null || true)"
    [ -n "$SKILLS" ] && CONTEXT_PARTS+=("active skills: $SKILLS")
fi

if [ ${#CONTEXT_PARTS[@]} -eq 0 ]; then
    exit 0
fi

# Combine into systemMessage (shown as context, not blocking)
CONTEXT="$(IFS=$'\n'; echo "${CONTEXT_PARTS[*]}")"
jq -n --arg ctx "$CONTEXT" '{
    systemMessage: $ctx
}'
exit 0
