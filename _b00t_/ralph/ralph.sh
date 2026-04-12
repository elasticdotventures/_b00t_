#!/bin/bash
# Ralph - Autonomous coding agent wrapper
# Handles setup, initialization, preflight checks before delegating to Python runtime

set -e

# Find script directory (handles symlinks)
SOURCE="${BASH_SOURCE[0]}"
while [ -h "$SOURCE" ]; do
    DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"
    TARGET="$(readlink "$SOURCE")"
    if [[ "$TARGET" = /* ]]; then
        SOURCE="$TARGET"
    else
        SOURCE="$DIR/$TARGET"
    fi
done
SCRIPT_DIR="$(cd -P "$(dirname "$SOURCE")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

info() {
    echo -e "${GREEN}✓${NC} $1"
}

warn() {
    echo -e "${YELLOW}⚠${NC} $1" >&2
}

error() {
    echo -e "${RED}✗${NC} $1" >&2
    exit 1
}

# Display instructions for creating backlog items
show_task_creation_instructions() {
    echo ""
    echo "To create backlog items with the ralph-prd skill, run your designated agent with this prompt:"
    echo ""
    cat <<'EOF'
Use the ralph-prd skill to generate TODO-next.md backlog items for this repo.
Requirements:
- Output must be markdown with checklist items under clear headings.
- Include 3-7 small, actionable backlog items with acceptance criteria in prose.
- Use IETF 2119 MUST/SHOULD/MAY in acceptance criteria.
- Put critical-path items first.
EOF
    echo ""
    echo "Then re-run: ./ralph.sh --agent <amp|claude|codex> [max_iterations]"
}

# 1. Find git repository root
find_git_root() {
    local dir="$PWD"
    while [[ "$dir" != "/" ]]; do
        if [[ -d "$dir/.git" ]]; then
            echo "$dir"
            return 0
        fi
        dir="$(dirname "$dir")"
    done
    return 1
}

GIT_ROOT=$(find_git_root) || error "Not in a git repository"
info "Git root: $GIT_ROOT"

# 2. Ensure dependencies are synced (uv only, never pip)
if ! command -v uv &> /dev/null; then
    error "uv not found. Install with: curl -LsSf https://astral.sh/uv/install.sh | sh"
fi

# Prefer a repo-local uv cache to avoid permission issues in restricted sandboxes.
export UV_CACHE_DIR="${UV_CACHE_DIR:-$GIT_ROOT/.uv-cache}"
mkdir -p "$UV_CACHE_DIR"

info "Syncing dependencies with uv..."
uv sync --quiet || error "uv sync failed (check permissions/network)."

# 3. Require TODO-next.md backlog to exist before running
BACKLOG_FILE="$GIT_ROOT/TODO-next.md"
if [[ ! -f "$BACKLOG_FILE" ]]; then
    warn "TODO-next.md not found; nothing to do."
    show_task_creation_instructions
    exit 1
fi

uv run python - <<PY
from pathlib import Path

backlog_path = Path("$BACKLOG_FILE")
lines = backlog_path.read_text().splitlines()
items = [line for line in lines if line.strip().startswith("- [ ] ") or line.strip().startswith("- [x] ") or line.strip().startswith("- [X] ")]
if not items:
    raise SystemExit(3)
PY
BACKLOG_CHECK_EXIT=$?
if [[ $BACKLOG_CHECK_EXIT -ne 0 ]]; then
    warn "TODO-next.md has no checklist items; nothing to do."
    show_task_creation_instructions
    exit 1
fi

# 4. All preflight checks passed, delegate to Python runtime
info "Initialization complete, starting Ralph runtime..."
echo ""

exec uv run ralph "$@"
