# cog.just

# 🔁 Validate conventional commits (only recent commits to avoid historical issues)
validate:
    cog check HEAD~10..HEAD

# 🔍 Validate specific range of commits (usage: just cog validate-range HEAD~20..HEAD)
validate-range RANGE:
    cog check {{RANGE}}

# 🔍 Validate all commits (will show historical errors)
validate-all:
    cog check

# 📝 Preview next version and changelog
changelog:
    cog changelog

# 🚀 Release new version based on commit history
release:
    cog bump --auto
    cog changelog
    git push --follow-tags

# 🔧 Manual version bump (use `major`, `minor`, or `patch`)
bump VERSION:
    cog bump {{VERSION}}
    cog changelog
    git push --follow-tags

# 📊 Show commit statistics
stats:
    @echo "Recent commit validation (last 10 commits):"
    @cog check HEAD~10..HEAD || echo "Some commits don't follow conventional format"
    @echo ""
    @echo "Total commits in repository:"
    @git rev-list --count HEAD
    @echo ""
    @echo "Recent commits:"
    @git log --oneline -10

# ✅ Verify a commit message format
verify MESSAGE:
    cog verify "{{MESSAGE}}"

# 💡 Show examples of valid commit messages
examples:
    @echo "Valid commit message examples:"
    @echo ""
    @echo "Standard types:"
    @echo "  feat: add user authentication system"
    @echo "  fix: resolve memory leak in session handler"
    @echo "  docs: update API documentation"
    @echo "  chore: bump dependency versions"
    @echo ""
    @echo "WIP commits (will be squashed):"
    @echo "  wip: implementing user auth - partial progress"
    @echo "  wip: debugging session issues"
    @echo ""
    @echo "With scope:"
    @echo "  feat(auth): add JWT token validation"
    @echo "  fix(api): handle null pointer in user service"

