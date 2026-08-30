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
# --lib only: b00t-cli/tests/grok_semantic_e2e_test.rs fails to compile here
# (needs CARGO_BIN_EXE_b00t-cli, unavailable in this compile mode) — a
# pre-existing, unrelated issue. All dstack-provider-plan tests are added as
# #[cfg(test)] unit tests inside library source files, so --lib covers them.
test-cli filter="":
    cargo test -p b00t-cli --lib {{filter}} -- --nocapture

# Full b00t-cli library test suite (no filter) — use before marking a task complete.
test-cli-all:
    cargo test -p b00t-cli --lib -- --nocapture

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

# Generate ~/.dstack/server/config.yml from RUNPOD_API_KEY sourced via dotenv
# (~/.b00t/.env, gitignored). Never hand-author or commit this file — see
# _b00t_/learn/managing-secrets.md. dstack's RunPod backend (verified against
# the installed 0.20.28 source: core/backends/runpod/models.py's
# RunpodAPIKeyCreds + server/services/config.py's plain yaml.safe_load) only
# accepts a literal creds.api_key; there is no ${VAR} interpolation and no
# env-var-chain creds type for this backend, so the key must land in this
# file at runtime — this recipe is the one place that happens, sourced fresh
# from .env every run, never pasted or typed literally. Idempotent; 0600.
dstack-server-config:
    #!/bin/bash
    set -euo pipefail
    root="$(git rev-parse --show-toplevel)"
    set -a
    source "$root/.env"
    set +a
    : "${RUNPOD_API_KEY:?RUNPOD_API_KEY not set in $root/.env — see _b00t_/learn/managing-secrets.md}"
    GCP_PROJECT_ID="${GCP_PROJECT_ID:-app4dog}"
    AZURE_TENANT_ID="${AZURE_TENANT_ID:-}"
    AZURE_SUBSCRIPTION_ID="${AZURE_SUBSCRIPTION_ID:-}"
    mkdir -p ~/.dstack/server
    umask 077
    printf '%s\n' \
        'projects:' \
        '  - name: b00t' \
        '    backends:' \
        '      - type: runpod' \
        '        creds:' \
        '          type: api_key' \
        "          api_key: \"$RUNPOD_API_KEY\"" \
        '      - type: gcp' \
        "        project_id: \"$GCP_PROJECT_ID\"" \
        '        creds:' \
        '          type: default' \
        '      - type: aws' \
        '        creds:' \
        '          type: default' \
        '      - type: azure' \
        "        tenant_id: \"$AZURE_TENANT_ID\"" \
        "        subscription_id: \"$AZURE_SUBSCRIPTION_ID\"" \
        '        creds:' \
        '          type: default' \
        > ~/.dstack/server/config.yml
    chmod 600 ~/.dstack/server/config.yml
    echo "Wrote ~/.dstack/server/config.yml (mode 600, not git-tracked, key sourced from .env)"
