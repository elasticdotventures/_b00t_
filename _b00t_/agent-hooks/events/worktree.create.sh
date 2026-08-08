#!/usr/bin/env bash
# events/worktree.create.sh — git worktree isolation for sub-agents
# Fires on WorktreeCreate event from agent harness (claude-code / opencode).
# Creates a per-agent git worktree in .claude/worktrees/<agent-id>/
# so each sub-agent has its own checked-out filesystem view without
# contaminating the captain's working tree.
set -euo pipefail

AGENT_ID="${1:-${B00T_AGENT_ID:-unknown}}"
SESSION_ID="${B00T_SESSION_ID:-unknown}"
PROJECT_ROOT="${B00T_PROJECT_DIR:-${B00T_PROJECT_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || echo "$PWD")}}"
HOOK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib/worktree-env.sh
source "${HOOK_DIR}/../../../scripts/lib/worktree-env.sh"

WORKTREE_ROOT="$(b00t_default_agent_worktree_root "${PROJECT_ROOT}")"
WORKTREE_DIR="${WORKTREE_ROOT}/${AGENT_ID}"

echo "[worktree.create] agent=${AGENT_ID} project=${PROJECT_ROOT}"

# Only create worktree if we're in a git repo with 🥾.tomllmd
if [ ! -f "${PROJECT_ROOT}/.git/🥾.tomllmd" ]; then
  echo "[worktree.create] skipping — not a b00t project (no .git/🥾.tomllmd)"
  exit 0
fi

# Idempotent: skip if worktree already exists
if [ -d "${WORKTREE_DIR}" ]; then
  echo "[worktree.create] worktree already exists at ${WORKTREE_DIR}"
  exit 0
fi

# Create worktree from current HEAD
mkdir -p "${WORKTREE_ROOT}"
git -C "${PROJECT_ROOT}" worktree add --detach "${WORKTREE_DIR}" HEAD 2>/dev/null || {
  # Fallback: if HEAD is ambiguous (detached), use current commit
  REF=""
  REF=$(git -C "${PROJECT_ROOT}" rev-parse HEAD 2>/dev/null || echo "")
  if [ -n "${REF}" ]; then
    git -C "${PROJECT_ROOT}" worktree add "${WORKTREE_DIR}" "${REF}"
  else
    echo "[worktree.create] failed to create worktree — no valid ref"
    exit 1
  fi
}

b00t_init_required_submodules "${WORKTREE_DIR}"

# Set up the worktree's own _b00t_/ if needed
if [ ! -d "${WORKTREE_DIR}/_b00t_" ]; then
  mkdir -p "${WORKTREE_DIR}/_b00t_"
fi

# Symlink the project soul into the worktree's .git/
ln -sf "${PROJECT_ROOT}/.git/🥾.tomllmd" "${WORKTREE_DIR}/.git/🥾.tomllmd" 2>/dev/null || true

# Stamp per-worktree git identity with the b00t agent + session id so commits
# made here are attributable, instead of silently inheriting whatever default
# (or stale placeholder) sits in the shared repo config.
git -C "${WORKTREE_DIR}" config user.name "b00t-agent[${AGENT_ID}]" || true
git -C "${WORKTREE_DIR}" config user.email "${AGENT_ID}.${SESSION_ID}@b00t.local" || true

echo "[worktree.create] created ${WORKTREE_DIR}"
echo "B00T_WORKTREE=${WORKTREE_DIR}"
