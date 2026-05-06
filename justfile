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
mod irontology 'vendor/irontology-mcp/irontology.just'

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

viz-entangle datum="l3dg3rr" format="mermaid":
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
    cd b00t-lib-chat && cargo publish --dry-run --allow-dirty

    echo "📦 Testing b00t-c0re-lib..."
    cd ../b00t-c0re-lib && cargo publish --dry-run --allow-dirty

    echo "📦 Testing b00t-cli..."
    cd ../b00t-cli && cargo publish --dry-run --allow-dirty

    echo "📦 Testing b00t-mcp..."
    cd ../b00t-mcp && cargo publish --dry-run --allow-dirty

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
    echo "removed"

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
        echo "    just commit-hook"
        echo "else"
        echo "    echo \"just is required to run commit-hook\" >&2"
        echo "    exit 1"
        echo "fi"
    } > "${HOOK_PATH}"
    chmod +x "${HOOK_PATH}"
    echo "✅ Installed .git/hooks/pre-commit to run 'just commit-hook'"

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
    curl -s http://localhost:8001/v1/models | python3 -m json.tool

# Run opencode one-shot against local qwen36-local/ch0nky
qwen36-test-opencode prompt="say hello in 3 words":
    opencode run --model qwen36-local/ch0nky "{{prompt}}"

# ── Worker agent — A/B experiment dispatch + phygital ontology ──────────────

# Run an A/B experiment: two sub-agents, parallel dispatch, stateless scoring
test-schema-drift:
    cargo test -p b00t-cli --lib -- datum_schema::tests::test_focus_schema_file_matches_generated

# ── ledgrrr — ledgerr-mcp lifecycle (just module) ─────────────────────────
# 🦨 Symlink: vendor/ledgrrr -> vendor/l3dg3rr (polyseme mapping)
# Module docs: https://just.systems/man/en/modules.html
# Invocation:  just ledgrrr build | docker-build | docker-run | docker-stop | …
mod ledgrrr 'vendor/ledgrrr/ledgrrr.just'

# ── pi agent — systemd service lifecycle ─────────────────────────────────────
# 🤓 pi is managed as b00t@pi-agent.service, NOT spawned per-invocation
opencode-task task="hello":
    opencode run --model gemma4-local/ch0nky "{{task}}"

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
