#!/bin/bash
set -ex # Exit immediately if a command exits with a non-zero status, and print commands and their arguments as they are executed.

# Detect if running in a CI environment or Docker
IS_CI="${CI:-false}"
echo "IS_CI: ${IS_CI}"

IS_DOCKER="${IS_DOCKER_BUILD:-false}"
echo "IS_DOCKER: ${IS_DOCKER}"


# Function to run a command, adding sudo if not in Docker
run_cmd() {
    if [ "$IS_DOCKER" = true ]; then
        "$@"
    else
        sudo "$@"
    fi
}

# 🥾 migrated from promptexecution/infrastructure
# this is the *minimal* setup for _b00t_ on bash
# the first few lines can be cut and paste onto a fresh server, then run ~/.dotfiles/setup.sh

# 🤓 my goal is to make this idempotent, so it will both install+upgrade (**someday)

run_cmd apt update && run_cmd apt upgrade -y
run_cmd apt install software-properties-common -y

if ! command -v gh; then
  ## github cli (ubuntu)
  # 🤓 https://github.com/cli/cli/blob/trunk/docs/install_linux.md
  type -p curl >/dev/null || (run_cmd apt update && run_cmd apt install curl -y)
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | run_cmd dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
  && run_cmd chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
  && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | run_cmd tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
  && run_cmd apt update \
  && run_cmd apt install git gh -y
fi

if [ -f /run/secrets/GH_TOKEN ]; then
    echo "attempting gh auth login"
    gh auth login --with-token < /run/secrets/GH_TOKEN
    
fi


check_github_auth() {
  if gh auth status 2>&1 | grep -q '✓ Logged in to github.com'; then
    echo "✅ Auth OK"
    return 0
  else
    echo "❌ Not authenticated"
    return 1
  fi
}

if check_github_auth; then
  echo "✅ GitHub auth OK — proceeding..."
  # your logic here
  ## setup extensions now.
  # 🤓
  gh extension install https://github.com/nektos/gh-act
  # 🤓 https://github.com/github/gh-copilot
  gh extension install github/gh-copilot --force
else
  echo "❌ GitHub not authenticated — skipping extension"
fi


# if stow is not installed, install it
if ! command -v stow; then
  run_cmd apt install stow -y
fi

if [ ! -d ~/.dotfiles ]; then
  gh repo clone elasticdotventures/dotfiles ~/.dotfiles -- --depth 1
fi
# stow is idempotent
stow -d ~/.dotfiles -t ~ bash

## now setup dotfiles, and run:
## ~/.dotfiles/setup.sh

run_cmd apt install -y build-essential joe libnotify-bin bc

if [ "$IS_CI" = false ]; then
  ## TODO: config file? -- for now change these settings to yours
  git config --global user.email "brianh@elastic.ventures"
  git config --global user.name "Brian H"
fi

if [ "$IS_DOCKER" = false ]; then
    run_cmd apt install -y ntpdate
    run_cmd ntpdate pool.ntp.org
fi

run_cmd apt update -yy
run_cmd apt-get install -y jq fzf ripgrep tree unzip

if ! command -v tofu; then
  # https://opentofu.org/docs/intro/install/deb
  curl --proto '=https' --tlsv1.2 -fsSL 'https://packages.opentofu.org/install/repositories/opentofu/tofu/script.deb.sh?any=true' -o /tmp/tofu-repository-setup.sh
  # Inspect the downloaded script at /tmp/tofu-repository-setup.sh before running
  run_cmd bash /tmp/tofu-repository-setup.sh
  rm /tmp/tofu-repository-setup.sh
  run_cmd apt-get install -y tofu
fi
alias tf=tofu


# 🦀 rust
if ! command -v rustc &> /dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "$HOME/.cargo/env"
else
  rustup update
fi


#curl -sS https://starship.rs/install.sh | sh
run_cmd apt-get update
run_cmd apt install -y cmake pkg-config
cargo install starship --locked
if [ -f ~/.bashrc ] && ! grep -q "starship init bash" ~/.bashrc; then
    echo 'eval "$(starship init bash)"' >> ~/.bashrc
fi

# dotenv
if ! command -v bun &> /dev/null; then
  curl -fsSL https://bun.sh/install | bash # install Bun
  export PATH="$HOME/.bun/bin:$PATH"
fi
if command -v bun &> /dev/null; then
    bun add dotenv
fi

# dotenvy
cargo install dotenvy --bin dotenvy --features cli

# tree but ignores .git (useful for chatgpt dumps)
if ! command -v rg &> /dev/null; then
  run_cmd apt-get install -y ripgrep
fi
alias itree='rg --files | tree --fromfile'

# just is a command runner
if ! command -v just &> /dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | run_cmd bash -s -- --to /usr/local/bin
fi

# 💩 yq
if [ -f /etc/apt/sources.list.d/rmescandon-ubuntu-yq-noble.sources ]; then
  # the ppa was abandoned, so we can't use it after ubuntu 20.04
  run_cmd rm /etc/apt/sources.list.d/rmescandon-ubuntu-yq-noble.sources
fi

# eget is a programmatic way to install stuff directly from github
if ! command -v eget &> /dev/null; then
    curl -s https://zyedidia.github.io/eget.sh | sh
    mkdir -p ~/.local/bin
    mv eget ~/.local/bin/eget
    export PATH="$HOME/.local/bin:$PATH"
fi

# alt to eget
# https://github.com/marverix/gah

~/.local/bin/eget mikefarah/yq --upgrade-only --tag v4.44.1 --asset yq_linux_amd64.tar.gz --all
mkdir -p ~/.local/bin
mv yq_linux_amd64 ~/.local/bin/yq


## someday..
# alias yq="podman run --rm -v \"${PWD}\":/workdir docker.io/mikefarah/yq"
# https://kislyuk.github.io/yq/

# ubuntu installs bat as batcat
run_cmd apt install bat -y
mkdir -p ~/.local/bin
if [ ! -e ~/.local/bin/bat ]; then
    ln -s /usr/bin/batcat ~/.local/bin/bat
fi

## DEV workstation
# for pgrx, llvm
 run_cmd apt install -y libclang-dev

# wasm to oci
if ! command -v wasm-to-oci &> /dev/null; then
  wget https://github.com/engineerd/wasm-to-oci/releases/download/v0.1.2/linux-amd64-wasm-to-oci
  mv linux-amd64-wasm-to-oci wasm-to-oci
  chmod +x wasm-to-oci
  run_cmd cp wasm-to-oci /usr/local/bin
fi

# azure
alias az="docker run -it -v ${HOME}/.ssh:/root/.ssh mcr.microsoft.com/azure-cli"

# aws
if ! command -v aws &> /dev/null; then
  cd /tmp
  curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
  unzip -q awscliv2.zip
  run_cmd ./aws/install
  cd -
fi

# gcloud
if ! command -v gcloud &> /dev/null; then
  # https://cloud.google.com/sdk/docs/install#deb
  run_cmd apt-get install apt-transport-https ca-certificates gnupg curl -y
  curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | run_cmd gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg
  echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | run_cmd tee -a /etc/apt/sources.list.d/google-cloud-sdk.list
  run_cmd apt-get update && run_cmd apt-get install google-cloud-cli -y
  if [ "$IS_CI" = false ]; then
    if ! gcloud config configurations list | grep -q 'NAME'; then
        gcloud init
    fi
    gcloud config set compute/zone australia-southeast2-c
  fi
fi

# kubectl command completion
if [ "$IS_CI" = false ] && command -v kubectl &> /dev/null; then
    if [ ! -f "$HOME/.kube/completion.bash.inc" ]; then
      ## If bash-completion is not installed on Linux, install the 'bash-completion' package
      ## via your distribution's package manager.
      ## Load the kubectl completion code for bash into the current shell
      source <(kubectl completion bash)
      ## Write bash completion code to a file and source it from .bash_profile
      mkdir -p "$HOME/.kube"
      kubectl completion bash > "$HOME/.kube/completion.bash.inc"
      printf "\n# kubectl shell completion\nsource '$HOME/.kube/completion.bash.inc'\n" >> $HOME/.bash_profile
      source $HOME/.bash_profile
    fi
fi

## 🤔 .. i might remove this.
# krew kubectl plugin
if [ "$IS_CI" = false ] && ! kubectl krew > /dev/null 2>&1; then
(
  set -x; cd "$(mktemp -d)" &&
  OS="$(uname | tr '[:upper:]' '[:lower:]')" &&
  ARCH="$(uname -m | sed -e 's/x86_64/amd64/' -e 's/\\(arm\\)\\(64\\)\\?.*/\\1\\2/')" &&
  KREW="krew-${OS}_${ARCH}" &&
  curl -fsSLO "https://github.com/kubernetes-sigs/krew/releases/latest/download/${KREW}.tar.gz" &&
  tar zxvf "${KREW}.tar.gz" &&
  ./"${KREW}" install krew
)
fi

# inotify-tools
if ! command -v inotifywait &> /dev/null; then
  run_cmd apt-get install -y inotify-tools
fi

if ! command -v uv &> /dev/null; then
  # On macOS and Linux. .. now perhaps i think uv > rye
  curl -LsSf https://astral.sh/uv/install.sh | sh
fi

# datafusion
if ! command -v datafusion-cli &> /dev/null; then
  cargo install datafusion-cli
fi

# https://github.com/bodo-run/yek
cargo install --git https://github.com/bodo-run/yek

# Direnv
if ! command -v direnv &> /dev/null; then
    curl -sfL https://direnv.net/install.sh | bash
fi

if ! command -v pixi &> /dev/null; then
    curl -fsSL https://pixi.sh/install.sh | bash
fi

if ! command -v cargo-binstall &> /dev/null; then
  curl -LsSf https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-gnu.tgz | tar -xz -C ~/.cargo/bin
fi
cargo binstall podlet

# This section was from a merge conflict. Integrating it now.
if [ "$IS_CI" = false ]; then
    # brew
    if ! command -v brew &> /dev/null; then
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
        if [ -f /home/linuxbrew/.linuxbrew/bin/brew ]; then
            eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
            echo "eval \"$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)\"" >> ~/.bashrc
        fi
    fi

    # then use brew to install asdf
    if command -v brew &> /dev/null && ! command -v asdf &> /dev/null; then
        brew install asdf
    fi

    # then use asdf to install go-lang
    if command -v asdf &> /dev/null && ! asdf list golang &> /dev/null; then
        asdf plugin add golang https://github.com/asdf-community/asdf-golang.git
        asdf install golang 1.24.1
        asdf global golang 1.24.1
    fi
fi

uv tool install huggingface_hub[cli]
uv tool install ramalama

# procfile apps, https://honcho.readthedocs.io/en/latest/
uvx honcho version

# https://github.com/aristocratos/bpytop
uvx bpytop -v

# qsv https://github.com/dathere/qsv
#cargo install qsv --locked --features all_features

# https://github.com/bootandy/dust
#cargo install du-dust

# https://github.com/theryangeary/choose
#cargo install choose

## TODO: keep

# pueue processes a queue of shell commands
# eget nukesor/pueue --asset  pueue-x86_64-unknown-linux-musl --to ~/.local/bin
# cargo install --locked pueue

# dog like tid
# alias dog="docker run -it --rm dog"

# https://tmate.io/
# tmate is a fork of tmux. tmate and tmux can coexist on the same system.
sudo apt-get -y install tmate

# Run arbitrary commands when files change
sudo apt-get install -y entr

# we use nmap to see if ports are open
sudo apt-get install -y nmap

# https://github.com/buger/probe
bun install -g @buger/probe

uv tool install multiplex-sh
multiplex --help


# ClaudeCode Plugins https://github.com/brennercruvinel/CCPlugins
# https://www.reddit.com/r/ClaudeAI/comments/1mb37uj/found_claude_code_plugins_that_actually_work/?share_id=TG81kGPK8f9feDN_7ymoC&utm_content=1&utm_medium=android_app&utm_name=androidcss&utm_source=share&utm_term=1
curl -sSL https://raw.githubusercontent.com/brennercruvinel/CCPlugins/main/install.sh | bash

# TODO: https://github.com/mcp-use
# https://github.com/awslabs/mcp
# https://github.com/browser-use/browser-use

# TILT:
# https://github.com/aryan-agrawal-glean/tilt-mcp


npm i -g opencode-ai@latest
