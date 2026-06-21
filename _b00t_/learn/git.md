---
GGG: "Go Get Git" - agents MUST `b00t learn git` first when encountering b00t workflows. Git is foundational to all b00t operations: checkpoints, branches, and TURBO AGILE development.

---
checkpoint: "b00t checkpoint" creates atomic progress snapshots. Runs tests/lint first, then commits with cocogitto-style message. Use `b00t checkpoint --skip-tests` to skip validation. Restores state on failure. Pattern: checkpoint often, clean history later.

---
TURBO-AGILE: "6C" methodology - Contextual-Comment, Commit-Code, Collapse-Cleanup (CULL). Comment old code with reasons BEFORE removing. Commit with context. Refactor in later audits. Makes rebasing simple and safe. DMMT: Don't Make Me Think.

---
branch-prefixes: "Three valid prefixes: feat/ (new features), fix/ (bug fixes), chore/ (maintenance). Always reference GitHub issue #. Example: feat/42-add-oauth-support, fix/137-null-pointer-crash, chore/deps-update-deps-2024"

---
cocogitto: Conventional commits enforced via cocogitto. Format: type(scope): description. Types: feat, fix, chore, docs, style, refactor, test, perf, ci, build, revert. Required for automated releases. Pre-commit hooks run cargo fmt, clippy, tests.

---
skunk-commits: "🦨 skunk commits mark stinky but functional code. NOT bad - just needs future cleanup. b00t counts skunks as refactor trigger metric. Identifying skunks is healthy retrospective practice. Remove in later CULL phase."

---
rebase-ready: "b00t favors rebase-ready history. Commented code is low-risk to remove and documents hard-learned lessons. Old code is obvious, new code is clear, cleanup is easy. Avoids git-blame dives."

---
git-stash-checkpoint: "Before destructive operations (hard reset, rebase), always checkpoint with: git stash push -m 'pre-reset backup'. Guards warn about this pattern."

---
github-cli: "Use `gh` CLI for all GitHub operations: `gh issue create`, `gh pr create`, `gh workflow view`. Repos MUST have `.github/workflows`. Check workflow status with `gh run list`."

---
workflow-branches: "Internal projects use vendor/ submodules with convention: feat/<org>/_b00t_ branch. Example: feat/elasticdotventures/_b00t_. Push to feature branches, never main directly."

# b00t:map v1
# summary: GGG pattern, TURBO-AGILE 6C, checkpoint often, cocogitto commits, feat/fix/chore branches
# tier: core
# cmds: [git stash, b00t checkpoint, gh issue create, cocogitto]

---
surgical staging: git add -A before auditing what's staged is dangerous. Pattern: git reset HEAD first, then git add ONLY target paths, verify with git diff --cached --stat. 37 unrelated files got staged in one misstep this session.

---
josh vs submodules: Josh (josh-project/josh) is a Rust git history filter that enables atomic cross-boundary PRs — subproject code lives in the parent repo, served as independent repo via proxy. Rust project uses it for miri/rust-analyzer/stdarch. Submodules fine for <5 deps with infrequent cross-changes; switch to Josh at 10+ subprojects with weekly cross-boundary PRs. joshmodule pattern: toml config with filter path, upstream, bidirectional sync. Avoids the 3-PR dance (parent→vendor→parent).
