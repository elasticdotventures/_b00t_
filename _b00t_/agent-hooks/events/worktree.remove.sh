#!/usr/bin/env bash
# events/worktree.remove.sh — cleanup sub-agent git worktree
# Fires on WorktreeRemove event from agent harness.
set -euo pipefail

AGENT_ID="${1:-${B00T_AGENT_ID:-unknown}}"
PROJECT_ROOT="${B00T_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || echo "$PWD")}"
WORKTREE_DIR="${PROJECT_ROOT}/.b00t/worktrees/${AGENT_ID}"

echo "[worktree.remove] agent=${AGENT_ID}"

if [ ! -d "${WORKTREE_DIR}" ]; then
  echo "[worktree.remove] no worktree to remove"
  exit 0
fi

# Remove worktree
git -C "${PROJECT_ROOT}" worktree remove "${WORKTREE_DIR}" --force 2>/dev/null || {
  # Force remove if git worktree remove fails
  rm -rf "${WORKTREE_DIR}"
  git -C "${PROJECT_ROOT}" worktree prune 2>/dev/null || true
}

echo "[worktree.remove] removed ${WORKTREE_DIR}"
