# 🤓 Subagent-driven-development execution recipes for the dstack provider plan
#    (docs/superpowers/plans/2026-07-22-b00t-dstack-provider-plan.md).
#    Wraps repeated controller-side bash into named recipes per operator
#    steer 2026-07-22: prefer `just <recipe>` over raw bash in agent turns.

sdd_scripts := "/home/brianh/.claude/plugins/cache/claude-plugins-official/superpowers/6.1.1/skills/subagent-driven-development/scripts"

# Step 0 detection from superpowers:using-git-worktrees — prints whether the
# current directory is already an isolated worktree, and its branch.
worktree-check:
    #!/bin/bash
    set -euo pipefail
    git_dir=$(cd "$(git rev-parse --git-dir)" && pwd -P)
    git_common=$(cd "$(git rev-parse --git-common-dir)" && pwd -P)
    branch=$(git branch --show-current)
    if git rev-parse --show-superproject-working-tree >/dev/null 2>&1 && [ -n "$(git rev-parse --show-superproject-working-tree)" ]; then
        echo "SUBMODULE: treat as normal repo checkout"
    elif [ "$git_dir" != "$git_common" ]; then
        echo "ISOLATED: already in a linked worktree, branch=$branch"
    else
        echo "NORMAL: plain checkout, branch=$branch — worktree-create needed"
    fi

# Create (or reuse) an isolated worktree for the named branch under .worktrees/,
# ensuring .worktrees/ is gitignored first.
worktree-create branch:
    #!/bin/bash
    set -euo pipefail
    root=$(git rev-parse --show-toplevel)
    if ! git check-ignore -q .worktrees 2>/dev/null; then
        echo ".worktrees" >> "$root/.gitignore"
        git add "$root/.gitignore"
        git commit -m "chore: ignore .worktrees/ for subagent-driven-development isolation"
    fi
    path="$root/.worktrees/{{branch}}"
    if [ -d "$path" ]; then
        echo "REUSE: $path"
    else
        git worktree add "$path" -b "{{branch}}"
        echo "CREATED: $path"
    fi

# Scoped, fast-feedback test run for the b00t-cli crate, optionally filtered
# to a single test name (matches cargo's own filter substring semantics).
test-cli filter="":
    cargo test -p b00t-cli {{filter}} -- --nocapture

# Full b00t-cli test suite (no filter) — use before marking a task complete.
test-cli-all:
    cargo test -p b00t-cli -- --nocapture

# Thin wrapper around subagent-driven-development's task-brief script —
# extracts one task's full text from the plan to a uniquely named file,
# prints the path.
task-brief plan n:
    {{sdd_scripts}}/task-brief {{plan}} {{n}}

# Thin wrapper around subagent-driven-development's review-package script —
# writes commit list + diffstat + full diff for BASE..HEAD to a uniquely
# named file, prints the path.
review-package base head:
    {{sdd_scripts}}/review-package {{base}} {{head}}

# Resolve (and ensure) the SDD artifact workspace directory (briefs, reports,
# ledger) — same directory task-brief/review-package already write into.
sdd-workspace:
    {{sdd_scripts}}/sdd-workspace

# Print the progress ledger, or "no ledger yet" if this is a fresh run.
ledger-show:
    #!/bin/bash
    set -euo pipefail
    f="$(git rev-parse --show-toplevel)/.superpowers/sdd/progress.md"
    if [ -f "$f" ]; then cat "$f"; else echo "no ledger yet"; fi

# Append one line to the progress ledger (creates it if needed).
ledger-append line:
    #!/bin/bash
    set -euo pipefail
    f="$(git rev-parse --show-toplevel)/.superpowers/sdd/progress.md"
    mkdir -p "$(dirname "$f")"
    echo "{{line}}" >> "$f"
