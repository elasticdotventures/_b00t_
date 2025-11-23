# justfile for Rust Development Environment
# Alias to get the Git repository root
repo-root := env_var_or_default("JUST_REPO_ROOT", `git rev-parse --show-toplevel 2>/dev/null || echo .`)
workspace_version := `toml get Cargo.toml workspace.package.version | tr -d '"'`



set shell := ["bash", "-cu"]
mod cog
mod b00t
# this is an antipattern (litellm is early-stage AI infra, skip for now)
mod litellm '_b00t_/litellm/justfile'
mod b00t-mcp-npm

# Datum justfiles (install recipes for core tech stacks)
mod python '_b00t_/python.🐍/justfile'
mod docker '_b00t_/docker.🐳/justfile'
mod bash '_b00t_/bash.🐚/justfile'
mod git '_b00t_/git.🐙/justfile'
mod terraform '_b00t_/terraform.🧊/justfile'
mod k8s '_b00t_/k8s.🚢/justfile'
mod pm2-tasker 'pm2-tasker/justfile'
mod embed '_b00t_/python.🐍/embed/justfile'

stow:
    stow --adopt -d ~/.dotfiles -t ~ bash

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
    VERSION="{{workspace_version}}"

    echo "🚀 Creating release v${VERSION}..."

    # Verify workspace is clean
    if ! git diff --quiet; then
        echo "⚠️ Uncommitted changes detected"
        exit 1
    fi

    # Run tests first
    echo "🧪 Running tests..."
    cargo test --workspace --all-features

    # Create git tag
    git tag -a "v${VERSION}" -m "Release v${VERSION}"
    git push origin "v${VERSION}"

    # Create GitHub release (triggers publish-crates.yml workflow)
    gh release create "v${VERSION}" \
        --title "Release v${VERSION}" \
        --generate-notes

    echo "✅ Release v${VERSION} created"
    echo "📦 Crates will be published to crates.io by GitHub Actions"
    echo "🔗 Check workflow: https://github.com/elasticdotventures/dotfiles/actions"


install:
    echo "🥾 _b00t_ install"
    cargo install --path b00t-mcp --force
    cargo install --path b00t-cli --force
    cargo install cocogitto --locked --force
    just install-commit-hook


installx:
    sudo apt update
    ## TODO: someday.
    # cd {{repo-root}} && ./_b00t_.sh setup
    sudo apt install -y fzf bat moreutils fd-find bc jq python3-argcomplete curl
    ln -sf /usr/bin/batcat ~/.local/bin/bat
    # 🦨 TODO setup.sh .. but first isolate python, rust, js
    # 🦨 TODO replace crudini with toml-cli
    command -v dotenv >/dev/null 2>&1 || uv tool install python-dotenv[cli]
    # toml-cli binary is just 'toml'
    export PATH="$HOME/.cargo/bin:$PATH" || command -v toml >/dev/null 2>&1 || cargo install toml-cli
    command -v dotenvy >/dev/null 2>&1 || cargo install dotenvy --features cli
    #command -v yq >/dev/null 2>&1 || sudo wget https://github.com/mikefarah/yq/releases/latest/download/yq_linux_amd64 -O /usr/bin/yq && sudo chmod +x /usr/bin/yq
    command -v eget >/dev/null 2>&1 || (curl https://zyedidia.github.io/eget.sh | sh && sudo mv -v eget /usr/local/bin/)
    command -v rg >/dev/null 2>&1 || (eget BurntSushi/ripgrep && sudo mv -v rg /usr/local/bin/)
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
    echo "{{workspace_version}}"

commit-hook:
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
	if ! command -v huggingface-cli >/dev/null 2>&1; then
		echo "⚠️ huggingface-cli missing; run 'b00t-cli cli install huggingface'" >&2
		exit 1
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
	huggingface-cli "${ARGS[@]}"
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

# Captain's Command Arsenal - Memoized Agent Operations

# Role switching commands
captain:
    #!/bin/bash
    export _B00T_ROLE="captain"
    echo "🎯 Switched to Captain role"
    cargo run --bin b00t-cli -- whatismy role --show-tools

operator:
    #!/bin/bash
    export _B00T_ROLE="operator"
    echo "⚙️ Switched to Operator role"
    cargo run --bin b00t-cli -- whatismy role --show-tools

# Agent creation commands (for future operator use)
create-coder LANG:
    #!/bin/bash
    echo "🛠️ Creating {{LANG}} coder agent..."
    echo "TODO: Implement agent creation via operator"

create-tester:
    #!/bin/bash
    echo "🧪 Creating test specialist agent..."
    echo "TODO: Implement test agent creation"

# Communication setup
setup-redis:
    #!/bin/bash
    echo "💾 Setting up Redis pub/sub for agent communication..."
    echo "TODO: Implement Redis agent channels"

# Session management
session-status:
    #!/bin/bash
    cargo run --bin b00t-cli -- whatismy status

session-build:
    #!/bin/bash
    cargo run --bin b00t-cli -- session build

# Tool installation (for operators)
install-tool TOOL:
    #!/bin/bash
    echo "📦 Installing tool: {{TOOL}}"
    echo "TODO: Implement tool installation via b00t cli"

# Qdrant vector database
qdrant-run:
    podman run -d --name qdrant-container -p 6333:6333 -p 6334:6334 -e QDRANT__SERVICE__GRPC_PORT="6334" docker.io/qdrant/qdrant:latest

qdrant-stop:
    podman stop qdrant-container && podman rm qdrant-container

# 🤓 PyO3/Maturin build commands for b00t-grok-py
grok-build:
    #!/bin/bash
    # 🤓 Critical: unset CONDA_PREFIX to avoid environment conflicts with uv
    # This prevents "Both VIRTUAL_ENV and CONDA_PREFIX are set" error
    echo "🦀🐍 Building b00t-grok with PyO3 bindings..."
    unset CONDA_PREFIX
    cd b00t-grok-py
    uv run maturin develop

grok-dev: grok-build
    #!/bin/bash
    echo "🚀 Starting b00t-grok-py development server..."
    cd b00t-grok-py
    unset CONDA_PREFIX
    uv run python -m uvicorn main:app --reload --port 8001

grok-clean:
    #!/bin/bash
    echo "🧹 Cleaning b00t-grok build artifacts..."
    cargo clean --package b00t-grok
    cd b00t-grok-py && rm -rf build/ dist/ *.egg-info/

# Validate MCP TOML files against schema
validate-mcp:
    #!/bin/bash
    echo "🔍 Validating MCP TOML files..."
    cd {{repo-root}}/_b00t_
    taplo lint --schema file://$PWD/schema-资源/mcp.json *.mcp.toml

# Build and package b00t browser extension
browser-ext-build:
    #!/bin/bash
    echo "🥾 Building b00t browser extension..."
    cd {{repo-root}}/b00t-browser-ext
    npm ci
    npm run build
    echo "✅ Extension built in build/chrome-mv3-prod/"

browser-ext-package:
    #!/bin/bash
    echo "📦 Packaging b00t browser extension..."
    cd {{repo-root}}/b00t-browser-ext
    npm run package
    VERSION=$(node -p "require('./package.json').version")
    echo "✅ Extension packaged as b00t-browser-ext-chrome-v${VERSION}.zip"

browser-ext-dev:
    #!/bin/bash
    echo "🚀 Starting b00t browser extension dev server..."
    cd {{repo-root}}/b00t-browser-ext
    npm run dev

socks5:
    {{repo-root}}/scripts/socks5.sh

port-map:
    {{repo-root}}/scripts/port-map.sh

install-services:
    {{repo-root}}/scripts/install-systemd-services.sh
