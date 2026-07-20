# ── Pre-flight checks — system-normal gate for reviewers ──────────────────
# ── Review & capability analysis ──────────────────────────────────────────
# Extracted from root justfile for module isolation.

# Run the reviewer system-normal precriteria gate (RHAI)
check-system-normal:
    @b00t script run _b00t_/scripts/reviewer-system-normal.rhai 2>/dev/null || echo "⚠️  RHAI not available — run 'just submodule-status' and check: git stash list, git branch --show-current, git status"

# Show b00t capability overview
review-capabilities:
    b00t capabilities

# Operator survey: load role + check system state
operator-survey:
    @echo "🔍 Operator survey"
    @echo "━━━━━━━━━━━━━━━━━"
    b00t learn operator 2>/dev/null | head -5
    @echo ""
    @just review-branch

# Show recent branch changes with stats
review-branch:
    @echo "📋 Branch: $$(git branch --show-current)"
    @echo ""
    @git log --oneline --stat -10
    @echo ""
    @echo "📦 Uncommitted:"
    @git status --short
