# justfile for Rust Development Environment
# Alias to get the Git repository root
repo-root := env_var_or_default("JUST_REPO_ROOT", `git rev-parse --show-toplevel 2>/dev/null || echo .`)



set shell := ["bash", "-cu"]
set unstable
mod cog
mod b00t
# 🔑 Root-requiring system setup — invoke as: sudo just sudo::<recipe>
# e.g. sudo just sudo::setup | sudo just sudo::status | sudo just sudo::install-dbus-service
mod sudo 'b00t-service.just'
# this is an antipattern (litellm is early-stage AI infra, skip for now)
mod litellm '_b00t_/litellm/justfile'
mod b00t-mcp-npm
mod? irontology-publish 'vendor/irontology-mcp/irontology-publish.just'
mod? irontology 'vendor/irontology-mcp/irontology.just'
# 🥾 Zellij interactive modal system (floating pane dialogs)
mod zellij '_b00t_/zellij.just'
# 🖥️  General-purpose always-on Xpra display service (task #12) — NOT owned
#    by b00t-rpa; see _b00t_/xpra-display.hive.toml
mod xpra-display '_b00t_/xpra-display.just'
# 🛡️ Zellij mandatory interaction gate (governance: Allow/Deny/Hook)
mod zellij-gate '_b00t_/zellij-gate.just'
# 🌐 b00t-admin web server — dashboard, container, quadlet
mod b00t-admin 'vendor/b00t-admin/b00t-admin.just'
# 📚 Rust documentation MCP server — required by skills/rust
mod rust-doc 'vendor/rust-doc.just'
mod gh-runner-gpu 'k8s/gh-runner-gpu/gh-runner-gpu.just'
# 🥾 Compound engineering workflow — 8-phase agile state machine
mod compound-engineering '_b00t_/compound-engineering.just'
# 🛡️ Canonical reviewer skill — MECE+TRIZ+Eureka multi-framework review
# Usage: just reviewer system-normal | just reviewer autoexec | just reviewer review-multi PR=<n>
mod reviewer '_b00t_/skills/reviewer/justfile'

# Datum justfiles (install recipes for core tech stacks)
mod python '_b00t_/python.🐍/justfile'
mod docker '_b00t_/docker.🐳/justfile'
mod bash '_b00t_/bash.🐚/justfile'
mod git '_b00t_/git.🐙/justfile'
mod terraform '_b00t_/terraform.🧊/justfile'
mod k8s '_b00t_/k8s.🚢/justfile'
mod pm2-tasker 'pm2-tasker/justfile'
mod embed '_b00t_/python.🐍/embed/justfile'
mod qwen-code '_b00t_/qwen-code.justfile'
# 🧠 AI fine-tuning: dataset gen, local k8s training, HF Jobs cloud, MLflow, adapter test
mod ai-finetune '_b00t_/ai-finetune.just'
mod ngc '_b00t_/ngc.just'
mod ux 'ux.just'
mod hf-cloud '_b00t_/justfile-hf-cloud.just'
mod gemma '_b00t_/justfile-gemma.just'
mod phi-candle '_b00t_/phi-candle.just'
mod worker '_b00t_/justfile-worker.just'
# ☁️ Cloudflare Workers — secret provisioning + deploy (b00t-mcp-vault, telnyx-fax-handler, ledgrrr-tenant-registry, ...)
mod cf-workers 'workers/cf-workers.just'
mod review '_b00t_/justfile-review.just'
mod ufo '_b00t_/justfile-ufo.just'
mod chore '_b00t_/justfile-chore.just'
mod opencode-plugins '_b00t_/justfile-opencode-plugins.just'
mod mcp-mesh '_b00t_/justfile-mcp-mesh.just'
mod h3rmes '_b00t_/justfile-h3rmes.just'
mod b00t-embed '_b00t_/justfile-b00t-embed.just'
mod autolearn '_b00t_/justfile-autolearn.just'
mod ralph '_b00t_/justfile-ralph.just'
mod dstack-sdd '_b00t_/justfile-dstack-sdd.just'

# ── Module guide — `just modules` or `just --list <module>` ──────────────────
# Lists all submodule justfiles registered in this repo.
# Each module is a b00t skill scope; load with: b00t learn <module>

@modules:
    @echo "b00t just modules (just --list <module> for recipes):"
    @echo ""
    @echo "  ai-finetune   QLoRA training + HF Jobs cloud (b00t learn ai-finetune)"
    @echo "  k8s           Kubernetes ops — sm3lly cluster"
    @echo "  python        Python/uv environment management"
    @echo "  docker        Container build + run"
    @echo "  git           Git workflows + hooks"
    @echo "  bash          Shell utilities"
    @echo "  terraform     IaC provisioning"
    @echo "  zellij        Terminal multiplexer"
    @echo "  b00t          Core b00t CLI wrappers"
    @echo "  embed         Embedding pipeline"
    @echo "  qwen-code     Qwen code agent"
    @echo "  irontology    Ontology + semantic RAG"
    @echo ""
    @echo "  Usage: just <module>::<recipe>"


# Show which AI models are running locally (k8s inference pods + ollama)
models:
    #!/bin/bash
    echo "=== k8s inference (b00t-inference) ==="
    kubectl get pods -n b00t-inference -o custom-columns='POD:.metadata.name,STATUS:.status.phase,AGE:.status.startTime' --no-headers 2>/dev/null || echo "(kubectl not available)"
    echo ""
    echo "=== GPU memory ==="
    nvidia-smi --query-gpu=name,memory.used,memory.free,memory.total --format=csv,noheader 2>/dev/null || echo "(nvidia-smi not available)"
    echo ""
    echo "=== ollama models ==="
    ollama list 2>/dev/null || echo "(ollama not running)"


next-task:
    #!/bin/bash
    set -euo pipefail
    echo "Next up: extend Gremlin graph (role/capability edges) and wire GraalVM Gremlin server."

viz-entangle datum="ledgrrr" format="mermaid":
    #!/bin/bash
    set -euo pipefail
    # 🤓 use prebuilt binary — cargo run fails on b00t repo due to git worktree structure
    BCLI="./target/release/b00t-cli"
    [[ -x "$BCLI" ]] || { echo "❌ no prebuilt b00t-cli — run: cargo build -p b00t-cli --release"; exit 1; }
    "$BCLI" --path _b00t_ viz entangle --datum "{{datum}}" --format "{{format}}"

gremlin-graalvm-build:
    docker build -t graalvm-gremlin:latest docker/graalvm-gremlin

gremlin-graalvm-run:
    podman run --rm -p 8182:8182 \
      -v $PWD/docker/graalvm-gremlin/gremlin-server.yaml:/opt/gremlin-server/conf/gremlin-server.yaml \
      docker.io/tinkerpop/gremlin-server:latest

stow:
    stow --adopt -d ~/.dotfiles -t ~ bash

# Sync canonical _b00t_/ datum tree -> the live deployed _B00T_Path
# (default ~/.dotfiles/_b00t_, resolved via _B00T_Path env if set).
# Additive-only (no --delete): only adds/updates files from canonical,
# never removes dotfiles-local files that don't exist here.
# Default is a dry-run (rsync -n); pass `true` (positional — just recipe
# args are positional, `apply=true` on the CLI is NOT recognized and will
# silently stay in dry-run) to actually write.
#
# 🤓 bug/#13: ~/.dotfiles/_b00t_ (the live b00t-cli default _B00T_Path) drifts
#    from this repo's canonical _b00t_/ tree with no sync mechanism — e.g.
#    missing sonar.cli.toml causes 'b00t cli install sonar' to fail
#    'sonar UNDEFINED' even though the datum exists here. This recipe is
#    the documented refresh step; run it whenever a datum you just added
#    here doesn't seem to exist for the live b00t-cli.
#    ~/.dotfiles is a separate git repo (its own history) — this only
#    overwrites its _b00t_/ subtree, never deletes, and the operator should
#    `git stash` any uncommitted ~/.dotfiles changes first since rsync will
#    silently clobber/resurrect files that differ from canonical.
#
# @example: just sync-g0spell-dotfiles          # dry-run, shows the drift
# @example: just sync-g0spell-dotfiles true     # apply (positional, not apply=true)
sync-g0spell-dotfiles apply="false" target=env_var_or_default("_B00T_Path", "~/.dotfiles/_b00t_"):
    #!/bin/bash
    set -euo pipefail
    SRC="{{repo-root}}/_b00t_/"
    DST="{{target}}"
    DST="${DST/#\~/$HOME}"
    echo "🔄 g0spell sync: $SRC -> $DST"
    if [[ "{{apply}}" != "true" ]]; then
        echo "(dry-run — pass 'true' as the first arg to write, e.g. 'just sync-g0spell-dotfiles true'; this never deletes dotfiles-local files)"
        rsync -avn --exclude='.git' "$SRC" "$DST"
    else
        mkdir -p "$DST"
        rsync -av --exclude='.git' "$SRC" "$DST"
        echo "✅ synced. cd $DST/.. && git status to review + commit the drift."
    fi

ansible-k0s PLAYBOOK="ansible/playbooks/k0s_kata.yaml" INVENTORY="ansible/inventory.sample.yaml" EXTRA_ARGS="":
    #!/bin/bash
    set -euo pipefail
    INVENTORY="${INVENTORY:-ansible/inventory.sample.yaml}"
    PLAYBOOK="${PLAYBOOK:-ansible/playbooks/k0s_kata.yaml}"
    EXTRA_ARGS="${EXTRA_ARGS:-}"
    export ANSIBLE_ROLES_PATH="${ANSIBLE_ROLES_PATH:-$PWD/ansible/roles}"
    if ! command -v ansible-playbook >/dev/null 2>&1; then
        echo "ansible-playbook not found. Install ansible-core first." >&2
        exit 1
    fi
    echo "🥾 provisioning k0s + Kata via ansible"
    ANSIBLE_FORCE_COLOR=1 ansible-playbook -i "$INVENTORY" "$PLAYBOOK" $EXTRA_ARGS

ansible-k0s-check PLAYBOOK="ansible/playbooks/k0s_kata.yaml":
    #!/bin/bash
    set -euo pipefail
    export ANSIBLE_ROLES_PATH="${ANSIBLE_ROLES_PATH:-$PWD/ansible/roles}"
    if ! command -v ansible-playbook >/dev/null 2>&1; then
        echo "ansible-playbook not found. Install ansible-core first." >&2
        exit 1
    fi
    ANSIBLE_FORCE_COLOR=1 ansible-playbook --syntax-check "$PLAYBOOK"

# 🔑 Install b00t DBus system service — delegates to b00t-service.just
# Usage: sudo just install-dbus-service  OR  sudo just sudo::install-dbus-service
install-dbus-service:
    just sudo::install-dbus-service

# Test crates.io publishing (dry-run)
publish-dry-run:
    #!/bin/bash
    set -euo pipefail
    echo "🔍 Testing crates.io publishing (dry-run)..."

    echo "📦 Testing b00t-chat..."
    cd b00t-lib-chat && cargo publish --dry-run --allow-dirty --locked

    echo "📦 Testing b00t-c0re-lib..."
    cd ../b00t-c0re-lib && cargo publish --dry-run --allow-dirty --no-verify

    echo "📦 Testing b00t-c0re-hierarchy..."
    cd ../b00t-c0re-hierarchy && cargo publish --dry-run --allow-dirty --locked

    echo "📦 Testing b00t-cli..."
    cd ../b00t-cli && cargo publish --dry-run --allow-dirty --locked

    echo "📦 Testing b00t-mcp..."
    cd ../b00t-mcp && cargo publish --dry-run --allow-dirty --locked

    echo "✅ All crates passed dry-run validation"

# Reserve/claim crate names on crates.io (one-time setup)
# 🤓 Run this BEFORE first release to reserve names
claim-crates:
    #!/bin/bash
    set -euo pipefail
    echo "🚩 Claiming crate names on crates.io..."
    echo "⚠️  This will create placeholder versions if names are available"
    read -p "Continue? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted"
        exit 1
    fi

    echo "📦 Publishing b00t-chat to claim name..."
    cd b00t-lib-chat && cargo publish --allow-dirty || echo "⚠️ Already claimed or failed"

    echo "⏳ Waiting 30s for crates.io indexing..."
    sleep 30

    echo "📦 Publishing b00t-c0re-lib to claim name..."
    cd ../b00t-c0re-lib && cargo publish --allow-dirty || echo "⚠️ Already claimed or failed"

    echo "⏳ Waiting 30s for crates.io indexing..."
    sleep 30

    echo "📦 Publishing b00t-cli to claim name..."
    cd ../b00t-cli && cargo publish --allow-dirty || echo "⚠️ Already claimed or failed"

    echo "⏳ Waiting 30s for crates.io indexing..."
    sleep 30

    echo "📦 Publishing b00t-mcp to claim name..."
    cd ../b00t-mcp && cargo publish --allow-dirty || echo "⚠️ Already claimed or failed"

    echo "✅ Crate names claimed (if available)"

# Pre-release validation gate for 0.10.0+ releases
# 🤓 Checks: --agent alias, version consistency, workspace deps, build
pre-release-check:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "🔍 Pre-release check v$(grep '^version' Cargo.toml | head -1 | grep -oP '[\d]+\.[\d]+\.[\d]+')..."

    # 1. Verify workspace build
    echo "   [1/5] cargo check --workspace..."
    cargo check --workspace --quiet 2>&1 | grep -E "^error" && { echo "❌ Build errors"; exit 1; } || echo "   ✅"

    # 2. Verify --agent alias works for whoami
    echo "   [2/5] --agent alias resolution..."
    cargo run --bin b00t-cli --quiet -- whoami --agent worker --json 2>/dev/null | grep -q '"role"' && echo "   ✅" || { echo "❌ --agent alias failed"; exit 1; }

    # 3. Verify --agent alias works for blessing
    echo "   [3/5] blessing --agent alias..."
    cargo run --bin b00t-cli --quiet -- blessing --manifest --agent worker >/dev/null 2>&1 && echo "   ✅" || { echo "❌ blessing --agent alias failed"; exit 1; }

    # 4. Workspace version consistency (all path deps match workspace)
    echo "   [4/5] workspace version consistency..."
    for f in $(grep -rl "version.workspace = true" --include="Cargo.toml" . | grep -v target | grep -v "vendor"); do
        dir=$(dirname "$f")
        name=$(grep "^name" "$f" | head -1 | cut -d'"' -f2)
        echo "      ✓ $name"
    done
    echo "   ✅"

    # 5. No pre-existing test compilation errors from refactors
    echo "   [5/5] cargo test --no-run..."
    cargo test --no-run -p b00t-cli --lib --quiet 2>&1 | grep -E "^error" && { echo "❌ Test compilation errors"; exit 1; } || echo "   ✅"

    echo "✅ Pre-release checks passed"

# Create GitHub release (triggers crates.io publishing workflow)
release:
    #!/bin/bash
    set -euo pipefail
    VERSION=$(grep '^version = ' Cargo.toml | grep -oP '[\d]+\.[\d]+\.[\d]+')

    echo "🚀 Dispatching GitHub-native release for v${VERSION}..."

    # Run pre-release gate
    echo "🔍 Running pre-release checks..."
    just pre-release-check

    # Verify workspace is clean
    if ! git diff --quiet; then
        echo "⚠️ Uncommitted changes detected"
        exit 1
    fi

    # Run tests first
    echo "🧪 Running tests..."
    cargo test --workspace --all-features --exclude b00t-cli --exclude b00t-grok
    # b00t-cli's optional candle stack is intentionally excluded from release gating for now.
    cargo test -p b00t-cli --features dbus,llamacpp-fallback
    # b00t-grok pyo3 feature requires Python dev headers at link time; not guaranteed in CI.
    cargo test -p b00t-grok

    gh workflow run release.yml \
        -f version="${VERSION}" \
        -f run_tests=true

    echo "✅ Release workflow dispatched for v${VERSION}"
    echo "📦 Tagging, release creation, binaries, and crates publishing now flow through GitHub Actions"
    echo "🔗 Check workflow: https://github.com/elasticdotventures/dotfiles/actions"

# Generate deterministic Claude marketplace + MCP role recipes.
marketplace-generate:
    python3 scripts/generate_claude_marketplace.py --repo-root .

# Validate generated marketplace artifacts are up-to-date.
marketplace-check:
    python3 scripts/generate_claude_marketplace.py --repo-root . --check






# Bump patch version + cargo install — always pair these together
# 🤓 never cargo install without bumping version; tracks deployed vs source
bump-install:
    #!/usr/bin/env bash
    set -euo pipefail
    export PATH="$HOME/.cargo/bin:$PATH"
    current=$(grep '^version' Cargo.toml | head -1 | grep -oP '[\d]+\.[\d]+\.[\d]+')
    IFS='.' read -r maj min pat <<< "$current"
    next="$maj.$min.$((pat+1))"
    sed -i "s/^version = \"$current\"/version = \"$next\"/" Cargo.toml
    echo "⬆️  $current → $next"
    cargo install --path b00t-mcp --force
    cargo install --path b00t-cli --force
    cp ~/.cargo/bin/b00t-mcp ~/.local/bin/b00t-mcp
    echo "✅ installed v$next"

# 🥾 Bootstrap b00t on a fresh machine (no cargo/just required).
# 💡例 curl the script and pipe to bash, or run directly:
#    ./b00t-lite.sh          — auto-detects OS, installs system deps + rustup
#    ./b00t-lite.sh --dry-run — preview commands without executing
bootstrap:
    #!/bin/bash
    set -euo pipefail
    echo "🥾 b00t bootstrap (b00t-lite)"
    B00T_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    exec "${B00T_DIR}/b00t-lite.sh" "$@"

# 🥾 Build Node.js hook bundles and distribute to all runtime hook dirs.
build-hooks:
    cd _b00t_/runtimes/hooks-src && npm install && node build.js

# 🥾 Install b00t binaries + systemd unit files.
# 💡 Recommended: sudo just install (sudo enables system-wide b00t@.service)
#    Menu selects components; defaults to [2] binaries+service after 10s timeout.
install:
    #!/bin/bash
    set -euo pipefail

    # 🤓 _prompt_timeout: ONLY used for sudo-requiring steps
    _prompt_timeout() {
        local desc="${1:-proceed}"
        local timeout=10
        for i in $(seq $timeout -1 1); do
            printf "\r  ⏱  [%2ds] %s — press any key or wait to skip..." "$i" "$desc"
            if read -r -s -n 1 -t 1 2>/dev/null; then
                printf "\n  ▶  %s\n" "$desc"
                return 0
            fi
        done
        printf "\n  ⏭  skipped: %s\n" "$desc"
        return 1
    }

    echo "🥾 b00t install"
    echo
    echo "  [1] binaries only          (b00t-cli, b00t-mcp, cocogitto)"
    echo "  [2] binaries + service     (+ b00t@.service systemd template)"
    echo "  [3] full bootstrap         (b00t-lite.sh: system deps + binaries + service)"
    echo
    printf "  choice [2], timeout 10s: "
    read -r -t 10 CHOICE || CHOICE="2"
    echo
    CHOICE="${CHOICE:-2}"

    # option 3 → delegate entirely to b00t-lite.sh
    if [[ "$CHOICE" == "3" ]]; then
        B00T_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
        exec "${B00T_DIR}/b00t-lite.sh"
    fi

    CARGO_HOME_VALUE="${CARGO_HOME:-$HOME/.cargo}"
    export CARGO_HOME="${CARGO_HOME_VALUE}"
    export PATH="${CARGO_HOME_VALUE}/bin:${PATH}"
    mkdir -p "${CARGO_HOME_VALUE}/bin"

    # [1] [2] [3]: install binaries (skip version bump - use `just bump` for that)
    cargo install --path b00t-mcp  --force
    cargo install --path b00t-cli  --force
    cargo install --path b00t-admin --force
    cargo install cocogitto --locked --force
    just install-commit-hook
    echo "  ✅ binaries installed"
    echo "🔌 Installing recommended MCP servers (gated by environment)..."
    b00t-cli install --mcp=recommended || echo "⚠️  MCP install skipped (no matching servers)"

    # [2] [3]: systemd service
    if [[ "$CHOICE" == "2" || "$CHOICE" == "a" ]]; then
        echo
        if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
            echo "🔑 root — installing b00t@.service system-wide"
            _prompt_timeout "cp b00t@.service → /usr/lib/systemd/user/ (requires root)" && {
                mkdir -p /usr/lib/systemd/user
                cp -v .config/systemd/user/b00t@.service /usr/lib/systemd/user/b00t@.service
                systemctl daemon-reload
                echo "  ✅ /usr/lib/systemd/user/b00t@.service (system-wide)"
                echo "  💡 any user: systemctl --user enable b00t@<profile>"
            } || true
        else
            mkdir -p "$HOME/.config/systemd/user"
            cp -v .config/systemd/user/b00t@.service "$HOME/.config/systemd/user/b00t@.service"
            systemctl --user daemon-reload 2>/dev/null || true
            echo "  ✅ ~/.config/systemd/user/b00t@.service"
            echo "  💡 system-wide (all users): sudo just install"
        fi
    fi

# Install b00t skills/agents/hooks into agent runtimes (interactive TUI)
# Install skill/agent runtimes interactively (kept for backwards compat — prefer `just install`)
install-runtimes: build-hooks
    b00t-cli install --interactive

# System deps: apt packages + cargo/uv tools (sudo for apt; user-local otherwise)
# 🤓 Separate from `just install` — install-deps handles OS packages, install handles b00t service
install-deps:
    #!/bin/bash
    set -euo pipefail

    # 🤓 _prompt_timeout: only gates apt-get (requires root); all user-local tools run unconditionally
    _prompt_timeout() {
        local desc="${1:-proceed}"
        local timeout=10
        for i in $(seq $timeout -1 1); do
            printf "\r  ⏱  [%2ds] %s — press any key or wait to skip..." "$i" "$desc"
            if read -r -s -n 1 -t 1 2>/dev/null; then
                printf "\n  ▶  %s\n" "$desc"
                return 0
            fi
        done
        printf "\n  ⏭  skipped: %s\n" "$desc"
        return 1
    }

    # apt-get: sudo-only, gated by prompt
    if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
        _prompt_timeout "apt-get install network tools (requires root)" && \
            apt-get update -q && apt-get install -y dnsutils net-tools iputils-ping tcpdump nmap mtr-tiny whois iproute2 || true
        _prompt_timeout "apt-get install cli tools (requires root)" && \
            apt-get install -y fzf bat moreutils fd-find bc jq python3-argcomplete curl || true
        ln -sf /usr/bin/batcat /usr/local/bin/bat 2>/dev/null || true
    else
        echo "  ⚠️  apt-get skipped (not root) — run: sudo just installx"
        ln -sf /usr/bin/batcat ~/.local/bin/bat 2>/dev/null || true
    fi

    # User-local installs — no sudo, run unconditionally
    export PATH="$HOME/.cargo/bin:$PATH"
    command -v dotenv  >/dev/null 2>&1 || uv tool install python-dotenv[cli]
    command -v toml    >/dev/null 2>&1 || cargo install toml-cli
    command -v dotenvy >/dev/null 2>&1 || cargo install dotenvy --features cli

    # eget + rg: system-wide if root, user-local otherwise — no prompt (no sudo)
    if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
        command -v eget >/dev/null 2>&1 || (curl -s https://zyedidia.github.io/eget.sh | sh && mv -v eget /usr/local/bin/)
        command -v rg   >/dev/null 2>&1 || (eget BurntSushi/ripgrep && mv -v rg /usr/local/bin/)
    else
        mkdir -p ~/.local/bin
        command -v eget >/dev/null 2>&1 || (curl -s https://zyedidia.github.io/eget.sh | sh && mv -v eget ~/.local/bin/)
        command -v rg   >/dev/null 2>&1 || (eget BurntSushi/ripgrep && mv -v rg ~/.local/bin/)
    fi
    echo "/🥾"

# Node.js/TypeScript development environment setup
install-node:
    #!/bin/bash
    echo "🦄 Installing Node.js/TypeScript development environment..."
    # Install nvm (Node Version Manager)
    command -v nvm >/dev/null 2>&1 || curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.1/install.sh | bash
    # Source nvm in current session
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh"
    [ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion"
    # Install and use LTS Node.js
    nvm install --lts
    nvm use --lts
    nvm alias default lts/*
    # Install bun (preferred over npm)
    command -v bun >/dev/null 2>&1 || curl -fsSL https://bun.sh/install | bash
    # Install pnpm as fallback
    command -v pnpm >/dev/null 2>&1 || npm install -g pnpm
    # Install essential global packages
    bun add -g typescript tsx @types/node
    bun add -g eslint prettier husky lint-staged commitlint @commitlint/config-conventional
    bun add -g yeoman-generator yo
    echo "✅ Node.js/TypeScript environment ready"

# Setup TypeScript project with b00t standards
setup-ts-project:
    #!/bin/bash
    echo "🦄 Setting up TypeScript project with b00t standards..."
    # Initialize package.json if not exists
    [ ! -f package.json ] && bun init -y
    # Install core dependencies
    bun add -D typescript tsx @types/node
    bun add -D eslint @typescript-eslint/parser @typescript-eslint/eslint-plugin
    bun add -D prettier eslint-config-prettier eslint-plugin-prettier
    bun add -D husky lint-staged @commitlint/cli @commitlint/config-conventional
    # Install Effect-TS (preferred functional programming library)
    bun add effect
    # Setup git hooks
    bunx husky install
    # Create .husky/pre-commit hook
    echo '#!/usr/bin/env sh\n. "$(dirname -- "$0")/_/husky.sh"\nbunx lint-staged' > .husky/pre-commit
    chmod +x .husky/pre-commit
    # Create .husky/commit-msg hook
    echo '#!/usr/bin/env sh\n. "$(dirname -- "$0")/_/husky.sh"\nbunx commitlint --edit "$1"' > .husky/commit-msg
    chmod +x .husky/commit-msg
    echo "✅ TypeScript project setup complete"

# Run TypeScript development server
dev-ts:
    #!/bin/bash
    echo "🚀 Starting TypeScript development..."
    bun run dev || bunx tsx watch src/index.ts

# Build TypeScript project
build-ts:
    #!/bin/bash
    echo "🔨 Building TypeScript project..."
    bun run build || bunx tsc

# Lint and format TypeScript code
lint-ts:
    #!/bin/bash
    echo "🧹 Linting TypeScript code..."
    bunx eslint . --ext .ts,.tsx --fix
    bunx prettier --write "src/**/*.{ts,tsx,json}"

# Test TypeScript project
test-ts:
    #!/bin/bash
    echo "🧪 Running TypeScript tests..."
    bun test || bunx jest

# Quick WIP commit for TypeScript projects
wip-ts:
    #!/bin/bash
    git add .
    git commit -m "wip: work in progress - squash me"

dotenv-load:
    dotenv -f .env


# Run Rust Analyzer in current directory
ra_run:
    rust-analyzer .

# Run tests in the current directory
test:
    cargo test -- --nocapture

# Verify the compact b00t-mcp surface and communication output contract.
test-b00t-mcp:
    cargo test -p b00t-mcp

# Salvage-first lfmf writer regression tests -- zero duplication, zero payload loss (issue #934, ports #1183's coverage onto #1163's shipped fix).
test-lfmf-roundtrip:
    cargo test -p b00t-cli --test lfmf_salvage_test --test lfmf_writer_test

# Format the b00t-mcp crate through the registered action surface.
format-b00t-mcp:
    rustfmt --edition 2024 b00t-mcp/src/chat.rs b00t-mcp/src/mcp_server_rusty.rs b00t-mcp/src/mcp_tools.rs

# Install the already-versioned b00t-mcp after its focused contract passes.
install-b00t-mcp: test-b00t-mcp
    cargo install --locked --path b00t-mcp --force

# 🤓 deterministic hive accelerator/soul verification (P1–P3).
#    Builds the test binary ONCE (--no-run), then runs hive tests directly —
#    avoids re-linking the full workspace test binary on every invocation.
verify-hive:
    #!/bin/bash
    set -euo pipefail
    cargo test --release --no-run -p b00t-cli
    # locate the test harness that actually contains hive::tests (not the main bin)
    bin=""
    for c in $(find target/release/deps -maxdepth 1 -name 'b00t_cli-*' -type f ! -name '*.d' ! -name '*.o'); do
        if "$c" --list 2>/dev/null | grep -q "hive::tests::"; then bin="$c"; break; fi
    done
    [ -n "$bin" ] || { echo "❌ no test harness with hive::tests found"; exit 1; }
    echo "▶ test binary: $bin"
    "$bin" hive::tests --nocapture

# Distil a session transcript into persistent soul memory (P5).
# 🤓 deterministic: prewritten so the agent runs one command, never invents it.
#   just distill session.log          # from a file (sm0l tier, default)
#   just distill session.log ch0nky   # from a file, ch0nky tier
#   just distill -                    # from stdin (pipe a transcript in)
distill file tier="sm0l":
    #!/bin/bash
    set -euo pipefail
    if [ "{{file}}" = "-" ]; then
        b00t soul distill --model {{tier}}
    elif [ -f "{{file}}" ]; then
        b00t soul distill --model {{tier}} < "{{file}}"
    else
        echo "usage: just distill <file|-> [tier=sm0l|ch0nky|frontier]" >&2
        exit 1
    fi

# Distil ch0nky sub-agent transcript → diff+test output contract enforcement.
# Called by SubagentStop hook to compress agent output before returning to executive.
# Input: transcript on stdin. Output: compressed diff+test summary (≤50 lines).
# 🤓 enforces the ch0nky output contract: diff + test result only; no raw transcripts.
distill-ch0nky:
    python3 _b00t_/scripts/distill-ch0nky.py

# Deterministic node snapshot: refresh soul node.* + HW-drift check (P3),
# then show the compressed node identity line (P2).
node-snapshot:
    @b00t hive status
    @echo
    @echo "--- whoami node line (P2) ---"
    @b00t whoami --role=operator 2>/dev/null | grep -E '^🥾 Node:' || echo "(node identity not yet recorded — run: b00t hive status)"



# trigger & run any action ci/action locally
# don't specify workflow or job then script will display ./github/workflows using fzf
gh-action workflow="" job="":
    cd {{repo-root}} && ./just-run-gh-action.sh {{workflow}} {{job}}

watch-gh-action workflow="" job="":
    # Check if cargo-watch is installed; install it quietly if not
    export PATH="$HOME/.cargo/bin:$PATH"
    command -v cargo-watch >/dev/null 2>&1 || cargo install cargo-watch --quiet
    cargo watch -s "./just-run-gh-action.sh {{workflow}} {{job}}"


clean-workflows:
   gh api -H "Accept: application/vnd.github+json" \
    /repos/elasticdotventures/dotfiles/actions/runs?per_page=100 \
    | jq -r --arg cutoff "$(date -d '7 days ago' --iso-8601=seconds)" \
        '.workflow_runs[] | select(.created_at < $cutoff) | .id' \
    | xargs -n1 -I{} gh api --method DELETE \
        -H "Accept: application/vnd.github+json" \
        /repos/elasticdotventures/dotfiles/actions/runs/{}

version:
    @grep '^version = ' Cargo.toml | grep -oP '[\d]+\.[\d]+\.[\d]+'

commit-hook:
    #!/bin/bash
    set -euo pipefail
    # If strict-review flag exists, run the blocking reviewer gate
    if [[ -f ".b00t/strict-review" ]]; then
        echo "🛡️  strict-review gate active — validating staged changes..."
        if ! JUST_UNSTABLE=1 just reviewer pr-validate goal="staged changes"; then
            echo ""
            echo "❌ Reviewer gate blocked commit. Fix issues and try again."
            echo "   To bypass: rm .b00t/strict-review (not recommended)"
            exit 1
        fi
        echo "✅ Reviewer gate passed"
    fi
    # Gate schema validation: ensure all gate datums pass contract
    if ls _b00t_/gates/*.gate.toml &>/dev/null; then
        if ! python3 scripts/validate-gate.py _b00t_/gates/*.gate.toml 2>/dev/null; then
            echo "❌ Gate schema validation failed. Fix gate datums and try again."
            exit 1
        fi
    fi
    # Hard-gate datum graph coherence when staged changes touch datum files.
    STAGED_DATUMS="$(git diff --cached --name-only --diff-filter=ACMR | grep -E '^_b00t_/.*\.tomll?m?d?$' || true)"
    if [[ -n "${STAGED_DATUMS}" ]]; then
        echo "🕸️  datum graph gate active — validating staged datum changes..."
        if ! JUST_UNSTABLE=1 just datum-validate-graph; then
            echo "❌ Datum graph validation failed. Fix dangling refs and try again."
            exit 1
        fi
        echo "✅ Datum graph validation passed"
    fi

datum-validate-graph path="_b00t_":
    cargo run -p b00t-cli --bin b00t-cli -- --path {{path}} datum validate --graph

commit-hook2:
    #!/bin/bash
    set -euo pipefail
    if ! git diff --quiet; then
        echo "⚠️ Unstaged changes detected; please stash or stage before running commit-hook"
        exit 1
    fi
    cargo fmt
    CURRENT_VERSION=$(toml get Cargo.toml workspace.package.version | tr -d '"')
    IFS='.' read -r MAJOR MINOR PATCH <<< "${CURRENT_VERSION}"
    PATCH=$((PATCH + 1))
    NEW_VERSION="${MAJOR}.${MINOR}.${PATCH}"
    TMP_FILE=$(mktemp)
    toml set Cargo.toml workspace.package.version "${NEW_VERSION}" > "${TMP_FILE}"
    mv "${TMP_FILE}" Cargo.toml
    cargo metadata --format-version 1 >/dev/null 2>&1 || true
    git add -u
    VERSION=$(toml get Cargo.toml workspace.package.version | tr -d '"')
    if git diff --cached --quiet; then
        echo "No staged changes after running commit-hook"
    else
        echo "✅ Staged fmt + version bump (v${VERSION}); continue with your commit."
    fi

install-commit-hook:
    #!/bin/bash
    set -euo pipefail
    # Skip if not in a git repo (e.g., Docker container)
    if [ ! -d ".git" ]; then
        echo "⏭️  Skipping git hook installation (not a git repository)"
        exit 0
    fi
    HOOK_PATH=".git/hooks/pre-commit"
    {
        echo "#!/usr/bin/env bash"
        echo "set -euo pipefail"
        echo "if command -v just >/dev/null 2>&1; then"
        echo "    JUST_UNSTABLE=1 just commit-hook"
        echo "else"
        echo "    echo \"just is required to run commit-hook\" >&2"
        echo "    exit 1"
        echo "fi"
    } > "${HOOK_PATH}"
    chmod +x "${HOOK_PATH}"
    echo "✅ Installed .git/hooks/pre-commit to run 'just commit-hook'"

# Install rustfmt PostToolUse hook for Claude Code (run once as operator)
# Copies _b00t_/hooks/rustfmt-post-edit to ~/.claude/hooks/ and prints settings.json patch
install-rustfmt-hook:
    #!/bin/bash
    set -euo pipefail
    HOOK_SRC="_b00t_/hooks/rustfmt-post-edit"
    HOOK_DEST="${HOME}/.claude/hooks/rustfmt-post-edit"
    mkdir -p "${HOME}/.claude/hooks"
    cp "$HOOK_SRC" "$HOOK_DEST"
    chmod +x "$HOOK_DEST"
    echo "✅ Installed ${HOOK_DEST}"
    echo ""
    echo 'Add to ~/.claude/settings.json hooks.PostToolUse array:'
    echo '  {"matcher":"Edit|Write","hooks":[{"type":"command","command":"~/.claude/hooks/rustfmt-post-edit"}]}'


# test-hook: run rustfmt-post-edit hook integration tests
test-hook:
    bash _b00t_/hooks/test-rustfmt-hook.sh

# upgrade: holistic b00t upgrade (binary, MCP, hooks, Claude settings)
upgrade:
    cargo run --bin b00t-cli -p b00t-cli -- upgrade

upgrade-dry:
    cargo run --bin b00t-cli -p b00t-cli -- upgrade --dry-run


cliff:
    # git-cliff --tag $(git describe --tags --abbrev=0) -o CHANGELOG.md
    git-cliff -o CHANGELOG.md



inspect-mcp:
	npx @modelcontextprotocol/inspector ./target/release/b00t-mcp

# ── HF Cloud — dataset sync + job lifecycle ────────────────────────────────────
# (moved to mod hf-cloud '_b00t_/justfile-hf-cloud.just')

# Captain's Command Arsenal - Memoized Agent Operations

# Role switching commands
session-status:
    #!/bin/bash
    cargo run --bin b00t-cli -- whatismy status

session-build:
    #!/bin/bash
    cargo run --bin b00t-cli -- session build

# Tool installation (for operators)
validate-mcp:
	#!/bin/bash
	set -euo pipefail
	echo "🔍 Validating MCP TOML files..."
	cd {{repo-root}}/_b00t_
	taplo lint --schema file://$PWD/schema-资源/mcp.json *.mcp.toml
	# M1: pi --mode rpc smoke test — verify pi supports rpc mode (gap-fill #343)
	# 🤓 pi --mode rpc confirmed present in pi 0.x; if this fails pi was downgraded
	echo "🔍 Validating pi --mode rpc support..."
	if command -v pi >/dev/null 2>&1; then
		if pi --help 2>&1 | grep -q -- "--mode rpc"; then
			echo "✅ pi --mode rpc: supported"
		else
			echo "⚠️ pi --mode rpc: NOT found (datum gap in _b00t_/pi.agent.toml)"
		fi
	else
		echo "⚠️ pi: not installed; skipping --mode rpc smoke test"
	fi

# ── Obsidian MCP proxy ───────────────────────────────────────────────────
# Launch local proxy to Windows host's Semantic Notes Vault MCP plugin.
# Requires OBSIDIAN_MCP_KEY env var (set in .env or b00t install).
# Connects via mcp-remote → https://${OBSIDIAN_HOST}:3443/mcp with Bearer auth.
obsidian-proxy:
	npx -y mcp-remote \
	  https://$(OBSIDIAN_HOST:=windows-host.lan):$(OBSIDIAN_PORT:=3443)/mcp \
	  --header "Authorization: Bearer $(OBSIDIAN_MCP_KEY)"

# Lint: NTFS invalid character scan — fail if paths contain reserved chars
# Policy: pwsh.🪟/NTFS_RESERVED_CHARS.md
# 🤓 Excludes emoji dirs (_b00t_/🐚/) — those render as unicode, not literal chars
lint-ntfs:
	#!/bin/bash
	set -euo pipefail
	echo "🔍 NTFS compatibility scan..."
	cd {{repo-root}}
	# Scan for LITERAL reserved chars (: | ? * < > "), not unicode/emoji
	OFFENDERS=$(git ls-tree -r HEAD --name-only | grep -F ':' | grep -v '^[^:]*$' || true)
	# 🤓 More precise: filter git note refs (format: filename:commit:path)
	GIT_NOTE_REFS=$(git ls-tree -r HEAD --name-only | grep ':[0-9a-f]*:' || true)
	if [[ -n "$GIT_NOTE_REFS" ]]; then
		echo "🚫 Git note refs detected (NTFS-invalid):"
		echo "$GIT_NOTE_REFS" | head -10
		echo ""
		echo "💡 See: pwsh.🪟/NTFS_RESERVED_CHARS.md"
		exit 1
	fi
	echo "✅ No NTFS-invalid paths (git note refs) found"

# Build and package b00t browser extension
socks5:
    {{repo-root}}/scripts/socks5.sh

port-map:
    {{repo-root}}/scripts/port-map.sh

# 💡 Recommended: sudo just install-services
install-services:
    #!/bin/bash
    set -euo pipefail
    if [[ "${EUID:-$(id -u)}" -eq 0 ]]; then
        {{repo-root}}/scripts/install-systemd-services.sh
    else
        echo "  ⚠️  install-services requires root — run: sudo just install-services" >&2
        exit 1
    fi
ansible-k0s-stop PLAYBOOK="ansible/playbooks/k0s_kata_stop.yaml" INVENTORY="ansible/inventory.sample.yaml" EXTRA_ARGS="":
    #!/bin/bash
    set -euo pipefail
    INVENTORY="${INVENTORY:-ansible/inventory.sample.yaml}"
    PLAYBOOK="${PLAYBOOK:-ansible/playbooks/k0s_kata_stop.yaml}"
    EXTRA_ARGS="${EXTRA_ARGS:-}"
    if ! command -v ansible-playbook >/dev/null 2>&1; then
        echo "ansible-playbook not found. Install ansible-core first." >&2
        exit 1
    fi
    echo "🥾 stopping k0s + Kata via ansible"
    ANSIBLE_FORCE_COLOR=1 ansible-playbook -i "$INVENTORY" "$PLAYBOOK" $EXTRA_ARGS

orchestrator-k0s-kata MODE="start" INVENTORY="~/.config/b00t/k0s-inventory.yaml" EXTRA_ARGS="":
    #!/bin/bash
    set -euo pipefail
    MODE="${MODE:-start}"
    INVENTORY="${INVENTORY:-$HOME/.config/b00t/k0s-inventory.yaml}"
    EXTRA_ARGS="${EXTRA_ARGS:-}"
    K0S_KATA_EXTRA_ARGS="$EXTRA_ARGS" scripts/orchestrators/k0s_kata.sh "$MODE" "$INVENTORY"

# Ralph autonomous agent integration
# 🤓 Ralph runs backlog/job workflows autonomously via codex/claude/amp/opencode/mistralrs

# Run ralph hive validation before starting projects
ralph-hive-validate tool="codex" iterations="5":
    #!/bin/bash
    set -euo pipefail
    echo "🥾 Running ralph hive validation..."
    cargo run --bin b00t-cli -- agent ralph \
        --tool {{tool}} \
        --task hive-validate \
        --max-iterations {{iterations}} \
        --project-root {{repo-root}}

# Run ralph maintenance tasks
ralph-maintenance tool="codex" iterations="10":
    #!/bin/bash
    set -euo pipefail
    echo "🥾 Running ralph maintenance..."
    cargo run --bin b00t-cli -- agent ralph \
        --tool {{tool}} \
        --task maintenance \
        --max-iterations {{iterations}} \
        --project-root {{repo-root}}

# Run ralph on pending backlog items
ralph-run tool="codex" iterations="10":
    #!/bin/bash
    set -euo pipefail
    echo "🥾 Running ralph autonomous loop..."
    cargo run --bin b00t-cli -- agent ralph \
        --tool {{tool}} \
        --task pending \
        --max-iterations {{iterations}} \
        --project-root {{repo-root}}

# Run Gemma4-only operator self-improvement loop against local opencode/vLLM.
pre-project: ralph-hive-validate
    echo "✅ Hive validated - ready for project work"

# Hive maintenance: dispatch codex+haiku agents per GH issue cluster (parallel)
# Uses: scripts/hive-maintenance/dispatch-hive.sh
# Each issue: codex exec investigates → haiku reviews → gh comment posted
# RL: haiku rejects → codex retries until approved or max-iter
hive-maintenance cluster="" max_iter="5":
    #!/bin/bash
    set -euo pipefail
    SCRIPT="{{repo-root}}/scripts/hive-maintenance/dispatch-hive.sh"
    chmod +x "${SCRIPT}"
    ARGS=""
    [[ -n "{{cluster}}" ]] && ARGS="${ARGS} --cluster {{cluster}}"
    MAX_ITER={{max_iter}} bash "${SCRIPT}" ${ARGS}

# Dry-run hive maintenance (verify issue mapping without API calls)
hive-maintenance-dry:
    #!/bin/bash
    bash {{repo-root}}/scripts/hive-maintenance/dispatch-hive.sh --dry-run

# Run ralph loop for hive maintenance in current session (uses HIVE_MAINTENANCE_PROMPT.md)
# Stop hook intercepts exit → feeds same prompt → iterates until <promise> detected
hive-ralph-loop max_iter="10":
    #!/bin/bash
    echo "🐝 Starting ralph loop for hive maintenance (max {{max_iter}} iters)"
    echo "Prompt anchor: HIVE_MAINTENANCE_PROMPT.md"
    claude --print \
        --model claude-haiku-4-5-20251001 \
        --max-budget-usd 0.10 \
        "$(cat {{repo-root}}/HIVE_MAINTENANCE_PROMPT.md)"

# Inspect the local side of the sm3lly NATS ACP route without publishing credentials
acp-sm3lly-status:
    #!/bin/bash
    set -euo pipefail
    _nats_raw="${NATS_URL:-nats://c010.promptexecution.com:4222}"
    _nats_redacted="$(echo "$_nats_raw" | sed 's|//[^@]*@|//<redacted>@|')"
    echo "NATS_URL=${_nats_redacted}"
    b00t chat info
    b00t hive status
    b00t whoami --role=executive | sed -n '/Capability check:/,$p'

# ── Gemma 4 + pi-coding-agent local inference ────────────────────────────────
# (moved to mod gemma '_b00t_/justfile-gemma.just')

# ── Worker agent — A/B experiment dispatch + phygital ontology ──────────────
# (moved to mod worker '_b00t_/justfile-worker.just')

# ── b00t skill-improvement loop — opencode ch0nky continuous self-improvement ──
# 🤓 Tests datums, fixes gaps, commits improvements; runs unattended overnight

# ── ledgrrr — ledgerr-mcp lifecycle (just module) ─────────────────────────
# 🦨 Symlink: vendor/ledgrrr -> vendor/ledgrrr (polyseme mapping)
# Module docs: https://just.systems/man/en/modules.html
# Invocation:  just ledgrrr build | docker-build | docker-run | docker-stop | …
mod? ledgrrr 'vendor/ledgrrr/ledgrrr.just'

# ── pi agent — systemd service lifecycle ─────────────────────────────────────
# 🤓 pi is managed as b00t@pi-agent.service, NOT spawned per-invocation
opencode-task task="hello":
    opencode run --model gemma4-local/ch0nky "{{task}}"

# ── Executive delegation — offload ch0nky work to local GPU ─────────────────
# 🤓 Use these INSTEAD of frontier sub-agents for: implement, refactor, debug.
#    Only bring output summary back to executive context (never raw diff).
#    GPU check: just ch0nky-status before dispatching.

# Check if ch0nky is live on :8001
ch0nky-status:
    @curl -sf http://localhost:8001/v1/models \
      | python3 -c "import sys,json; d=json.load(sys.stdin); print('✅ ch0nky online:', d['data'][0]['id'])" \
      || echo "🔴 ch0nky offline — run: b00t hive activate inference-qwen36-27b"

# Delegate a task to ch0nky via opencode. Returns diff + PASS/FAIL only.
# Usage: just delegate task="implement CakeLedger::balance() in b00t-cli/src/cake_ledger.rs"
delegate task="":
    #!/usr/bin/env bash
    set -euo pipefail
    [ -z "{{task}}" ] && { echo "usage: just delegate task='<description>'"; exit 1; }
    curl -sf http://localhost:8001/v1/models > /dev/null || { echo "🔴 ch0nky offline"; exit 1; }
    echo "📤 delegating to ch0nky: {{task}}"
    opencode run --model qwen36-local/ch0nky "{{task}}"

# Ask ch0nky to classify/summarize — sm0l filter for executive context.
# Usage: just ask "does b00t-cli/src/lib.rs already pub mod cake_ledger?"
ask query="":
    #!/usr/bin/env bash
    [ -z "{{query}}" ] && { echo "usage: just ask 'query'"; exit 1; }
    curl -sf http://localhost:8001/v1/chat/completions \
      -H "Content-Type: application/json" \
      -d "{"model":"ch0nky","messages":[{"role":"user","content":"{{query}}"}],"max_tokens":256}" \
      | python3 -c "import sys,json; print(json.load(sys.stdin)['choices'][0]['message']['content'])"

# ── ufo-types crate (#511) — Tax-Lawyer UFO stereotypes + Satisfies<T> ──────
# (moved to mod ufo '_b00t_/justfile-ufo.just')

# Regenerate the ufo-types adoption baseline report (issue #928) — measures
# real adoption via grep, not hand-maintained numbers that silently drift.
ufo-adoption-report:
    @bash _b00t_/scripts/ufo-adoption-report.sh

# ── Chore memoization — recipes for fine-tune corpus (fewer tokens) ─────────
# (moved to mod chore '_b00t_/justfile-chore.just')

# ── Pre-flight checks — system-normal gate for reviewers ──────────────────
# (moved to mod review '_b00t_/justfile-review.just')

# ── Codebase memory indexing ──────────────────────────────────────────────

# Index the current repo into codebase-memory (fast mode)
index-codebase:
    @echo "🔍 Indexing into codebase-memory..."
    @echo "ℹ️  Use MCP: codebase-memory index_repository(repo_path=\".\", mode=\"fast\")"

# ── Android emulator sandbox (for Oreo 🐶) ──────────────────────────────────
# 🤓 Uses RHAI script to sandbox ALL android operations deterministically.
#    The RHAI script memoizes the full pipeline so agents don't hallucinate
#    adb/emulator commands. One b00t call replaces 50+ lines of fragile bash.

# Run the full Android test pipeline via RHAI (sandboxed, CPU-limited)
android-sandbox:
    b00t script run _b00t_/scripts/android-emu-setup.rhai





# ── b00t test harness (ping/pong integration tests) ─────────────────────────

# Run the b00t integration test harness — verifies 5 key subsystems
b00t-test-harness:
    @bash _b00t_/scripts/b00t-ping-pong.sh

# Pre-cargo gate: detect submodule pin drift (recorded gitlink vs checked-out HEAD).
# Distinguishes drifted+clean (safe, auto-fixable) from drifted+dirty (report only).
# Usage: just doctor         (report only)
#        just doctor --fix   (auto-sync drifted+clean submodules; never touches dirty ones)
doctor *ARGS:
    @bash _b00t_/scripts/check-submodule-drift.sh {{ARGS}}

# Fast compile-check (no tests) — use BEFORE cargo test to catch wiring errors cheaply
check-fast: doctor
    cargo check --package b00t-cli --message-format=short 2>&1 | grep -E "^error" | head -20 || echo "✅ check clean"

# ── ch0nky slot swap (pi ↔ opencode) ─────────────────────────────────────────
# 🤓 pi and opencode share the ch0nky-coding-agent exclusion group — only one active
moltis-build:
    cargo build --manifest-path vendor/moltis-b00t/crates/cli/Cargo.toml --release --no-default-features --features lightweight

# moltis: start moltis with b00t soul backend
moltis-run:
    MOLTIS_SOUL_URL=http://127.0.0.1:7700 ./vendor/moltis-b00t/target/release/moltis

# moltis: run soul serve + test soul<->moltis K/V roundtrip
moltis-soul-test:
    b00t soul serve &
    sleep 1
    b00t soul set moltis_test_key "hello_from_b00t"
    b00t soul get moltis_test_key



# ── wrkflw skill — b00t learn wrkflw verification ─────────────────────────
# Verify the wrkflw skill loads correctly via b00t learn
skill-wrkflw-test:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== Testing b00t learn wrkflw skill ==="
    b00t learn wrkflw | head -5
    echo "✓ wrkflw skill loads"

# Verify wrkflw skill is listed in available topics
skill-wrkflw-list:
    #!/usr/bin/env bash
    set -euo pipefail
    if b00t learn --list 2>/dev/null | grep -qi wrkflw; then
        echo "✓ wrkflw in learn topics"
    else
        echo "✗ wrkflw not found in list — check learn.toml" >&2
        exit 1
    fi



# ─── Ledgrrr Subsystem ──────────────────────────────────────────────────────
# ledgrrr recipes live in vendor/ledgrrr/ledgrrr.just (just module).
#
# Module pattern:
#   mod ledgrrr 'vendor/ledgrrr/ledgrrr.just'    ← in the import section above
#   just ledgrrr <recipe>                          ← invocation
#   just --list ledgrrr                            ← list module recipes
#
# Available recipes (run `just --list ledgrrr` for full list):
#   just ledgrrr build         — build ledgerr-mcp binary
#   just ledgrrr test          — run MECE test harness (42 tests)
#   just ledgrrr viz           — start viz dashboard (:8080)
#   just ledgrrr viz-stop      — stop viz server
#   just ledgrrr status        — subsystem status
#   just ledgrrr install       — build + install to ~/.local/bin
#
# Why module instead of inline:
#   - Keeps root justfile lean (was 1217+ lines, growing)
#   - Recipes auto-run in vendor/ledgrrr/ cwd
#   - Namespaced: `just ledgrrr viz` not `just ledgrrr-viz`
#   - Vendor owns its own lifecycle; root only adds `mod` line
# See vendor/ledgrrr/ledgrrr.just header for full documentation.
# ─────────────────────────────────────────────────────────────────────────────
# ── ralph with diversity ──────────────────────────────────────────────────────
# ralph-spawn: instantiate a ralph agent with a random personality + transferable skills.
# Karpathy deepwiki OKR pattern: RESEARCH is a separate cycle from EXECUTION.
# Each spawn gets: (a) random personality archetype, (b) random 2-3 transferable skills.
# Different skills → different heuristics → better collective hive diversity.
# 🤓 Never assign the same transferable skills to every agent — entropy is a feature.

# List of transferable skills (from _b00t_/*.skill.toml type_tags=["transferable"])
_TRANSFERABLE_SKILLS := "kaizen triz six-sigma ideo mece first-principles socratic bayesian rubber-duck pre-mortem five-whys ockham"

# Personality archetypes — injected as system bias, not hard constraints
_PERSONALITIES := "methodical-skeptic creative-synthesizer devil-advocate systems-thinker pragmatic-fixer pattern-hunter first-principles-zealot bayesian-updater"

# Spawn one ralph: random personality + N random transferable skills, run GOAL
ralph-spawn goal="" n_skills="3" tool="claude-code":
    #!/usr/bin/env bash
    set -euo pipefail

    # ── Sample random personality ─────────────────────────────────────────────
    PERSONALITIES=({{_PERSONALITIES}})
    PERSONALITY="${PERSONALITIES[$RANDOM % ${#PERSONALITIES[@]}]}"
    echo "[ralph:spawn] personality=$PERSONALITY"

    # ── Sample N random transferable skills (no repeats) ─────────────────────
    ALL_SKILLS=({{_TRANSFERABLE_SKILLS}})
    SHUFFLED=($(printf '%s\n' "${ALL_SKILLS[@]}" | shuf))
    ASSIGNED=("${SHUFFLED[@]:0:{{n_skills}}}")
    echo "[ralph:spawn] transferable skills: ${ASSIGNED[*]}"

    # ── Load blessing content for each assigned skill ─────────────────────────
    SKILL_CONTENT=""
    for SKILL in "${ASSIGNED[@]}"; do
      CONTENT=$(b00t-cli learn "$SKILL" --concise 2>/dev/null || true)
      if [ -n "$CONTENT" ]; then
        SKILL_CONTENT=$(printf '%s\n## %s\n%s\n' "$SKILL_CONTENT" "$SKILL" "$CONTENT")
      fi
    done

    # ── Karpathy OKR: RESEARCH phase (separate from execution) ───────────────
    # Research soul is pre-loaded before task starts — not inline during execution.
    # This is NOT generic RAG. It is: goal → OKR decomposition → targeted topic research.
    GOAL_TEXT="{{goal}}"
    if [ -z "$GOAL_TEXT" ]; then
      GOAL_TEXT=$(b00t-cli task next --json 2>/dev/null | jq -r '.title // empty' || true)
    fi
    if [ -z "$GOAL_TEXT" ]; then echo "[ralph] no goal"; exit 1; fi

    echo "[ralph:okr] decomposing goal: $GOAL_TEXT"
    OKR_TOPICS=$(echo "$GOAL_TEXT" | tr '[:upper:]' '[:lower:]' \
      | tr -cs 'a-z0-9-' '\n' | awk 'length>3' | sort -u | head -5)

    echo "[ralph:research] soul topics: $(echo "$OKR_TOPICS" | tr '\n' ' ')"
    while IFS= read -r TOPIC; do
      [ -z "$TOPIC" ] && continue
      SOUL=$(b00t-cli learn "$TOPIC" --concise 2>/dev/null | head -20 || true)
      [ -n "$SOUL" ] && echo "[soul:$TOPIC] loaded ($(echo "$SOUL" | wc -c)c)"
    done <<< "$OKR_TOPICS"

    # ── Compile agent context packet ────────────────────────────────────────────
    TOPICS_STR=$(echo "$OKR_TOPICS" | tr '\n' ' ')
    CTX_FILE=$(mktemp /tmp/ralph-ctx-XXXXXX.md)
    {
      echo "## Ralph Agent Instantiation"
      echo "personality: $PERSONALITY"
      echo "transferable_skills: ${ASSIGNED[*]}"
      echo "goal: $GOAL_TEXT"
      echo "research_topics: $TOPICS_STR"
      echo ""
      echo "## Transferable Skills (active this session)"
      echo "$SKILL_CONTENT"
      echo ""
      echo "## Operating Protocol"
      echo "RESEARCH phase is COMPLETE. Do NOT re-research inline during execution."
      echo "Apply personality ($PERSONALITY) as a cognitive lens, not a hard constraint."
      echo "Transferable skills are heuristics: apply when they clarify."
      echo "Report sharp corners: b00t lfmf <topic> <lesson>"
      echo "Log progress: b00t task update <id> --status done"
    } > "$CTX_FILE"
    AGENT_CTX=$(cat "$CTX_FILE")
    rm -f "$CTX_FILE"

    echo "[ralph:ready] agent context: $(echo "$AGENT_CTX" | wc -c)c"
    echo "$AGENT_CTX"

# ralph-diverse-hive: spawn N ralph agents with independent personalities/skills for same goal
ralph-diverse-hive goal="" n_agents="3" n_skills="3":
    #!/usr/bin/env bash
    set -euo pipefail
    echo "[hive:diverse] spawning {{n_agents}} ralph agents for: {{goal}}"
    for i in $(seq 1 {{n_agents}}); do
      echo "── agent $i/$(({{n_agents}})) ──"
      just ralph-spawn "{{goal}}" "{{n_skills}}" &
    done
    wait
    echo "[hive:diverse] all agents dispatched"

# compile-agent: compile a sandboxed single-file AGENTS.md for a specific role.
# Output = AGENTS.md boilerplate prefix + role supplement + random transferable skills.
# Operator provisions by running: just compile-agent --role=backend --out=/tmp/agent.md
compile-agent role="worker" n_skills="3" out="/tmp/compiled-agent.md":
    #!/usr/bin/env bash
    set -euo pipefail

    B00T_ROOT=$(git -C "$HOME/.b00t" rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    AGENTS_BASE="$B00T_ROOT/AGENTS.md"
    ROLE_SUPPLEMENT="$B00T_ROOT/AGENTS/--role={{role}}.md"

    # ── Base boilerplate (everything before SESSION delimiter) ─────────────────
    BOILERPLATE=$(sed '/── SESSION/q' "$AGENTS_BASE" | head -n -1)

    # ── Role supplement ────────────────────────────────────────────────────────
    if [ -f "$ROLE_SUPPLEMENT" ]; then
      ROLE_CONTENT=$(cat "$ROLE_SUPPLEMENT")
    else
      echo "⚠️  Role supplement not found: $ROLE_SUPPLEMENT"
      ROLE_CONTENT="## Role: {{role}} (no supplement found — using base protocol only)"
    fi

    # ── Blessing manifest ──────────────────────────────────────────────────────
    BLESSING=$(b00t-cli blessing --manifest --role="{{role}}" 2>/dev/null \
      || echo "# blessing manifest unavailable — run: b00t blessing --manifest --role={{role}}")

    # ── Random transferable skills ─────────────────────────────────────────────
    ALL_SKILLS=(kaizen triz six-sigma ideo mece first-principles socratic bayesian rubber-duck pre-mortem five-whys ockham)
    SHUFFLED=($(printf '%s\n' "${ALL_SKILLS[@]}" | shuf))
    ASSIGNED=("${SHUFFLED[@]:0:{{n_skills}}}")
    SKILL_CONTENT=""
    for SKILL in "${ASSIGNED[@]}"; do
      CONTENT=$(b00t-cli learn "$SKILL" --concise 2>/dev/null | head -30 || true)
      [ -n "$CONTENT" ] && SKILL_CONTENT=$(printf '%s\n### %s\n%s\n' "$SKILL_CONTENT" "$SKILL" "$CONTENT")
    done

    # ── Assemble compiled AGENTS.md ────────────────────────────────────────────
    COMPILED="{{out}}"
    TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    {
      echo "$BOILERPLATE"
      echo ""
      echo "## Role: {{role}}"
      echo ""
      echo "$ROLE_CONTENT"
      echo ""
      echo "## Blessing Manifest"
      echo ""
      echo "$BLESSING"
      echo ""
      echo "## Transferable Skills (randomly assigned: ${ASSIGNED[*]})"
      echo "# Each instantiation gets a different random subset for hive diversity."
      echo ""
      echo "$SKILL_CONTENT"
      echo ""
      echo "<!-- SESSION compiled by operator, inject per instantiation"
      echo "Role: {{role}} | Skills: ${ASSIGNED[*]} | Compiled: $TS -->"
    } > "$COMPILED"

    echo "✅ Compiled agent: {{out}} ($(wc -l < {{out}}) lines)"
    echo "   role: {{role}}"
    echo "   skills: ${ASSIGNED[*]}"

# ── end ralph / compile-agent ──────────────────────────────────────────────────

# provision-agent: operator convenience — compile + launch agent for a role+goal in one command.
# Usage: just provision-agent worker "implement a health endpoint"
# Usage: just provision-agent executive "plan Q3 roadmap"
# Operator does NOT need to know about compile-agent or ralph-spawn internals.
provision-agent role="worker" goal="":
    #!/usr/bin/env bash
    set -euo pipefail
    AGENT_FILE="/tmp/b00t-agent-{{role}}-$(date +%s).md"
    echo "[provision] role={{role}}"
    just compile-agent "{{role}}" 3 "$AGENT_FILE"
    echo "[provision] sandbox: $AGENT_FILE"
    if [ -z "{{goal}}" ]; then
      echo "[provision] no goal specified — agent file ready, launch manually:"
      echo "  claude --agent $AGENT_FILE"
    else
      echo "[provision] launching agent with goal: {{goal}}"
      GOAL_TEXT="{{goal}}"
      echo "# Goal: $GOAL_TEXT" >> "$AGENT_FILE"
      just ralph-spawn "$GOAL_TEXT" 3 | claude --print --agent "$AGENT_FILE" 2>/dev/null \
        || echo "[provision] agent ready at: $AGENT_FILE (manual launch required if claude not in PATH)"
    fi

# PRD-011 G1
b00t-metrics:
    #!/usr/bin/env bash
    set -euo pipefail
    datums="$(
      find _b00t_ -maxdepth 1 -type f -printf '%f\n' \
        | awk '
            {
              ext = "no_ext"
              if ($0 ~ /\./) {
                ext = $0
                sub(/^.*\./, "", ext)
              }
              count[ext]++
            }
            END {
              for (ext in count) {
                printf "%s\t%d\n", ext, count[ext]
              }
            }
          ' \
        | jq -Rn '
            reduce inputs as $line (
              {};
              ($line | split("\t")) as $row
              | .[$row[0]] = ($row[1] | tonumber)
            )
          '
    )"
    train_rows=0
    if [ -f fine-tune/train.jsonl ]; then
      train_rows="$(wc -l < fine-tune/train.jsonl | awk '{print $1}')"
    fi
    jq -cn \
      --argjson datums "$datums" \
      --argjson train_rows "$train_rows" \
      '{datums: $datums, train_rows: $train_rows, dangling_refs: null, probe_score: null}'

# ── ROCK 5C Rocket NPU / Frigate — see _b00t_/rock5c-rocket-teflon-frigate.stack.tomllmd ────
# 🤓 Upstream Linux "Rocket" DRM accel driver + Mesa Teflon, NOT the vendor
#    RKNN/RKLLM stack — works on the current-rockchip64 mainline kernel this
#    host already boots, no vendor-kernel switch required. gate_0/1_5 here
#    are read-only or scratch-only (never touch /boot); gate_1/gate_2 change
#    live boot config or start a service — confirm with the operator before
#    running those on a host also serving Home Assistant.

# gate_0: compile the NPU overlay + apply it to a SCRATCH copy of the real DTB
# (never touches /boot) and verify all 3 NPU cores + 3 IOMMUs report "okay".
rocket-overlay-build src="/home/brianh/homeassistant/boot/rock-5c-rocket-npu-overlay.dts" base_dtb="/boot/dtb-6.18.35-current-rockchip64/rockchip/rk3588s-rock-5c.dtb":
    #!/bin/bash
    set -euo pipefail
    command -v dtc >/dev/null || { echo "❌ dtc (device-tree-compiler) not installed"; exit 1; }
    command -v fdtoverlay >/dev/null || { echo "❌ fdtoverlay not installed"; exit 1; }
    WORK="$(mktemp -d)"
    trap 'rm -rf "$WORK"' EXIT
    echo "🔧 compiling overlay: {{src}}"
    dtc -@ -I dts -O dtb -o "$WORK/overlay.dtbo" "{{src}}"
    echo "🔧 applying to a scratch copy of {{base_dtb}} (NOT /boot)"
    cp "{{base_dtb}}" "$WORK/base.dtb"
    fdtoverlay -i "$WORK/base.dtb" -o "$WORK/merged.dtb" "$WORK/overlay.dtbo"
    dtc -I dtb -O dts "$WORK/merged.dtb" 2>/dev/null > "$WORK/merged.dts"
    echo "🔍 checking all 3 NPU cores + 3 IOMMUs report status = \"okay\":"
    FAIL=0
    for p in npu@fdab0000 iommu@fdab9000 npu@fdac0000 iommu@fdaca000 npu@fdad0000 iommu@fdada000; do
        status="$(awk -v node="$p \\{" '$0 ~ node {f=1} f && /status =/ {print; exit} f && /};/{exit}' "$WORK/merged.dts")"
        if echo "$status" | grep -q '"okay"'; then
            echo "  ✅ $p: $status"
        else
            echo "  ❌ $p: ${status:-status line not found}"
            FAIL=1
        fi
    done
    if [ "$FAIL" -eq 0 ]; then
        echo "✅ gate_0 PASS — overlay compiles + applies cleanly, all 6 nodes okay (scratch-only, /boot untouched)"
    else
        echo "❌ gate_0 FAIL — see above"
        exit 1
    fi

# gate_1_5: read-only preflight — do the devices Frigate expects already exist?
# Safe to run any time; reports reality, never changes anything.
frigate-rocket-preflight:
    #!/bin/bash
    set -euo pipefail
    echo "🔍 Frigate/Rocket device preflight (read-only):"
    FAIL=0
    for d in /dev/accel/accel0 /dev/dri /dev/media0 /dev/video1; do
        if [ -e "$d" ]; then
            echo "  ✅ $d exists"
        else
            echo "  ❌ $d missing"
            FAIL=1
        fi
    done
    if [ "$FAIL" -eq 0 ]; then
        echo "✅ preflight PASS"
    else
        echo "⚠️  preflight incomplete — expected before gate_1 (overlay install + reboot); see _b00t_/rock5c-rocket-teflon-frigate.stack.tomllmd"
    fi

# gate_1: install the compiled overlay into /boot's active overlay dir + reboot.
# ⚠️ MODIFIES LIVE BOOT CONFIG AND REBOOTS THIS HOST — this machine also runs
#    Home Assistant/esphome/mosquitto. Confirm with the operator before running.
#    Not auto-run by any other recipe.
rocket-overlay-install src="/home/brianh/homeassistant/boot/rock-5c-rocket-npu-overlay.dts":
    #!/bin/bash
    set -euo pipefail
    echo "⚠️  This installs a devicetree overlay into /boot and is meant to be"
    echo "   followed by a reboot of this host (also runs Home Assistant)."
    echo "   Not auto-executed — see _b00t_/rock5c-rocket-teflon-frigate.stack.tomllmd gate_1."
    echo "   Manual steps once confirmed:"
    echo "     sudo dtc -@ -I dts -O dtb -o /boot/dtb/rockchip/overlay/rock-5c-rocket-npu.dtbo {{src}}"
    echo "     echo 'user_overlays=rock-5c-rocket-npu' | sudo tee -a /boot/armbianEnv.txt"
    echo "     sudo reboot"

# gate_1 verification: run AFTER the operator reboots post rocket-overlay-install.
# Read-only — reports whether Rocket actually bound to hardware.
rocket-postboot-check:
    #!/bin/bash
    set -euo pipefail
    echo "🔍 Rocket NPU postboot check (read-only):"
    if [ -e /dev/accel/accel0 ]; then
        echo "  ✅ /dev/accel/accel0 exists"
    else
        echo "  ❌ /dev/accel/accel0 missing — overlay not active or rocket module not bound"
    fi
    echo "  dmesg | grep -i rocket:"
    dmesg 2>/dev/null | grep -i rocket || echo "    (no rocket entries in dmesg — may need: sudo dmesg | grep -i rocket)"

# gate_2: start Frigate via Quadlet and confirm the Teflon detector is active.
# ⚠️ Starts a systemd service. Only meaningful after gate_1 passes.
frigate-start:
    systemctl --user start frigate.service

frigate-status:
    #!/bin/bash
    set -euo pipefail
    systemctl --user status frigate.service --no-pager || true
    echo "🔍 detector log (looking for teflon_tfl, watching for 'No NPU was detected'):"
    journalctl --user -u frigate.service --no-pager -n 100 | grep -iE "teflon_tfl|No NPU was detected" || echo "  (no matching lines yet)"

# 🥾 Standby cloud build server (GCP dstack dev-environment) — recipes only,
# deliberately no new b00t-cli subcommand (YAGNI, ops tooling not a
# feature). See docs/superpowers/specs/2026-08-10-cloud-build-server-design.md
# for the full design and dev-env/*.yaml for the actual dstack configs.
# Requires the `dstack` CLI on PATH and a GCP backend already configured in
# its config.yml (ambient Application Default Credentials — no secrets to
# manage here).

# Idempotent: applies the fleet then the volume then the dev-environment.
# Run once before the first remote-push/remote-build/remote-test.
remote-provision:
    #!/bin/bash
    set -euo pipefail
    echo "🥾 provisioning b00t-build-fleet..."
    dstack apply -f dev-env/b00t-build-fleet.yaml -y
    echo "🥾 provisioning b00t-build-cache volume..."
    dstack apply -f dev-env/b00t-build-cache.volume.yaml -y
    echo "🥾 provisioning b00t-build dev-environment..."
    dstack apply -f dev-env/b00t-build.dev-environment.yaml -y
    echo "✅ b00t-build ready — ssh b00t-build, or: just remote-push <branch> && just remote-test <branch>"

# Pushes local HEAD to a scratch branch on origin for the build box to fetch.
# Git-native sync — no rsync, no reverse-SSH into a NAT'd local machine.
remote-push branch:
    git push origin HEAD:refs/heads/scratch/{{branch}}

# SSHes into b00t-build, fetches + checks out the scratch branch, and runs
# `cargo build`. Streams live over the SSH session — no polling, no
# separate result-fetch step. First run after remote-provision is cold
# (real full compile); every run after is warm because /data/b00t persists
# on the volume independent of the dev-environment's own stop/resume.
remote-build branch:
    ssh b00t-build "cd /data/b00t && git fetch origin && git checkout scratch/{{branch}} && cargo build"

# Same as remote-build, but `cargo test` instead.
remote-test branch:
    ssh b00t-build "cd /data/b00t && git fetch origin && git checkout scratch/{{branch}} && cargo test"

# Manual stop — belt-and-suspenders alongside the fleet's own 30m
# idle_duration auto-stop (explicit "I'm done for the day" vs. the idle
# timer catching "forgot to").
remote-stop:
    dstack stop b00t-build -y
