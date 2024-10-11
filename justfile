# justfile for Rust Development Environment
# Alias to get the Git repository root
repo-root := `git rev-parse --show-toplevel`

dotenv-load:
    dotenv -f .env

# Command to check/install Rust Analyzer
ra_install:
    ```bash
    if ! command -v rust-analyzer > /dev/null; then
        echo "Rust Analyzer not found. Installing..."
        cargo install rust-analyzer --locked
    else
        echo "Rust Analyzer is already installed."
    fi
    ```

# Run Rust Analyzer in current directory
ra_run:
    rust-analyzer .

# Run tests in the current directory
test:
    cargo test -- --nocapture

O# trigger & run any action ci/action locally
# don't specify workflow or job then script will display ./github/workflows using fzf
gh-action workflow="" job="":
    cd {{repo-root}} && ./just-run-gh-action.sh {{workflow}} {{job}}

watch-gh-action workflow="" job="":
    # Check if cargo-watch is installed; install it quietly if not
    export PATH="$HOME/.cargo/bin:$PATH"        
    command -v cargo-watch >/dev/null 2>&1 || cargo install cargo-watch --quiet
    cargo watch -s "./just-run-gh-action.sh {{workflow}} {{job}}"

