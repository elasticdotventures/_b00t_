#!/usr/bin/env bash
# b00t agent-hook: executive/agent.start  (SubagentStart for executive role)
#
# Fires when executive spawns a subagent. Injects cognitive tier constraints
# and output contract expectations into the subagent context.
#
# 🤓 Executive role enforces tier routing: sm0l tasks MUST NOT reach frontier.
#    Output contracts by tier:
#      sm0l:    "PASS | FAIL: <name> <≤5 line excerpt>"
#      ch0nky:  "diff_summary + test_pass_count/total"
#      frontier: structured decision (JSON or structured markdown)

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
AGENT_TYPE="$(echo "$INPUT" | jq -r '.agent_type // "unknown"' 2>/dev/null)"
AGENT_ID="$(  echo "$INPUT" | jq -r '.agent_id   // "unknown"' 2>/dev/null)"

# Classify agent tier from agent_type name
TIER="frontier"
case "$AGENT_TYPE" in
    Explore|Grep|Glob|qa|lint|test-runner|sm0l*)  TIER="sm0l" ;;
    backend|frontend|refactor*|fix*|ch0nky*)       TIER="ch0nky" ;;
    architect|security|Plan|product|frontier*)     TIER="frontier" ;;
esac

# Derive output contract for this tier
case "$TIER" in
    sm0l)     CONTRACT="Output contract: PASS | FAIL: <test_name> <≤5 line excerpt>" ;;
    ch0nky)   CONTRACT="Output contract: diff summary + test_pass_count/total" ;;
    frontier) CONTRACT="Output contract: structured decision with rationale" ;;
esac

echo "b00t executive: spawning $AGENT_TYPE ($AGENT_ID) tier=$TIER" >&2

# Inject tier context so the subagent knows its constraints
jq -n \
    --arg tier "$TIER" \
    --arg contract "$CONTRACT" \
    --arg agent_type "$AGENT_TYPE" \
    '{
        hookSpecificOutput: {
            hookEventName: "SubagentStart",
            additionalContext: ("b00t executive tier=" + $tier + " | " + $contract + " | agent=" + $agent_type)
        }
    }'
exit 0
