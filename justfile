# justfile for Rust Development Environment
# Alias to get the Git repository root
repo-root := env_var_or_default("JUST_REPO_ROOT", `git rev-parse --show-toplevel 2>/dev/null || echo .`)

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
    ## TODO: someday.
    # cd {{repo-root}} && ./_b00t_.sh setup
    sudo apt update
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


