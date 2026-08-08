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

---
josh correction: b00t maintains 60+ repos (30 submodules + 30 forks) across PromptExecution/elasticdotventures. Submodules are breaking at this scale — constant upstream sync, dirty state, 3-PR dance. Josh justified NOW not later. Rust project migrated from submodules→subtrees→Josh at similar scale. This node has c0re subset only; b00ty-verse is the full hive network.

---
submodule-moltis-b00t: origin/main references vendor/moltis-b00t commit 857aaed923c6d783bbf57a8f5537919c800aacaf; the PromptExecution GitHub URLs return repository not found/no access, and the working remote is git@github.com:elasticdotventures/moltis-b00t.git.

---
NEVER delete corrupt git objects without first proving they are BOTH corrupt AND unreachable. The safe filter is: comm -12 <(sort corrupt.txt) <(sort unreachable.txt). `git log --find-object` only searches reachable commits — it MISSES dangling commits and will give a false "safe to delete" signal. Deleting a corrupt object that is still reachable from a live branch permanently corrupts that branch. Root cause here: OS crash created corrupt loose objects from in-flight commits; reset moved HEAD but ORIG_HEAD and reflogs may still reference those objects. Always run `git fsck --unreachable` not `git log --find-object` to classify reachability before any rm on .git/objects/.

---
build-artifact staleness: A locally-built artifact's embedded git-hash/build-hash file can lag HEAD by dozens of commits with no warning. Before treating a local build (APK, binary, bundle) as representative of current source, diff its recorded build hash against current HEAD — don't assume 'a file exists in build/' means 'built from current source'.

---
bare-repo-worktree-only: Never edit files or run builds directly in a bare repo's own top-level directory. If a bare repo (core.bare=true) has stray working files sitting in it, treat that as a legacy hazard, not a working tree — always 'git worktree add <path> <branch>' first. A concurrent process/session can check out a different branch in that same directory underneath you, silently mixing your uncommitted edits into someone else's tree.

---
worktree-on-real-disk: Create git worktrees for compiled-language work on real disk, never /tmp. tmpfs is RAM-backed, often shared and contended across every concurrent session on the host, and can silently hit 0 bytes free mid-build with no warning.
