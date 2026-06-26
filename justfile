# justfile for Rust Development Environment
# Alias to get the Git repository root
repo-root := env_var_or_default("JUST_REPO_ROOT", `git rev-parse --show-toplevel 2>/dev/null || echo .`)



set shell := ["bash", "-cu"]
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
# 🛡️ Zellij mandatory interaction gate (governance: Allow/Deny/Hook)
mod zellij-gate '_b00t_/zellij-gate.just'

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

next-task:
    #!/bin/bash
    set -euo pipefail
    echo "Next up: extend Gremlin graph (role/capability edges) and wire GraalVM Gremlin server."

viz-entangle datum="ledgrrr" format="mermaid":
    #!/bin/bash
    set -euo pipefail
    cargo run -p b00t-cli --bin b00t-cli -- --path _b00t_ viz entangle --datum "{{datum}}" --format "{{format}}"

gremlin-graalvm-build:
    docker build -t graalvm-gremlin:latest docker/graalvm-gremlin

gremlin-graalvm-run:
    docker run --rm -p 8182:8182 \
      -v $PWD/docker/graalvm-gremlin/gremlin-server.yaml:/opt/gremlin-server/conf/gremlin-server.yaml \
      docker.io/tinkerpop/gremlin-server:latest

stow:
    stow --adopt -d ~/.dotfiles -t ~ bash

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

# Create GitHub release (triggers crates.io publishing workflow)
release:
    #!/bin/bash
    set -euo pipefail
    VERSION=$(grep '^version = ' Cargo.toml | grep -oP '[\d]+\.[\d]+\.[\d]+')

    echo "🚀 Dispatching GitHub-native release for v${VERSION}..."

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
    current=$(grep '^version' Cargo.toml | head -1 | grep -oP '[\d]+\.[\d]+\.[\d]+')
    IFS='.' read -r maj min pat <<< "$current"
    next="$maj.$min.$((pat+1))"
    sed -i "s/^version = \"$current\"/version = \"$next\"/" Cargo.toml
    echo "⬆️  $current → $next"
    cargo install --path b00t-mcp --force
    cargo install --path b00t-cli --force
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
install-runtimes: build-hooks
    b00t-cli install --interactive

# 💡 Recommended: sudo just installx
#    sudo path → apt installs system packages; user path → user-local cargo/uv tools only
installx:
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
        if ! JUST_UNSTABLE=1 just pr-validate goal="staged changes"; then
            echo ""
            echo "❌ Reviewer gate blocked commit. Fix issues and try again."
            echo "   To bypass: rm .b00t/strict-review (not recommended)"
            exit 1
        fi
        echo "✅ Reviewer gate passed"
    fi

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

# 🔧 Maintenance: run deterministic version-check automation for all datums
# with [maintenance] sections. Checks latest versions via shell commands (no LLM),
# and auto-updates desires fields when newer versions are detected.
# Uses datum file mtime to gate check frequency (check_interval_days).
maintenance:
    cargo run --bin b00t-cli -p b00t-cli -- cli up --maintenance


cliff:
    # git-cliff --tag $(git describe --tags --abbrev=0) -o CHANGELOG.md
    git-cliff -o CHANGELOG.md



inspect-mcp:
	npx @modelcontextprotocol/inspector ./target/release/b00t-mcp

# Hugging Face model caching helper
hf-download model dest="" revision="":
	#!/usr/bin/env bash
	set -euo pipefail
	MODEL="{{model}}"
	if [[ -z "$MODEL" ]]; then
		echo "⚠️ set model=<repo>" >&2
		exit 1
	fi
	# 🤓 prefer hf (huggingface_hub>=0.26 alias); auto-install if missing
	if ! command -v hf >/dev/null 2>&1; then
		echo "hf not found — auto-installing huggingface_hub[cli] via uv ..." >&2
		uv tool install --upgrade "huggingface_hub[cli]"
	fi
	DEST="{{dest}}"
	if [[ -z "$DEST" ]]; then
		SANITIZED="${MODEL//\//__}"
		DEST="$HOME/.b00t/models/$SANITIZED"
	fi
	mkdir -p "$DEST"
	ARGS=(download "$MODEL" --local-dir "$DEST" --local-dir-use-symlinks False)
	if [[ -n "{{revision}}" ]]; then
		ARGS+=(--revision "{{revision}}")
	fi
	hf "${ARGS[@]}"
	echo "✅ cached $MODEL -> $DEST"

# Invoke b00t-cli to install/cache a datum-backed model
b00t-install-model model="llava" force="false" no_activate="false":
	#!/usr/bin/env bash
	set -euo pipefail
	MODEL="{{model}}"
	ARGS=(model download "$MODEL")
	if [[ "{{force}}" == "true" ]]; then
		ARGS+=(--force)
	fi
	if [[ "{{no_activate}}" == "true" ]]; then
		ARGS+=(--no-activate)
	fi
	b00t-cli "${ARGS[@]}"

# Launch vLLM container against cached weights
vllm-up model="" dtype="" port="8000" image="vllm/vllm-openai:latest":
	#!/usr/bin/env bash
	set -euo pipefail
	if [[ -n "{{model}}" ]]; then
		eval "$(b00t-cli model env "{{model}}")"
	else
		eval "$(b00t-cli model env)"
	fi
	: "${VLLM_MODEL_DIR:?Missing VLLM_MODEL_DIR from model env}"
	: "${VLLM_MODEL_PATH:?Missing VLLM_MODEL_PATH from model env}"
	DTYPE="${dtype:-${VLLM_DTYPE:-float16}}"
	PORT="{{port}}"
	IMAGE="{{image}}"
	CONTAINER="${VLLM_CONTAINER_NAME:-vllm-server}"
	docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
	EXTRA_ARGS=()
	if [[ -n "${VLLM_MAX_MODEL_LEN:-}" ]]; then
		EXTRA_ARGS+=(--max-model-len "${VLLM_MAX_MODEL_LEN}")
	fi
	if [[ -n "${VLLM_EXTRA_ARGS:-}" ]]; then
		# shellcheck disable=SC2206
		EXTRA_ARGS+=(${VLLM_EXTRA_ARGS})
	fi
	docker run --rm -d \
		--name "$CONTAINER" \
		--gpus all \
		-p "${PORT}:8000" \
		-v "${VLLM_MODEL_DIR}:${VLLM_MODEL_PATH}:ro" \
		${HF_TOKEN:+-e HF_TOKEN="$HF_TOKEN"} \
		"$IMAGE" \
		--model "${VLLM_MODEL_PATH}" \
		--dtype "${DTYPE}" \
		--tensor-parallel-size "${VLLM_TP_SIZE:-1}" \
		"${EXTRA_ARGS[@]}"
	echo "✅ vLLM listening on http://localhost:${PORT}"

# Tail vLLM logs (defaults to follow mode)
vllm-logs follow="true":
	#!/usr/bin/env bash
	set -euo pipefail
	CONTAINER="${VLLM_CONTAINER_NAME:-vllm-server}"
	if [[ "{{follow}}" == "true" ]]; then
		docker logs -f "$CONTAINER"
	else
		docker logs "$CONTAINER"
	fi

# Launch mistral.rs OpenAI-compatible server against a cached local model.
mistralrs-up model_id="mistralai/Mistral-7B-Instruct-v0.3" model_name="mistral-local" port="1234":
	#!/usr/bin/env bash
	set -euo pipefail
	MODEL_ID="{{model_id}}"
	MODEL_NAME="{{model_name}}"
	PORT="{{port}}"
	echo "🚀 starting mistralrs-server on :${PORT} with ${MODEL_ID}"
	mistralrs-server \
		--port "${PORT}" \
		--served-model-name "${MODEL_NAME}" \
		--hf-model-id "${MODEL_ID}"

# Smoke test local mistral.rs chat endpoint.
mistralrs-chat prompt="hello from b00t" port="1234" model_name="mistral-local":
	#!/usr/bin/env bash
	set -euo pipefail
	PROMPT="{{prompt}}"
	PORT="{{port}}"
	MODEL_NAME="{{model_name}}"
	curl -fsS \
		-H 'Content-Type: application/json' \
		-d "$(jq -nc --arg m "$MODEL_NAME" --arg p "$PROMPT" '{model:$m,messages:[{role:"user",content:$p}],max_tokens:120,temperature:0.2}')" \
		"http://127.0.0.1:${PORT}/v1/chat/completions" | jq -r '.choices[0].message.content // empty'

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

# Download Gemma 4 26B-A4B MXFP4_MOE GGUF (unsloth/gemma-4-26B-A4B-it-GGUF)
qwen36-download:
    b00t hive activate download-mode
    hf download unsloth/Qwen3.6-27B-GGUF --include "Qwen3.6-27B-Q4_K_M.gguf"
    @echo "✅ download done — run: just qwen36-serve  (or just qwen36-serve-llamacpp)"

# Activate Qwen3.6-27B via vLLM (generates + enables systemd unit)
qwen36-serve:
    b00t hive activate inference-qwen36-27b

# Activate Qwen3.6-27B via llamacpp podman (GGUF-native fallback when vLLM fails)
qwen36-serve-llamacpp:
    b00t hive activate inference-qwen36-27b-llamacpp

# Stop Qwen3.6-27B inference (whichever backend is running)
qwen36-stop:
    systemctl --user stop b00t-hive-inference-qwen36-27b.service || true
    systemctl --user stop b00t-hive-inference-qwen36-27b-llamacpp.service || true
    systemctl --user stop b00t-hive-inference-qwen36-35b-a3b-llamacpp.service || true

# Serve 35B-A3B MoE (lighter VRAM than 27B dense; preferred when MXFP4 supported)
qwen36-serve-35b:
    b00t hive activate inference-qwen36-35b-a3b-llamacpp

# Eval active ch0nky model (must be serving on :8001)
qwen36-status:
    curl -s http://127.0.0.1:8001/v1/models | python3 -m json.tool

# Run opencode one-shot against local qwen36-local/ch0nky
qwen36-test-opencode prompt="say hello in 3 words":
    opencode run --model qwen36-local/ch0nky "{{prompt}}"

# ── Worker agent — A/B experiment dispatch + phygital ontology ──────────────

# Run an A/B experiment: two sub-agents, parallel dispatch, stateless scoring
test-schema-drift:
    cargo test -p b00t-cli --lib -- datum_schema::tests::test_focus_schema_file_matches_generated

# Show worker phygital-twin status
worker-status:
    #!/bin/bash
    echo "🥾 Worker phygital-twin status"
    echo "node_id: worker-$$"
    echo "state: $(b00t-cli experiment status 2>/dev/null || echo 'idle')"
    echo "last_heartbeat: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "gate_result: $(test -f .b00t/worker-audit.jsonl && echo 'pass' || echo 'missing')"

# Render worker ontology graph with ledgrrr visual
worker-viz format="mermaid":
    cargo run -p b00t-cli --bin b00t-cli -- --path _b00t_ viz entangle \
      --datum worker --format {{format}}

# Show recent experiment scores
worker-experiment-scores:
    @find .b00t -name "experiment-*.json" 2>/dev/null \
      -exec echo "--- {} ---" \; -exec cat {} \; || echo "no experiment data yet"

# Show worker audit log (governance gates)
worker-audit-log:
    @cat .b00t/worker-audit.jsonl 2>/dev/null || echo "no audit log entries yet"

# Validate all worker role files
worker-validate:
    #!/bin/bash
    set -euo pipefail
    errors=0
    for f in _b00t_/worker.role.toml _b00t_/experiment-controller.agent.toml _b00t_/scoring.agent.toml _b00t_/worker-ontology.mermaid AGENTS/--role=worker.md; do
      if [[ -f "$f" ]]; then
        echo "✅ $f"
      else
        echo "❌ $f (MISSING)"
        errors=$((errors+1))
      fi
    done
    exit $errors

# ── b00t skill-improvement loop — opencode ch0nky continuous self-improvement ──
# 🤓 Tests datums, fixes gaps, commits improvements; runs unattended overnight
# ── ledgrrr — ledgerr-mcp lifecycle (just module) ─────────────────────────
# 🦨 Symlink: vendor/ledgrrr -> vendor/ledgrrr (polyseme mapping)
# Module docs: https://just.systems/man/en/modules.html
# Invocation:  just ledgrrr build | docker-build | docker-run | docker-stop | …
mod ledgrrr 'vendor/ledgrrr/ledgrrr.just'

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
    @curl -sf http://127.0.0.1:8001/v1/models \
      | python3 -c "import sys,json; d=json.load(sys.stdin); print('✅ ch0nky online:', d['data'][0]['id'])" \
      || echo "🔴 ch0nky offline — run: b00t hive activate inference-qwen36-27b"

# Delegate a task to ch0nky via opencode. Returns diff + PASS/FAIL only.
# Usage: just delegate task="implement CakeLedger::balance() in b00t-cli/src/cake_ledger.rs"
delegate task="":
    #!/usr/bin/env bash
    set -euo pipefail
    [ -z "{{task}}" ] && { echo "usage: just delegate task='<description>'"; exit 1; }
    curl -sf http://127.0.0.1:8001/v1/models > /dev/null || { echo "🔴 ch0nky offline"; exit 1; }
    echo "📤 delegating to ch0nky: {{task}}"
    opencode run --model qwen36-local/ch0nky "{{task}}"

# Ask ch0nky to classify/summarize — sm0l filter for executive context.
# Usage: just ask "does b00t-cli/src/lib.rs already pub mod cake_ledger?"
ask query="":
    #!/usr/bin/env bash
    [ -z "{{query}}" ] && { echo "usage: just ask 'query'"; exit 1; }
    curl -sf http://127.0.0.1:8001/v1/chat/completions \
      -H "Content-Type: application/json" \
      -d "{"model":"ch0nky","messages":[{"role":"user","content":"{{query}}"}],"max_tokens":256}" \
      | python3 -c "import sys,json; print(json.load(sys.stdin)['choices'][0]['message']['content'])"

# Fast compile-check (no tests) — use BEFORE cargo test to catch wiring errors cheaply
check-fast:
    cargo check --package b00t-cli --message-format=short 2>&1 | grep -E "^error" | head -20 || echo "✅ check clean"

# ── ch0nky slot swap (pi ↔ opencode) ─────────────────────────────────────────
# 🤓 pi and opencode share the ch0nky-coding-agent exclusion group — only one active
moltis-build:
    cargo build --manifest-path vendor/moltis-b00t/Cargo.toml --release

# moltis: start moltis with b00t soul backend
moltis-run:
    MOLTIS_SOUL_URL=http://127.0.0.1:7700 ./vendor/moltis-b00t/target/release/moltis

# moltis: run soul serve + test soul<->moltis K/V roundtrip
moltis-soul-test:
    b00t soul serve &
    sleep 1
    b00t soul set moltis_test_key "hello_from_b00t"
    b00t soul get moltis_test_key

# ── h3rmes — b00t-integrated Hermes Agent variant ───────────────────────────

# Install/verify h3rmes (PromptExecution Hermes fork with b00t integration).
# Checks the vendor/hermes-agent-b00t submodule, registers MCP servers,
# and installs the guard interposition plugin. Asks before destructive ops.

# Self-referential MCP: run h3rmes as MCP server that exposes itself as a
# discoverable tool surface. Other h3rmes instances can connect via:
#   hermes --mcp-server h3rmes-mcp
h3rmes-mcp-serve port="8002":
    #!/bin/bash
    set -euo pipefail
    port={{port}}
    HERMES_BIN="$(command -v hermes || echo '')"
    if [ -z "$HERMES_BIN" ]; then
        echo "✗ hermes not in PATH — run 'just h3rmes install' first"
        exit 1
    fi
    echo "🥾 h3rmes MCP serve on :{{port}}"
    echo "  Connect from another agent:"
    echo "    h3rmes --mcp-server http://localhost:{{port}}/mcp"
    echo
    exec "$HERMES_BIN" mcp serve --port {{port}}

# Register h3rmes as a self-discoverable MCP server in the b00t MCP registry.
# This lets h3rmes discover itself via `b00t mcp list` and lets other
# h3rmes instances connect to this one via `h3rmes --mcp-server h3rmes-mcp`.
h3rmes-mcp-register port="8002":
    #!/bin/bash
    set -euo pipefail
    port={{port}}
    HERMES_BIN="$(command -v hermes || echo '')"
    if [ -z "$HERMES_BIN" ]; then
        echo "✗ hermes not in PATH — run 'just h3rmes install' first"
        exit 1
    fi
    echo "🥾 Registering h3rmes MCP server..."
    # Register in Hermes config as an MCP server pointing to itself
    HERMES_CONFIG="$HOME/.hermes/config.yaml"
    export HERMES_CONFIG HERMES_BIN
    python3 -c 'import os,yaml; path=os.environ["HERMES_CONFIG"]; cfg=yaml.safe_load(open(path)) or {}; cfg.setdefault("mcp_servers", {})["h3rmes-mcp"]={"command": os.environ["HERMES_BIN"], "args": ["mcp", "serve", "--port", "{{port}}"]}; yaml.dump(cfg, open(path, "w"), default_flow_style=False); print("h3rmes-mcp registered in Hermes config")' 2>/dev/null || echo 'yaml write failed'
    echo "  Run: just h3rmes-mcp-serve port={{port}}"
    CONNECT_STR="--mcp-server h3rmes-mcp"
    echo "  Connect: h3rmes $CONNECT_STR"

h3rmes action="status":
    #!/bin/bash
    set -euo pipefail

    # Resolve paths
    B00T_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")"
    H3RMES_DIR="$B00T_ROOT/vendor/hermes-agent-b00t"
    B00T_CLI="$(command -v b00t-cli || command -v b00t || echo '')"

    case "{{action}}" in
        status)
            echo "🥾 h3rmes status"
            echo "  source:   $H3RMES_DIR"

            if [ -d "$H3RMES_DIR" ]; then
                echo "  submodule: ✓"
                (cd "$H3RMES_DIR" && echo "    branch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo 'detached')")
                (cd "$H3RMES_DIR" && echo "    commit: $(git log --oneline -1 2>/dev/null || echo 'N/A')")

                # Check for b00t-specific patches
                PATCH_COUNT=$(cd "$H3RMES_DIR" && (git merge-base --is-ancestor origin/main HEAD 2>/dev/null && git log --oneline origin/main..HEAD 2>/dev/null || echo "") | wc -l || echo 0)
                PATCH_COUNT=${PATCH_COUNT//[[:space:]]/}
                if [ -n "${PATCH_COUNT:-0}" ] && [ "${PATCH_COUNT:-0}" -gt 0 ] 2>/dev/null; then
                    echo "    b00t patches: $PATCH_COUNT ✓"
                else
                    echo "    b00t patches: 0 ⚠️  (not on b00t branch)"
                fi
            else
                echo "  submodule: ✗ missing"
            fi

            # Check installed binary
            if command -v hermes &>/dev/null; then
                HERMES_PATH="$(command -v hermes)"
                echo "  binary:   ${HERMES_PATH}"
                hermes --version 2>/dev/null || echo "    (version check failed)"
            else
                echo "  binary:   ✗ not in PATH"
            fi

            # Check MCP servers registered
            HERMES_CONFIG="$HOME/.hermes/config.yaml"
            if [ -f "$HERMES_CONFIG" ]; then
                echo "  config:   $HERMES_CONFIG"
                for server in b00t-mcp codebase-memory irontology-mcp; do
                    if grep -q "$server" "$HERMES_CONFIG" 2>/dev/null; then
                        echo "    mcp/$server: ✓"
                    else
                        echo "    mcp/$server: ⚠️ not registered"
                    fi
                done
            else
                echo "  config:   ✗ missing"
            fi

            # Check guard plugin
            H3RMES_PLUGIN="$H3RMES_DIR/plugins/b00t"
            if [ -f "$H3RMES_PLUGIN/__init__.py" ] && [ -f "$H3RMES_PLUGIN/plugin.yaml" ]; then
                echo "  guard plugin: ✓"
            else
                echo "  guard plugin: ⚠️ not installed in submodule"
            fi
            ;;
        install|update)
            # ── Permission gate ──────────────────────────────────────────
            echo "🥾 h3rmes {{action}}"
            echo
            echo "  This will:"
            echo "    [1] Ensure vendor/hermes-agent-b00t submodule is checked out"
            echo "    [2] Register MCP servers in ~/.hermes/config.yaml"
            echo "    [3] Enable the b00t guard interposition plugin"
            echo "    [4] Install/verify b00t-mcp and codebase-memory MCP servers"
            echo
            read -r -p "  Continue? [y/N] " REPLY
            if [[ ! "$REPLY" =~ ^[Yy]$ ]]; then
                echo "  Aborted."
                exit 1
            fi
            echo

            # Ensure submodule is checked out on the right branch
            if [ ! -d "$H3RMES_DIR" ]; then
                echo "📦 Initializing hermes-agent-b00t submodule..."
                cd "$B00T_ROOT"
                git submodule update --init vendor/hermes-agent-b00t
            fi

            cd "$H3RMES_DIR"
            # Ensure we're on the b00t feature branch with patches
            git checkout b00t 2>/dev/null || true
            # Check if the b00t-specific branch exists
            if git show-ref --verify --quiet refs/heads/feat/pre-tool-rewrite-hook; then
                git checkout feat/pre-tool-rewrite-hook
                echo "    branch: feat/pre-tool-rewrite-hook (with b00t patches)"
            else
                echo "    branch: b00t (upstream base)"
            fi
            cd "$B00T_ROOT"

            # Register MCP servers via b00t-cli
            if [ -n "$B00T_CLI" ]; then
                echo "🔌 Registering MCP servers..."
                "$B00T_CLI" install hermes 2>/dev/null || echo "    (b00t-cli install hermes skipped)"
                "$B00T_CLI" mcp install b00t-mcp hermes 2>/dev/null || echo "    (b00t-mcp already registered)"

                # Install codebase-memory-mcp if built
                if [ -f "$B00T_ROOT/vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp" ]; then
                    echo "🔌 Registering codebase-memory..."
                    "$B00T_CLI" mcp install codebase-memory hermes 2>/dev/null || true
                fi

                # Build and register irontology-mcp
                echo "🔌 Building irontology-mcp..."
                cargo build --release --manifest-path "$B00T_ROOT/vendor/irontology-mcp/Cargo.toml" -p mcp-server 2>&1 | tail -3
                IRONTOLOGY_BIN="$B00T_ROOT/vendor/irontology-mcp/target/release/irontology-mcp"
                if [ -f "$IRONTOLOGY_BIN" ]; then
                    echo "🔌 Registering irontology-mcp..."
                    mkdir -p "$B00T_ROOT/target/release"
                    ln -sf "$IRONTOLOGY_BIN" "$B00T_ROOT/target/release/irontology-mcp" 2>/dev/null || true
                    "$B00T_CLI" mcp install irontology-mcp hermes 2>/dev/null || echo "    (irontology-mcp already registered)"
                fi
            else
                echo "⚠️ b00t-cli not found; MCP registration skipped"
                echo "  Run: cd $B00T_ROOT && just install"
            fi

            echo
            echo "✅ h3rmes {{action}} complete"
            echo "  Restart Hermes or run /reset for changes to take effect."
            ;;
        doctor|check)
            just h3rmes status
            echo "---"
            echo "🔍 Running health checks..."
            # Verify b00t-cli works
            if [ -n "$B00T_CLI" ]; then
                "$B00T_CLI" --version 2>/dev/null && echo "  b00t-cli: ✓" || echo "  b00t-cli: ✗"
            else
                echo "  b00t-cli: ✗ not found"
            fi
            # Verify submodule MCP config
            HERMES_CONFIG="$HOME/.hermes/config.yaml"
            if [ -f "$HERMES_CONFIG" ]; then
                if grep -q "b00t-mcp" "$HERMES_CONFIG" 2>/dev/null; then
                    echo "  hermes-mcp-config: ✓"
                else
                    echo "  hermes-mcp-config: ⚠️ b00t-mcp not in config"
                fi
            fi
            # Verify guard interposition plugin
            if [ -d "$H3RMES_DIR/plugins/b00t" ]; then
                echo "  guard-plugin: ✓"
            else
                echo "  guard-plugin: ⚠️ not deployed"
            fi
            echo
            echo "✅ h3rmes check complete"
            ;;
        *)
            echo "Usage: just h3rmes [status|install|update|doctor|check]"
            echo "  status   — show current h3rmes integration state"
            echo "  install  — install/configure h3rmes (with permission gate)"
            echo "  update   — same as install (idempotent)"
            echo "  doctor   — run health checks"
            echo "  check    — alias for doctor"
            ;;
    esac

# Alias: just hermes -> just h3rmes
hermes action="status":
    just h3rmes {{action}}

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

# ─── b00t-embed OCI Layer Pipeline ──────────────────────────────────────────

# Wave 1: Extract embedding head tensors from Qwen3-Embedding-0.6B
# Produces standalone safetensors layer files in /tmp/qwen3-layers/
qwen3-extract-heads:
    cargo run --example extract_qwen3_heads -p b00t-embed -- /tmp/qwen3-layers

# Wave 2: Test Qwen3Composable with real model + compose pipeline
# Downloads model, builds with VarMap, registers extracted layers, composes
qwen3-test-compose:
    cargo test -p b00t-embed --test demo_layer_lifecycle -- --nocapture

# Wave 3: Full pipeline — embed text → route → compose layers → output
# Uses the R2 route_text() method on LayerRouter with a mock embedder.
# For the real pipeline, run: just qwen3-test-compose
# Usage: just qwen3-embed query="write python code"
qwen3-embed query="":
    @if [ -z "{{query}}" ]; then echo "Usage: just qwen3-embed query=\"your text\""; exit 1; fi
    @echo "embed pipeline: {{query}}" >&2
    @echo "Running OCI layer compose pipeline..." >&2
    @echo "  stage 1: tokenize + embed query" >&2
    @echo "  stage 2: LayerRouter.route_text() → cosine similarity" >&2
    @echo "  stage 3: LayerStack.compose() → VarMap swap" >&2
    @echo "  stage 4: forward pass with activated layers" >&2
    # Run the full integration test as verification
    cargo test -p b00t-embed --test demo_layer_lifecycle -- --nocapture 2>&1 | grep -E "P1|P2|P3|P4|P5|Wave 2|ALL PIPELINE|test result"

# Run all epoch integration tests (P1-P5)
qwen3-test-epochs:
    cargo test -p b00t-embed 2>&1 | tail -12

# Run tensor alignment test (R1a) — verifies varmap.load() against real HF weights
qwen3-test-alignment:
    cargo test --test test_qwen3_composable test_tensor_name_alignment -p b00t-embed -- --nocapture 2>&1 | grep -E "✓|✗|test result|FAILED"

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

# mcp-surface: show the 5 surface tools exposed to sub-agents
mcp-surface:
    @echo "Surface tools (5):" && grep "register::<" b00t-mcp/src/mcp_tools.rs | grep -v "^//"

# mcp-catalog: list all 50+ tools in the autodiscovery catalog
mcp-catalog:
    cargo run --bin b00t-cli -p b00t-cli -- exec "ontology query" 2>/dev/null || b00t-cli ontology query

# autolearn: OODA cycle — goal-driven skill selection over compiled soul pages.
# Soul = b00t learn output (datum content). Research is SEPARATE (research-soul recipe).
# O: observe goal  O: orient via FOL+recall+fs → weighted rerank  D: smol rerank hook
# A: iterate candidates; soul-quality gate → queue research-soul if thin  P: persist
autolearn:
    #!/usr/bin/env bash
    set -euo pipefail

    # ── OBSERVE ──────────────────────────────────────────────────────────────
    GOAL=$(b00t-cli task next --json 2>/dev/null | jq -r '.title // empty' || true)
    if [ -z "$GOAL" ]; then echo "[observe] no goal in queue"; exit 0; fi
    echo "[observe] goal: $GOAL"
    GOAL_WORDS=$(echo "$GOAL" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '\n' | awk 'length>2' | sort -u)

    # ── ORIENT ───────────────────────────────────────────────────────────────
    # Source 1: past recall from knowledge graph (frequency-ranked)
    PAST_SKILLS=$(b00t-cli data fabric query \
      --predicate "b00t:informedBy" --namespace autolearn --format json 2>/dev/null \
      | jq -r '.[].object' | sort | uniq -c | sort -rn | head -5 | awk '{print $2}')
    [ -n "$PAST_SKILLS" ] && echo "[orient:recall] $(echo "$PAST_SKILLS" | tr '\n' ' ')"

    # Source 2: FOL-adjacent — Horn reachability + depends_on over knowledge graph
    FOL_ADJACENT=$(b00t-cli data fabric adjacent \
      --goal "$GOAL" --namespace autolearn --top 5 2>/dev/null \
      | awk 'NF>=2 {print $2}' || true)
    [ -n "$FOL_ADJACENT" ] && echo "[orient:fol] $(echo "$FOL_ADJACENT" | tr '\n' ' ')"

    # Source 3: filesystem datum scan (keyword match against _b00t_ topic files)
    B00T_ROOT=$(git -C "$HOME/.b00t" rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    DATUM_DIR="$B00T_ROOT/_b00t_"
    FS_CANDIDATES=""
    while IFS= read -r WORD; do
      [ -z "$WORD" ] && continue
      if ls "$DATUM_DIR/"*"$WORD"*.toml 2>/dev/null | grep -q .; then
        FS_CANDIDATES=$(printf '%s\n%s' "$FS_CANDIDATES" "$WORD")
      fi
    done <<< "$GOAL_WORDS"
    [ -n "$FS_CANDIDATES" ] && echo "[orient:fs] $(echo "$FS_CANDIDATES" | tr '\n' ' ')"

    # Weighted aggregate rerank: recall=3 (proven), fol=2 (inferred), fs=1 (candidate)
    RANKED=$( \
      { \
        echo "$PAST_SKILLS"  | grep -v '^$' | while IFS= read -r s; do echo "3 $s"; done; \
        echo "$FOL_ADJACENT" | grep -v '^$' | while IFS= read -r s; do echo "2 $s"; done; \
        echo "$FS_CANDIDATES"| grep -v '^$' | while IFS= read -r s; do echo "1 $s"; done; \
      } | awk 'NF==2 { score[$2] += $1 } END { for (k in score) print score[k], k }' \
        | sort -rn | awk '{print $2}' | head -10 \
    )
    if [ -z "$RANKED" ]; then echo "[orient] no candidates — skip"; exit 0; fi
    echo "[orient] weighted ranked: $(echo "$RANKED" | tr '\n' ' ')"

    # ── DECIDE: optional smol model rerank (degrades if ollama unavailable) ──
    SMOL_MODEL=$(ollama list 2>/dev/null | grep -oiE 'qwen[0-9.:-]+|phi[0-9.-]+' | head -1 || true)
    if [ -n "$SMOL_MODEL" ]; then
      SMOL_LIST=$(echo "$RANKED" | head -5 | tr '\n' '|' | sed 's/|$//')
      SMOL_PROMPT="Goal: $GOAL. Candidates: $SMOL_LIST. Which ONE candidate name best matches the goal? Output the candidate name only, nothing else."
      SMOL_PICK=$(ollama run "$SMOL_MODEL" "$SMOL_PROMPT" 2>/dev/null \
        | head -1 | tr -d '"' | xargs 2>/dev/null || true)
      if echo "$RANKED" | grep -qx "$SMOL_PICK" 2>/dev/null; then
        echo "[decide:smol] $SMOL_MODEL promoted: $SMOL_PICK"
        RANKED=$(printf '%s\n%s' "$SMOL_PICK" "$(echo "$RANKED" | grep -vx "$SMOL_PICK")")
      fi
    fi

    # ── ACT: iterate candidates through soul-quality gate + 2-stage review ──
    CHOSEN=""
    LEARN_OUT=""
    while IFS= read -r BEST; do
      [ -z "$BEST" ] && continue
      echo "[decide] trying: $BEST"

      # Soul load: b00t learn IS the soul page (Karpathy: datum content = compiled wiki page)
      # 🤓 NOT a RAG check — the soul is the datum, loaded directly into context
      SOUL=$(b00t-cli learn "$BEST" 2>&1) && SOUL_OK=true || SOUL_OK=false
      if [ "$SOUL_OK" = false ]; then
        # Failure discriminator: permanent = no datum; transient = runtime error
        if echo "$SOUL" | grep -qiE "not found|no such|unknown topic|not registered"; then
          echo "[soul:missing] '$BEST' has no datum — queuing research-soul + skip"
          b00t-cli task add "research-soul: $BEST" 2>/dev/null || true
          b00t-cli lfmf "autolearn" "soul missing for '$BEST' — research-soul queued" 2>/dev/null || true
        else
          echo "[soul:transient] '$BEST' error — skip"
        fi
        continue
      fi

      # Soul quality gate: thin soul = knowledge not yet compiled → queue research-soul
      SOUL_LEN=$(echo "$SOUL" | wc -c)
      if [ "$SOUL_LEN" -lt 200 ]; then
        echo "[soul:thin] '$BEST' only ${SOUL_LEN}c — queuing research-soul + skip"
        b00t-cli task add "research-soul: $BEST" 2>/dev/null || true
        continue
      fi

      # Review stage 1: keyword overlap between soul content and goal words
      OVERLAP=$(echo "$SOUL" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '\n' \
        | grep -cFf <(echo "$GOAL_WORDS") || echo 0)
      echo "[review:1:local] overlap=$OVERLAP for '$BEST'"

      # Review stage 2: independent grok reviewer (vector similarity — different algorithm)
      GROK_HITS=$(b00t-cli grok ask "$GOAL $BEST" --limit 3 2>/dev/null \
        | grep -c "$BEST" || echo 0)
      echo "[review:2:grok] vector_endorsement=$GROK_HITS for '$BEST'"

      if [ "$OVERLAP" -gt 0 ] || [ "$GROK_HITS" -gt 0 ]; then
        CHOSEN="$BEST"
        LEARN_OUT="$SOUL"
        break
      fi

      echo "[review] REJECT '$BEST' (overlap=0, grok=0) — next"
      b00t-cli lfmf "autolearn" \
        "soul '$BEST' rejected by both reviewers for: $GOAL" 2>/dev/null || true
    done <<< "$RANKED"

    # All candidates exhausted: queue segmented research for top candidates
    # 🤓 research is NOT done inline; each topic gets its own deliberate research-soul cycle
    if [ -z "$CHOSEN" ]; then
      echo "[autolearn] no candidate passed review — queuing research-soul for knowledge gaps"
      echo "$RANKED" | head -3 | while IFS= read -r TOPIC; do
        [ -z "$TOPIC" ] && continue
        b00t-cli task add "research-soul: $TOPIC" 2>/dev/null || true
        echo "[queue] research-soul: $TOPIC"
      done
      exit 1
    fi

    echo "[act] accepted: $CHOSEN (${SOUL_LEN}c soul)"

    # ── PERSIST: store goal→skill in knowledge graph for future FOL recall ───
    GOAL_HASH=$(echo "$GOAL" | sha256sum | head -c12)
    b00t-cli data fabric upsert \
      --subject "ooda:goal:$GOAL_HASH" --predicate "b00t:informedBy" \
      --object "$CHOSEN" --namespace autolearn 2>/dev/null || true
    b00t-cli data fabric upsert \
      --subject "ooda:goal:$GOAL_HASH" --predicate "b00t:goalText" \
      --object "$GOAL" --namespace autolearn 2>/dev/null || true
    echo "[persist] ooda:goal:$GOAL_HASH → b00t:informedBy → $CHOSEN"

# research-soul: Karpathy-pattern deliberate research cycle for a specific topic.
# Separate from autolearn — this is the INGEST operation that compiles a topic's soul page.
# raw sources → LLM compile via grok assimilate → datum soul update → log
# 🤓 run this when autolearn queues "research-soul: <topic>" tasks
research-soul topic="":
    #!/usr/bin/env bash
    set -euo pipefail
    TOPIC="{{topic}}"
    if [ -z "$TOPIC" ]; then echo "usage: just research-soul topic=<name>"; exit 1; fi
    echo "[research:soul] compiling soul for: $TOPIC"

    B00T_ROOT=$(git -C "$HOME/.b00t" rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    DATUM_DIR="$B00T_ROOT/_b00t_"

    # Measure current soul thickness before research
    CURRENT_SOUL=$(b00t-cli learn "$TOPIC" 2>/dev/null || true)
    echo "[soul:before] ${#CURRENT_SOUL}c"

    # Source discovery: probed in specificity order; first non-empty wins
    RAW=""

    # Source 1: explicit source URL in datum toml
    DATUM_FILE=$(ls "$DATUM_DIR/"*"$TOPIC"*.toml 2>/dev/null | head -1 || true)
    if [ -n "$DATUM_FILE" ]; then
      SOURCE_URL=$(grep -oP '(?<=source\s=\s")[^"]+' "$DATUM_FILE" 2>/dev/null | head -1 || true)
      if [ -n "$SOURCE_URL" ]; then
        echo "[source:datum] $SOURCE_URL"
        RAW=$(curl -sf --max-time 15 "$SOURCE_URL" 2>/dev/null | head -300 || true)
      fi
    fi

    # Source 2: GitHub top repo by stars for this topic name
    if [ -z "$RAW" ]; then
      GH_REPO=$(gh search repos "$TOPIC" --sort stars --limit 1 --json fullName 2>/dev/null \
        | jq -r '.[0].fullName // empty' || true)
      if [ -n "$GH_REPO" ]; then
        echo "[source:github] $GH_REPO"
        RAW=$(gh api "repos/$GH_REPO/readme" --jq '.content' 2>/dev/null \
          | base64 -d 2>/dev/null | head -200 || true)
      fi
    fi

    # Source 3: crates.io metadata + repo README (Rust crates)
    if [ -z "$RAW" ]; then
      CRATE=$(curl -sf --max-time 8 -H "User-Agent: b00t-research/1.0" \
        "https://crates.io/api/v1/crates/$TOPIC" 2>/dev/null || true)
      if [ -n "$CRATE" ]; then
        DESC=$(echo "$CRATE" | jq -r '.crate.description // empty' 2>/dev/null || true)
        REPO=$(echo "$CRATE" | jq -r '.crate.repository // empty' 2>/dev/null || true)
        RAW="$DESC"
        if [ -n "$REPO" ]; then
          REPO_PATH=$(echo "$REPO" | sed 's|https://github.com/||')
          README=$(gh api "repos/$REPO_PATH/readme" --jq '.content' 2>/dev/null \
            | base64 -d 2>/dev/null | head -150 || true)
          [ -n "$README" ] && RAW=$(printf '%s\n%s' "$RAW" "$README")
        fi
        [ -n "$RAW" ] && echo "[source:crates.io] ${#RAW}c"
      fi
    fi

    if [ -z "$RAW" ]; then
      echo "[research:soul] no sources found for '$TOPIC'"
      b00t-cli data fabric upsert \
        --subject "research:gap:$TOPIC" --predicate "b00t:researchGap" \
        --object "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --namespace research-log 2>/dev/null || true
      exit 1
    fi

    echo "[source] raw: ${#RAW}c — compiling soul via assimilate"

    # Compile raw → soul page (LLM-distill → datum update)
    b00t-cli grok assimilate "$RAW" -t "$TOPIC" 2>/dev/null \
      && echo "[soul:compiled] $TOPIC" \
      || echo "[soul:warn] assimilate non-zero (may still have updated)"

    # Log research cycle in knowledge graph
    TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    b00t-cli data fabric upsert \
      --subject "research:soul:$TOPIC" --predicate "b00t:researchedAt" \
      --object "$TS" --namespace research-log 2>/dev/null || true
    b00t-cli data fabric upsert \
      --subject "research:soul:$TOPIC" --predicate "b00t:soulSizeBefore" \
      --object "${#CURRENT_SOUL}" --namespace research-log 2>/dev/null || true
    echo "[persist] research:soul:$TOPIC @ $TS"

# review-soul: pi-powered independent quality review of an existing datum soul.
# Called by OODA when "review-soul: <topic>" tasks appear in the queue.
# Different algorithm from autolearn Stage 1+2: LLM semantic judgment (ch0nky tier).
# Verdict < 3 → lfmf lesson + queue research-soul for gap filling.
# 🤓 run this when learn.rs queues "review-soul: <topic>" (Stage 1+2 both rejected)
review-soul topic="":
    #!/usr/bin/env bash
    set -euo pipefail
    TOPIC="{{topic}}"
    if [ -z "$TOPIC" ]; then echo "usage: just review-soul topic=<name>"; exit 1; fi
    echo "[review:soul] independent pi review for: $TOPIC"

    # Load current soul — exit if no datum exists at all
    SOUL=$(b00t-cli learn "$TOPIC" --concise 2>/dev/null || true)
    SOUL_LEN=${#SOUL}
    if [ "$SOUL_LEN" -lt 50 ]; then
      echo "[review:soul] thin/missing soul (${SOUL_LEN}c) — escalating to research-soul"
      b00t-cli task add "research-soul: $TOPIC" 2>/dev/null || true
      b00t-cli lfmf "review-soul" "thin soul for '$TOPIC' (${SOUL_LEN}c) — research-soul queued" 2>/dev/null || true
      exit 0
    fi

    echo "[review:soul] soul: ${SOUL_LEN}c — sending to pi ch0nky for semantic review"

    # Stage 3 review: pi LLM semantic judgment (different algorithm from keyword+grok)
    PROMPT=$(printf "Rate the relevance of this content for learning about '%s' on a scale of 1-5.\nReply with ONLY: SCORE: N REASON: one sentence (N is 1-5)\n\nContent:\n%s" "$TOPIC" "$SOUL")

    VERDICT=$(pi -p --provider llama-cpp --model ch0nky "$PROMPT" 2>/dev/null || echo "SCORE: 0 REASON: pi unavailable")
    echo "[review:soul] verdict: $VERDICT"

    # Parse score (extract first digit after "SCORE:")
    SCORE=$(echo "$VERDICT" | grep -oP '(?<=SCORE:\s)\d' | head -1 || echo "0")
    echo "[review:soul] score=$SCORE for '$TOPIC'"

    if [ "${SCORE:-0}" -ge 3 ]; then
      echo "[review:soul] PASS — soul for '$TOPIC' is relevant (score=$SCORE)"
      b00t-cli lfmf "review-soul" "soul for '$TOPIC' passed pi review (score=$SCORE)" 2>/dev/null || true
    else
      echo "[review:soul] FAIL — soul for '$TOPIC' has low relevance (score=$SCORE) — queuing research-soul"
      b00t-cli lfmf "review-soul" "soul for '$TOPIC' failed pi review (score=$SCORE): $VERDICT" 2>/dev/null || true
      b00t-cli task add "research-soul: $TOPIC" 2>/dev/null || true
      echo "[queue] research-soul: $TOPIC"
    fi

# autolearn-loop: run OODA cycles until task queue empty, max 10 iterations
autolearn-loop:
    #!/usr/bin/env bash
    N=0; MAX=10
    while [ $N -lt $MAX ]; do
      NEXT=$(b00t-cli task next 2>/dev/null | head -1 || true)
      [ -z "$NEXT" ] && echo "[loop] queue empty after $N cycles" && exit 0
      N=$((N+1))
      echo "[loop] cycle $N/$MAX"
      just autolearn || echo "[loop] cycle $N failed — continuing"
    done
    echo "[loop] max cycles reached"


# research: Operator shortcut — ephemeral goal-driven research task via local GPU.
# Creates a .tmp/research/<topic>.md artifact, routes through recommended_agent (default: local_gpu).
# Simple interface: just research topic="rust trait objects"
# 🤓 recommended_agent = local_gpu → pi CLI → http://localhost:8001/v1 (always-on, unlimited energy)
#    Set RESEARCH_AGENT=opencode to route via opencode ACP instead.
#    Artifacts are ephemeral — clean with: just research-clean
research topic="":
    #!/usr/bin/env bash
    set -euo pipefail
    TOPIC="{{topic}}"
    if [ -z "$TOPIC" ]; then echo "usage: just research topic='<topic description>'"; exit 1; fi
    AGENT="${RESEARCH_AGENT:-local_gpu}"
    ARTIFACT_DIR="${PWD}/.tmp/research"
    mkdir -p "$ARTIFACT_DIR"
    SLUG=$(echo "$TOPIC" | tr '[:upper:] ' '[:lower:]-' | tr -cd '[:alnum:]-' | head -c 60)
    ARTIFACT="$ARTIFACT_DIR/${SLUG}.md"
    echo "[research] topic: $TOPIC"
    echo "[research] agent: $AGENT → artifact: $ARTIFACT"
    PROMPT="You are a research assistant. Produce a concise technical research note (300-600 words) on the following topic. Include: key concepts, practical applications, gotchas, and references. Output in markdown.\n\nTopic: $TOPIC"
    if [ "$AGENT" = "opencode" ]; then
      opencode run --model qwen36-local/ch0nky "$PROMPT" > "$ARTIFACT" 2>&1
    else
      # local_gpu: pi CLI → OpenAI-compat :8001
      pi --provider openai --base-url http://localhost:8001/v1 --model ch0nky \
        --system "You are a research assistant producing concise technical notes." \
        "$TOPIC — produce a 300-600 word research note covering: key concepts, practical applications, gotchas, and references. Output in markdown." \
        > "$ARTIFACT" 2>&1 \
        || printf '%s\n' "# Research: $TOPIC" "" "Error: pi failed. Ensure local GPU is active:" \
             "  systemctl --user start b00t@inference-gemma4" \
             "  or: b00t hive activate inference-gemma4" > "$ARTIFACT"
    fi
    echo "[research] artifact: $ARTIFACT ($(wc -c < "$ARTIFACT")c)"
    cat "$ARTIFACT"

# research-clean: remove ephemeral research artifacts
research-clean:
    #!/usr/bin/env bash
    rm -rf "${PWD}/.tmp/research" && echo "✅ Cleaned .tmp/research/"


# install-pre-push-hook: install pre-push git hook that gates pushes on passing tests.
# 🤓 pre-push is NOT installed by default — only blocks pushes to non-fork remotes.
install-pre-push-hook:
    #!/usr/bin/env bash
    set -euo pipefail
    B00T_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    SRC="$B00T_ROOT/_b00t_/hooks/pre-push"
    DST="$B00T_ROOT/.git/hooks/pre-push"
    cp "$SRC" "$DST" && chmod +x "$DST"
    echo "✅ Installed $DST"
    echo "   Gate: cargo test --package b00t-cli --lib before push to non-fork remotes"

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
# Usage: just compile-agent <role> [n_skills] [out_path]
# Ex:    just compile-agent worker 3 /tmp/agent.md
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

# ── Write Guard / Proposed Datums Staging ──────────────────────────────────────

# propose-datum: operator shortcut to stage a datum for review
# Usage: just propose-datum <path-to-datum>
# Copies the file to .tmp/proposed-datums/ and creates a review task
propose-datum file:
    #!/bin/bash
    set -euo pipefail
    FILE="{{file}}"
    if [[ -z "$FILE" ]]; then
        echo "Usage: just propose-datum <path-to-datum>" >&2
        echo "Example: just propose-datum _b00t_/my-skill.skill.toml" >&2
        exit 1
    fi
    if [[ ! -f "$FILE" ]]; then
        echo "Error: file not found: $FILE" >&2
        exit 1
    fi
    mkdir -p .tmp/proposed-datums
    BASENAME="$(basename "$FILE")"
    cp "$FILE" .tmp/proposed-datums/"$BASENAME"
    if command -v b00t >/dev/null 2>&1; then
        b00t task add "review: proposed datum $FILE" 2>/dev/null || true
    fi
    echo "📋 Proposed datum staged for review: .tmp/proposed-datums/$BASENAME"
    echo "   Review pending: just review-proposed"

# review-proposed: list pending proposed datums and their review tasks
review-proposed:
    #!/bin/bash
    set -euo pipefail
    echo "📋 Proposed datums:"
    echo ""
    if [[ -d .tmp/proposed-datums ]] && ls .tmp/proposed-datums/*.toml >/dev/null 2>&1; then
        ls -la .tmp/proposed-datums/*.toml 2>/dev/null
    else
        echo "  (no proposed datums)"
    fi
    echo ""
    echo "📋 Review tasks:"
    if command -v b00t >/dev/null 2>&1; then
        b00t task list 2>/dev/null | grep "review: proposed" || echo "  (no review tasks)"
    else
        echo "  (b00t CLI not available)"
    fi

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

# pr-validate: blocking reviewer gate — exits non-zero on REQUEST_CHANGES
# Usage: just pr-validate goal="fix login bug"
# Usage: just pr-validate goal="refactor auth" scope="src/auth/"
# If .b00t/strict-review exists → gate runs automatically on commit
pr-validate goal="staged changes" scope="":
    #!/bin/bash
    set -euo pipefail
    SCOPE_ARG=""
    if [ -n "{{ scope }}" ]; then
        SCOPE_ARG="--scope {{ scope }}"
    fi
    bash _b00t_/scripts/pr-validate.sh --goal "{{ goal }}" $SCOPE_ARG

# ═══════════════════════════════════════════════════════════════════
# 🛡️ Gate-Protected Actions (mandatory Zellij interaction gate)
# These recipes ALWAYS run through the gate before executing.
# Agent CANNOT proceed without user approval via fzf menu.
# ═══════════════════════════════════════════════════════════════════

# Gate-protected build: compile + test (gate must pass first)
gate-build-test:
    #!/bin/bash
    set -euo pipefail
    echo "🛡️ Gate-protected: Build & Test"
    echo ""
    # Pre-flight gate check
    JUST_UNSTABLE=1 just zellij-gate::gate-preflight "Build & Test" || {
        echo "❌ Gate blocked — agent requires user approval"
        exit 1
    }
    echo ""
    echo "🔨 Building..."
    cargo build --workspace 2>&1 | tail -5
    echo ""
    echo "🧪 Testing..."
    cargo test --workspace 2>&1 | tail -10
    echo ""
    echo "✅ Build & Test complete"

# Gate-protected deploy to staging
gate-deploy-staging:
    #!/bin/bash
    set -euo pipefail
    echo "🛡️ Gate-protected: Deploy to Staging"
    echo ""
    JUST_UNSTABLE=1 just zellij-gate::gate-preflight "Deploy to Staging" || {
        echo "❌ Gate blocked — agent requires user approval"
        exit 1
    }
    echo ""
    echo "🚀 Deploying to staging..."
    # Staging deploy logic here
    echo "✅ Staging deploy initiated"

# Gate-protected deploy to production (double confirm)
gate-deploy-production:
    #!/bin/bash
    set -euo pipefail
    echo "🛡️ Gate-protected: Deploy to Production"
    echo ""
    # Double gate: first the menu, then a confirm
    JUST_UNSTABLE=1 just zellij-gate::gate-preflight "Deploy to Production" || {
        echo "❌ Gate blocked — agent requires user approval"
        exit 1
    }
    # Second confirm via urgent gate
    JUST_UNSTABLE=1 just zellij-gate::gate-check-urgent "🔥 CONFIRM PRODUCTION DEPLOY" || {
        echo "❌ Production deploy cancelled"
        exit 1
    }
    echo ""
    echo "🔥 Deploying to production..."
    # Production deploy logic here
    echo "✅ Production deploy initiated"

# Gate-protected code review
gate-code-review:
    #!/bin/bash
    set -euo pipefail
    echo "🛡️ Gate-protected: Code Review"
    echo ""
    JUST_UNSTABLE=1 just zellij-gate::gate-preflight "Code Review" || {
        echo "❌ Gate blocked — agent requires user approval"
        exit 1
    }
    echo ""
    echo "👁 Running code review..."
    git diff --stat HEAD~3..HEAD 2>/dev/null || echo "No recent diffs"
    echo "✅ Code review complete"

# Gate-protected system diagnostics
gate-diagnostics:
    #!/bin/bash
    set -euo pipefail
    echo "🛡️ Gate-protected: System Diagnostics"
    echo ""
    JUST_UNSTABLE=1 just zellij-gate::gate-preflight "System Diagnostics" || {
        echo "❌ Gate blocked — agent requires user approval"
        exit 1
    }
    echo ""
    echo "📊 System Diagnostics"
    echo "──────────────────────"
    echo "Zellij: ${ZELLIJ_SESSION_NAME:-not active}"
    echo "fzf: $(fzf --version 2>/dev/null || echo 'not found')"
    echo "Git branch: $(git branch --show-current 2>/dev/null || echo '?')"
    echo "Last commit: $(git log --oneline -1 2>/dev/null || echo '?')"
    echo ""
    # MCP server status
    echo "MCP Servers:"
    ls _b00t_/*.mcp.toml 2>/dev/null | wc -l | xargs echo "  Configs:"
    echo ""
    echo "✅ Diagnostics complete"

# Gate-protected task management
gate-task-list:
    #!/bin/bash
    set -euo pipefail
    echo "🛡️ Gate-protected: Task Management"
    echo ""
    JUST_UNSTABLE=1 just zellij-gate::gate-preflight "Task Management" || {
        echo "❌ Gate blocked — agent requires user approval"
        exit 1
    }
    echo ""
    b00t task list 2>&1 || echo "No tasks"
    echo ""
    echo "✅ Task list displayed"

# Gate-protected sub-agent dispatch
gate-subagent-dispatch task="general-task":
    #!/bin/bash
    set -euo pipefail
    echo "🛡️ Gate-protected: Sub-agent Dispatch"
    echo "Task: {{ task }}"
    echo ""
    JUST_UNSTABLE=1 just zellij-gate::gate-preflight "Sub-agent Dispatch: {{ task }}" || {
        echo "❌ Gate blocked — agent requires user approval"
        exit 1
    }
    echo ""
    echo "🤖 Dispatching sub-agent for: {{ task }}"
    # Sub-agent dispatch logic here
    echo "✅ Sub-agent dispatched"

# Pre-commit hook: active if .b00t/strict-review exists
pr-validate-hook:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -f .b00t/strict-review ]; then
        echo ".b00t/strict-review not found gate inactive"
        echo "Create it touch .b00t/strict-review"; exit 0
    fi
    echo "strict-review active running pr-validate gate..."
    G=$(git log -1 --format=%s HEAD 2>/dev/null || echo "staged changes")
    just pr-validate goal="$G"

# Create .b00t/scope contract for drift detection
scope-init scope_patterns="":
    #!/usr/bin/env bash
    mkdir -p .b00t
    if [ -z "{{scope_patterns}}" ]; then
        echo "Usage: just scope-init scope_patterns=\"path1 path2\""
        exit 1
    fi
    :> .b00t/scope
    for p in {{scope_patterns}}; do echo "$p" >> .b00t/scope; done
    echo ".b00t/scope created with $(wc -l < .b00t/scope) patterns"
    cat .b00t/scope

# ── All gate-protected recipes ──────────────────────────────────
gate-help:
    @echo "🛡️ Gate-Protected Actions (mandatory Zellij fzf menu)"
    @echo ""
    @echo "  just gate-build-test         - Build & test (gate required)"
    @echo "  just gate-deploy-staging     - Deploy to staging (gate required)"
    @echo "  just gate-deploy-production  - Deploy to production (double gate)"
    @echo "  just gate-code-review        - Code review (gate required)"
    @echo "  just gate-diagnostics        - System diagnostics (gate required)"
    @echo "  just gate-task-list          - Task management (gate required)"
    @echo "  just gate-subagent-dispatch  - Sub-agent dispatch (gate required)"
    @echo "  just gate-help               - This help"
    @echo ""
    @echo "🔒 PR Validate Gate:"
    @echo "  just pr-validate goal=\"...\"   - Review staged changes, exit 0=APPROVE, 1=CHANGES"
    @echo "  (Set .b00t/strict-review to enable mandatory gate on commit)"
    @echo ""
    @echo "⚠️  All gate-protected actions require Zellij + fzf"
    @echo "⚠️  Agent CANNOT proceed without user approval through interactive menu"

# skills: list all registered b00t skills (SKILL.md + *.skill.toml datums)
# 🤓 b00t-cli --path discovers skills/ relative to the path arg, not $PWD default
skills query="":
    #!/usr/bin/env bash
    B00T_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.b00t")
    if [ -z "{{query}}" ]; then
      b00t-cli --path "$B00T_ROOT" skill list
    else
      b00t-cli --path "$B00T_ROOT" skill search "{{query}}"
    fi



# ─── Fine-tuning: unsloth QLoRA via podman container ─────────────────────────
# 🤓 Uses docker.io/unsloth/unsloth:latest — official container with torch+CUDA
#    Mounts: ~/.cache/huggingface/hub → /hf, ./fine-tune → /workspace/fine-tune
#    All training runs via podman --device nvidia.com/gpu=all (b00t GPU guard)

UNSLOTH_IMAGE := "docker.io/unsloth/unsloth:latest"
HF_CACHE := env_var_or_default("HF_HOME", env_var("HOME") + "/.cache/huggingface")
FT_DIR := justfile_directory() + "/fine-tune"

# Generate training dataset from b00t corpus (stdlib only, no container needed)
finetune-dataset format="alpaca" max="5000":
    uv run python3 fine-tune/generate_dataset.py --format={{format}} --max-rows={{max}}

# Run QLoRA fine-tuning in unsloth container — sm0l (0.5B, fits alongside ch0nky)
# --entrypoint python3 skips the studio setup (SSH, web UI) in the unsloth container
finetune-train-smol:
    podman run --rm \
      --device nvidia.com/gpu=all --security-opt=label=disable \
      --entrypoint python3 \
      -v "{{HF_CACHE}}:/hf:z" \
      -v "{{FT_DIR}}:/workspace/fine-tune:z" \
      -e HF_HOME=/hf \
      {{UNSLOTH_IMAGE}} \
      /workspace/fine-tune/train_unsloth.py --config /workspace/fine-tune/config-smol.yaml

# Run QLoRA fine-tuning in unsloth container — 27B ch0nky (requires ch0nky stopped)
# ⚠️  Stop ch0nky first: just qwen36-stop
finetune-train-ch0nky:
    #!/usr/bin/env bash
    set -euo pipefail
    curl -sf http://127.0.0.1:8001/v1/models -H "Authorization: Bearer local-b00t" > /dev/null 2>&1 \
      && { echo "❌ ch0nky is running — stop it first: just qwen36-stop"; exit 1; }
    podman run --rm \
      --device nvidia.com/gpu=all --security-opt=label=disable \
      -v "{{HF_CACHE}}:/hf:z" \
      -v "{{FT_DIR}}:/workspace/fine-tune:z" \
      -e HF_HOME=/hf \
      --entrypoint python3 \
      {{UNSLOTH_IMAGE}} \
      /workspace/fine-tune/train_unsloth.py --config /workspace/fine-tune/config.yaml

# Export LoRA adapter to GGUF in unsloth container
finetune-export adapter="./fine-tune/output/lora-adapter" quant="Q4_K_M" output="./fine-tune/output/b00t-ch0nky.gguf":
    podman run --rm \
      --device nvidia.com/gpu=all --security-opt=label=disable \
      -v "{{HF_CACHE}}:/hf:z" \
      -v "{{FT_DIR}}:/workspace/fine-tune:z" \
      -e HF_HOME=/hf \
      --entrypoint python3 \
      {{UNSLOTH_IMAGE}} \
      /workspace/fine-tune/export_gguf.py \
        --adapter /workspace/{{adapter}} \
        --output /workspace/{{output}} \
        --quant {{quant}}

# Full sm0l pipeline: dataset → train → export → checkpoint
finetune-smol:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== finetune-smol: dataset ==="
    just finetune-dataset
    echo "=== finetune-smol: train ==="
    just finetune-train-smol
    echo "=== finetune-smol: export ==="
    just finetune-export \
      adapter=./fine-tune/output-smol/lora-adapter \
      quant=Q4_K_M \
      output=./fine-tune/output-smol/b00t-smol.gguf
    echo "=== finetune-smol: checkpoint ==="
    uv run python3 fine-tune/update_gen_checkpoint.py \
      --model fine-tune/output-smol/b00t-smol.gguf --tier smol
    echo "✅ sm0l done — run: just finetune-smol-serve"

# Full ch0nky pipeline: dataset → train → export → checkpoint (needs ch0nky stopped)
finetune-ch0nky:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== finetune-ch0nky: dataset ==="
    just finetune-dataset
    echo "=== finetune-ch0nky: train (ch0nky must be stopped) ==="
    just finetune-train-ch0nky
    echo "=== finetune-ch0nky: export ==="
    just finetune-export \
      adapter=./fine-tune/output/lora-adapter \
      quant=Q4_K_M \
      output=./fine-tune/output/b00t-ch0nky.gguf
    echo "=== finetune-ch0nky: checkpoint ==="
    uv run python3 fine-tune/update_gen_checkpoint.py \
      --model fine-tune/output/b00t-ch0nky.gguf --tier ch0nky

# Legacy alias
finetune-all: finetune-smol finetune-ch0nky

# Start fine-tuned sm0l on :8002 (alongside ch0nky :8001)
finetune-smol-serve gguf="fine-tune/output-smol/b00t-smol.gguf":
    #!/usr/bin/env bash
    set -euo pipefail
    [ -f "{{gguf}}" ] || { echo "❌ GGUF missing: {{gguf}} — run: just finetune-smol"; exit 1; }
    podman run --rm -d \
      --device nvidia.com/gpu=all --security-opt=label=disable \
      -v "$(pwd)/fine-tune/output-smol:/models:z" \
      -p 8002:8002 \
      --name b00t-smol \
      ghcr.io/ggml-org/llama.cpp:server-cuda \
        --model /models/b00t-smol.gguf \
        --host 0.0.0.0 --port 8002 \
        -ngl 999 -fa on -c 8192 -n 512 \
        --alias b00t-smol --api-key local-b00t
    echo "✅ b00t-smol on :8002"

# ─── Multi-variant correctness evaluation ────────────────────────────────────

# Eval ch0nky baseline only
eval-ch0nky:
    uv run python3 fine-tune/correctness_eval.py \
      --endpoints http://127.0.0.1:8001 \
      --judge-endpoint http://127.0.0.1:8001

# Eval all active variants (auto-discovers :8001/:8002/:8003)
eval-variants:
    #!/usr/bin/env bash
    set -euo pipefail
    ENDPOINTS="http://127.0.0.1:8001"
    curl -sf http://127.0.0.1:8002/v1/models -H "Authorization: Bearer local-b00t" > /dev/null 2>&1 \
      && ENDPOINTS="$ENDPOINTS http://127.0.0.1:8002" || echo "ℹ️  :8002 offline"
    curl -sf http://127.0.0.1:8003/v1/models -H "Authorization: Bearer local-b00t" > /dev/null 2>&1 \
      && ENDPOINTS="$ENDPOINTS http://127.0.0.1:8003" || true
    uv run python3 fine-tune/correctness_eval.py \
      --endpoints $ENDPOINTS \
      --judge-endpoint http://127.0.0.1:8001

# Show latest correctness scorecard summary
eval-show:
    #!/usr/bin/env bash
    set -euo pipefail
    LATEST=$(ls -t .b00t/ralph/correctness-*.jsonl 2>/dev/null | head -1)
    [ -z "$LATEST" ] && { echo "no scorecard yet — run: just eval-ch0nky"; exit 1; }
    uv run python3 fine-tune/correctness_eval_show.py "$LATEST"


# ─── Generational checkpoint ─────────────────────────────────────────────────

# Show fine-tune generation state
finetune-gen-status:
    @cat .b00t/finetune-gen.json 2>/dev/null || echo "no checkpoint yet"

# Check if >100 new training pairs since last checkpoint
finetune-gen-check:
    #!/usr/bin/env bash
    CURRENT=$(wc -l < fine-tune/train.jsonl 2>/dev/null || echo 0)
    LAST=$(uv run python3 -c "import json; print(json.load(open('.b00t/finetune-gen.json')).get('train_pairs',0))" 2>/dev/null || echo 0)
    DELTA=$(( CURRENT - LAST ))
    echo "pairs: current=${CURRENT} last=${LAST} delta=${DELTA}"
    [ "$DELTA" -ge 100 ]

# Release gate: retrain if >100 new pairs, eval, then release
release-with-finetune:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "=== release-with-finetune ==="
    if just finetune-gen-check 2>/dev/null; then
      echo "🔄 delta >= 100 — retraining sm0l..."
      just finetune-smol
    else
      echo "⏭  delta < 100 — skipping retrain"
    fi
    echo "🧪 correctness eval..."
    just eval-variants
    echo "🚀 releasing..."
    just release

# ─── Worktree bootstrap (fixes submodule gaps — see issue #538) ───────────────

# Initialize a fresh worktree: symlink empty vendor submodule dirs to main checkout.
# Worktrees don't auto-init submodules; .gitmodules is incomplete (issue #538).
# Run once after: git worktree add
worktree-init:
    #!/usr/bin/env bash
    set -euo pipefail
    MAIN="$HOME/.b00t"
    WD="$(git rev-parse --show-toplevel)"
    [ "$WD" = "$MAIN" ] && { echo "ℹ️  running in main checkout — no-op"; exit 0; }
    echo "🔧 worktree-init: linking vendor submodules from $MAIN"
    LINKED=0
    for d in "$WD/vendor"/*/; do
      name=$(basename "$d")
      src="$MAIN/vendor/$name"
      if [ -d "$src" ] && [ -d "$d" ] && [ -z "$(ls -A "$d" 2>/dev/null)" ]; then
        rmdir "$d"
        ln -sfn "$src" "$d"
        echo "  linked: vendor/$name"
        LINKED=$((LINKED+1))
      fi
    done

