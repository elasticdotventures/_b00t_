#!/usr/bin/env bash
# b00t hook: agent.stop  (SubagentStop)
#
# Fires when a subagent finishes. CAN block (exit 2) to prevent the subagent
# from completing — useful for requiring a retry or injecting follow-up work.
#
# Lifecycle:
#   1. Ledgerr audit receipt — record completion, release resource budget
#   2. Worktree cleanup — remove per-agent git worktree from agent.start
#   3. Role-specific hooks — roles/<agent-type>/agent.stop.sh
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
AGENT_ID="$(  echo "$INPUT" | jq -r '.agent_id            // "unknown"' 2>/dev/null || echo "unknown")"
LAST_MSG="$(  echo "$INPUT" | jq -r '.last_assistant_message // ""'    2>/dev/null || echo "")"

PROJECT_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "")"

# ── Phase 1: Ledgerr audit receipt (release resource budget) ──────────────
if [ -n "${PROJECT_ROOT}" ] && [ "${B00T_NO_LEDGERR:-0}" != "1" ]; then
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
    # Log completion receipt — ledgerr records the audit trail
    printf '{"jsonrpc":"2.0","method":"tools/call","id":1,"params":{"name":"ledgerr_b00t_complete_task","arguments":{"agent_id":"%s","task_id":"%s","status":"completed"}}}\n' \
      "${AGENT_ID}" "${AGENT_ID}" \
      | "${LEDGERR_BIN}" 2>/dev/null || true
    echo "b00t agent.stop: ledgerr receipt yei=${AGENT_ID}" >&2
  fi
fi

# ── Phase 2: Worktree cleanup ────────────────────────────────────────────
if [ -n "${PROJECT_ROOT}" ]; then
  WT_DIR="${PROJECT_ROOT}/.b00t/worktrees/${AGENT_ID}"
  if [ -d "${WT_DIR}" ]; then
    git -C "${PROJECT_ROOT}" worktree remove "${WT_DIR}" --force 2>/dev/null || {
      rm -rf "${WT_DIR}"
      git -C "${PROJECT_ROOT}" worktree prune 2>/dev/null || true
    }
    echo "b00t agent.stop: removed worktree ${WT_DIR}" >&2
  fi
fi

# Load role-specific agent.stop hook if defined
ROLE_HOOK="${B00T_DIR:-$HOME/.b00t/_b00t_}/hooks/roles/$AGENT_TYPE/agent.stop.sh"
if [ -x "$ROLE_HOOK" ]; then
    echo "$INPUT" | "$ROLE_HOOK"
    exit $?
fi

# Default: allow
exit 0
