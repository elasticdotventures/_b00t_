#!/usr/bin/env bash
# b00t hook: agent.start  (SubagentStart)
#
# Fires when a subagent is spawned. agent_type in stdin identifies the agent.
# Non-blocking — use this for logging/observability, not gating.
#
# Worktree isolation: when .git/🥾.tomllmd exists (b00t project), creates a
# git worktree in .b00t/worktrees/<agent-id>/ so the sub-agent has its own
# checked-out filesystem view. Set B00T_NO_WORKTREE=1 to disable.
#
# 🤓 Use SubagentStop (agent.stop) to gate on subagent results.
#    agent.start is the right place to set up per-agent context or log spawns.

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
AGENT_TYPE="$(echo "$INPUT" | jq -r '.agent_type // "unknown"' 2>/dev/null || echo "unknown")"
AGENT_ID="$(echo "$INPUT"   | jq -r '.agent_id   // "unknown"' 2>/dev/null || echo "unknown")"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id  // "unknown"' 2>/dev/null || echo "unknown")"

# ── Worktree isolation ──────────────────────────────────────────────────
if [ "${B00T_NO_WORKTREE:-0}" != "1" ]; then
  PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "")"
  if [ -n "${PROJECT_ROOT}" ] && [ -f "${PROJECT_ROOT}/.git/🥾.tomllmd" ]; then
    WT_DIR="${PROJECT_ROOT}/.b00t/worktrees/${AGENT_ID}"
    if [ ! -d "${WT_DIR}" ]; then
      mkdir -p "${PROJECT_ROOT}/.b00t/worktrees"
      REF=$(git -C "${PROJECT_ROOT}" rev-parse HEAD 2>/dev/null || echo "")
      if [ -n "${REF}" ]; then
        git -C "${PROJECT_ROOT}" worktree add --detach "${WT_DIR}" "${REF}" 2>/dev/null && {
          ln -sf "${PROJECT_ROOT}/.git/🥾.tomllmd" "${WT_DIR}/.git/🥾.tomllmd" 2>/dev/null || true
          echo "b00t agent.start: worktree ${WT_DIR}" >&2
        } || echo "b00t agent.start: worktree creation failed (non-fatal)" >&2
      fi
    fi
  fi
fi

# Load role-specific agent.start hook if defined
ROLE_HOOK="${B00T_DIR:-$HOME/.b00t/_b00t_}/hooks/roles/$AGENT_TYPE/agent.start.sh"
if [ -x "$ROLE_HOOK" ]; then
    echo "$INPUT" | "$ROLE_HOOK"
    exit $?
fi

# Default: log spawn (stderr so it doesn't interfere with JSON stdout)
echo "b00t agent.start: $AGENT_TYPE ($AGENT_ID) session=$SESSION_ID" >&2

exit 0
