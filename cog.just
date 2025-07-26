# cog.just

# 🔁 Validate conventional commits
validate:
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

