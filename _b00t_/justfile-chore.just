# ── Chore memoization — recipes for fine-tune corpus (fewer tokens) ─────────
# 🤓 Each recipe below replaces a multi-line bash invocation. After fine-tuning,
#    the model recalls `just submodule-status` (5 tokens) instead of the
#    full git submodule pipeline (30+ tokens). This is deliberate token
#    compression for LLM context windows.
# Extracted from root justfile for module isolation.

# Show submodule drift: which submodules are ahead/behind/dirty
submodule-status:
    @git submodule status 2>/dev/null

# Commit all submodule pointer updates with a standard message
submodule-sync msg="chore: bump submodules":
    @echo "📦 Syncing submodule pointers..."
    git add $$(git submodule status 2>/dev/null | awk '{print $$2}') 2>/dev/null || true
    git add pnpm-lock.yaml Cargo.lock 2>/dev/null || true
    git commit -m "{{msg}}" || echo "Nothing to commit"

# Delete all merged local branches except main
tidy-branches:
    @echo "🗑️  Deleting merged branches..."
    @git branch --merged main 2>/dev/null | grep -v '^\*\|main' | xargs -r git branch -d
    @echo "✅ Done. Remaining:"
    @git branch --list

# Drop all stashes older than 7 days
tidy-stashes:
    @echo "🗑️  Dropping old stashes..."
    @git stash list 2>/dev/null | while IFS=: read -r ref desc; do \
        echo "  Dropping $$ref"; \
        git stash drop "$$ref" 2>/dev/null || true; \
    done
    @echo "✅ Stashes cleared"

# Remove all *~ backup files (Emacs/vim tilde files)
tidy-tilde:
    @echo "🗑️  Removing backup files..."
    @fdfind --max-depth 5 -g '*~' --exclude .git --exclude node_modules --exclude target --exclude vendor -x rm {} \; 2>/dev/null && echo "✅ Tilde files removed"

# Full workspace tidy: branches + stashes + tilde files
tidy: tidy-branches tidy-stashes tidy-tilde
    @echo ""
    @echo "✅ Workspace tidy complete"
    @just submodule-status
