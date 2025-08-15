# justfile for Rust Development Environment
# Alias to get the Git repository root
repo-root := env_var_or_default("JUST_REPO_ROOT", `git rev-parse --show-toplevel 2>/dev/null || echo .`)



set shell := ["bash", "-cu"]
mod cog
mod b00t
# this is an antipattens
mod litellm '_b00t_/litellm/justfile'

stow:
    stow --adopt -d ~/.dotfiles -t ~ bash

release:
    gh release create v1.1.0 --title "Release v1.1.0" --notes "Release notes for version 1.1.0"

    # check for latest release tag of _b00t_ in github using gh cli
    NET_VERSION=$(cd "$HOME/.dotfiles" && gh release view -R elasticdotventures/dotfiles --json tagName | jq -r .tagName)

    # compare to local release
    OUR_VERSION=$(cd "$HOME/.dotfiles" && git tag -l | sort -V | tail -n 1)


install:
    echo "🥾 _b00t_ install"
    cargo install --path b00t-mcp
    cargo install --path b00t-cli

installx:
    sudo apt update
    ## TODO: someday.
    # cd {{repo-root}} && ./_b00t_.sh setup
    sudo apt install -y fzf bat moreutils fd-find bc jq python3-argcomplete
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
    git describe --tags --abbrev=0

cliff:
    # git-cliff --tag $(git describe --tags --abbrev=0) -o CHANGELOG.md
    git-cliff -o CHANGELOG.md



inspect-mcp:
	npx @modelcontextprotocol/inspector ./target/release/b00t-mcp

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
    docker run -d -p 1080:1080   -v ./koblas.toml:/etc/koblas/config.toml   -e RUST_LOG=debug   -e KOBLAS_NO_AUTHENTICATION=true   -e KOBLAS_ANONYMIZE=false   --name koblas docker.io/ynuwenhof/koblas:latest

