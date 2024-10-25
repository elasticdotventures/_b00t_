# justfile for Rust Development Environment
# Alias to get the Git repository root
repo-root := `git rev-parse --show-toplevel`

install:
    echo "🥾 _b00t_ install"
    ## TODO: someday.
    # cd {{repo-root}} && ./_b00t_.sh setup
    sudo apt update
    sudo apt install -y fzf bat moreutils fd-find bc jq
    ln -sf /usr/bin/batcat ~/.local/bin/bat
    # 🦨 TODO setup.sh .. but first isolate python, rust, js
    # 🦨 TODO replace crudini with toml-cli
    rye install crudini
    rye install python-dotenv[cli]
    cargo install toml-cli
    cargo install dotenvy
    sudo wget https://github.com/mikefarah/yq/releases/latest/download/yq_linux_amd64 -O /usr/bin/yq && sudo chmod +x /usr/bin/yq
    curl https://zyedidia.github.io/eget.sh | sh
    sudo mv -v eget /usr/local/bin/
    eget BurntSushi/ripgrep
    sudo mv -v ripgrep /usr/local/bin/

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

