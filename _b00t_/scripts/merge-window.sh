#!/usr/bin/env bash
# ──────────────────────────────────────────────────────────────
# merge-window.sh — b00t merge window procedure
# Ask [y/N] before push. Exit early if repo is clean.
# Usage: ./_b00t_/scripts/merge-window.sh [--dry-run]
# ──────────────────────────────────────────────────────────────
set -euo pipefail

DRY_RUN="${1:-}"
B00T_ROOT="${B00T_ROOT:-$HOME/.b00t}"

echo "═══════════════════════════════════════════"
echo "  🥾 b00t merge window — $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "═══════════════════════════════════════════"

# Step 0: are we in a git repo?
cd "$B00T_ROOT"
if ! git rev-parse --git-dir >/dev/null 2>&1; then
    echo "❌ Not a git repository at $B00T_ROOT"
    exit 1
fi

# Step 1: exit early if repo clean
BRANCH=$(git rev-parse --abbrev-ref HEAD)
echo "📋 Branch: $BRANCH"

if [ -z "$(git status --porcelain)" ]; then
    echo "✅ Working tree clean — nothing to merge."
    echo "   (Use 'b00t checkpoint' if you intended to commit first.)"
    exit 0
fi

# Step 2: check branch safety
if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "dev" ] || [ "$BRANCH" = "master" ]; then
    echo "⚠️  You are on branch '$BRANCH'. Merge window should be done from a feature branch."
    echo "   Switch to a feature branch and retry."
    exit 1
fi

# Step 3: verify tasks are done
echo ""
echo "📋 Checking b00t task backlog..."
B00T_TASKS_DONE=$(b00t task list --status=done 2>/dev/null | head -1 || echo "0")
echo "   Done tasks: $B00T_TASKS_DONE"

# Step 4: show diffstat
echo ""
echo "📊 Diffstat (vs main):"
git diff --stat main..HEAD 2>/dev/null || git diff --stat 2>/dev/null | tail -20

# Step 5: show commit summary
echo ""
echo "📝 Commits on this branch:"
git log --oneline --first-parent "$(git merge-base HEAD main 2>/dev/null || echo 'main')..HEAD" 2>/dev/null || echo "   (cannot compute)"

# Step 6: cargo check
echo ""
echo "🧪 Running cargo check..."
if ! cargo check 2>&1 | tail -5; then
    echo "❌ cargo check FAILED — aborting merge window."
    exit 1
fi
echo "✅ cargo check passed"

# Step 7: run tests (optional subset)
echo ""
echo "🧪 Running tests..."
if ! cargo test --lib 2>&1 | tail -5; then
    echo "❌ Some tests FAILED — review before pushing."
    echo "   Press Ctrl+C to abort, or continue to push anyway."
fi

# Step 8: confirm push
echo ""
echo "═══════════════════════════════════════════"
echo "  🚀 Ready to push branch '$BRANCH'"
echo "═══════════════════════════════════════════"

if [ "$DRY_RUN" = "--dry-run" ]; then
    echo "  [DRY RUN] Would push to origin/$BRANCH"
    echo "  [DRY RUN] Would open PR against main"
    exit 0
fi

read -r -p "Push branch '$BRANCH' to origin and open PR? [y/N] " CONFIRM
if [ "$CONFIRM" != "y" ] && [ "$CONFIRM" != "Y" ]; then
    echo "🛑 Push cancelled by user."
    exit 0
fi

# Step 9: push
echo ""
echo "🚀 Pushing to origin/$BRANCH..."
git push origin "$BRANCH"

# Step 10: open PR via gh
if command -v gh &>/dev/null; then
    echo ""
    echo "🔗 Opening PR against main..."
    gh pr create --base main --head "$BRANCH" --fill 2>&1 || \
        echo "   (gh pr create failed — maybe PR already exists)"
else
    echo ""
    echo "  (gh CLI not available — open PR manually)"
fi

echo ""
echo "✅ Merge window complete. Branch '$BRANCH' pushed."
echo "   🍰 Cake earned."
