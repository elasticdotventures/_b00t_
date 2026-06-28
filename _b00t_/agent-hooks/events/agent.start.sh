#!/usr/bin/env bash
# b00t hook: agent.start  (SubagentStart)
#
# Fires when a subagent is spawned. agent_type in stdin identifies the agent.
# Non-blocking — use this for logging/observability, not gating.
#
# Lifecycle:
#   1. Worktree isolation — git worktree in .b00t/worktrees/<agent-id>/
#   2. Ledgerr registration — yei registers for resource budget (GPU, network)
#   3. Role-specific hooks — roles/<agent-type>/agent.start.sh
#
# Set B00T_NO_WORKTREE=1 to disable worktree isolation.
# Set B00T_NO_LEDGERR=1 to skip ledgerr registration.

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
AGENT_TYPE="$(echo "$INPUT" | jq -r '.agent_type // "unknown"' 2>/dev/null || echo "unknown")"
AGENT_ID="$(echo "$INPUT"   | jq -r '.agent_id   // "unknown"' 2>/dev/null || echo "unknown")"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id  // "unknown"' 2>/dev/null || echo "unknown")"
TASK_ID="${B00T_TASK_ID:-${SESSION_ID}}"

PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "")"

# ── Phase 1: Worktree isolation ──────────────────────────────────────────
if [ "${B00T_NO_WORKTREE:-0}" != "1" ] && [ -n "${PROJECT_ROOT}" ] && [ -f "${PROJECT_ROOT}/.git/🥾.tomllmd" ]; then
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

# ── Phase 2: Ledgerr registration (yei identity + resource budget) ───────
if [ "${B00T_NO_LEDGERR:-0}" != "1" ] && [ -n "${PROJECT_ROOT}" ]; then
  LEDGERR_BIN=""
  for candidate in \
    "${LEDGERR_MCP_CMD:-}" \
    "${PROJECT_ROOT}/vendor/l3dg3rr/target/release/ledgerr-mcp-server" \
    "$HOME/.b00t/vendor/l3dg3rr/target/release/ledgerr-mcp-server"; do
    if [ -n "${candidate}" ] && [ -x "${candidate}" ]; then
      LEDGERR_BIN="${candidate}"
      break
    fi
  done
  if [ -n "${LEDGERR_BIN}" ]; then
    printf '{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"ledgerr_b00t_delegate_datum","arguments":{"datum_id":"%s","agent_id":"%s","task_id":"%s","estimated_cost_usd":0.0}}}\n' \
      "${AGENT_TYPE}" "${AGENT_ID}" "${TASK_ID}" \
      | "${LEDGERR_BIN}" 2>/dev/null || true
    echo "b00t agent.start: ledgerr registered yei=${AGENT_ID} task=${TASK_ID}" >&2
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
