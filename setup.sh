#!/bin/bash
set -ex # Exit immediately if a command exits with a non-zero status, and print commands and their arguments as they are executed.

# Detect if running in a CI environment or Docker
IS_CI="${CI:-false}"
echo "IS_CI: ${IS_CI}"

IS_DOCKER="${IS_DOCKER_BUILD:-false}"
echo "IS_DOCKER: ${IS_DOCKER}"

# Auto-detect Docker if not explicitly set
if [ "$IS_DOCKER" != true ]; then
  if [ -f "/.dockerenv" ] || grep -qaE '(docker|containerd|kubepods)' /proc/1/cgroup 2>/dev/null; then
    IS_DOCKER=true
    echo "IS_DOCKER: ${IS_DOCKER} (auto-detected)"
  fi
fi


# Privilege detection
IS_ROOT=false
if [ "$(id -u)" -eq 0 ]; then
  IS_ROOT=true
fi
CAN_SUDO=false
if command -v sudo >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
  CAN_SUDO=true
fi

# Function to run a command with privilege awareness
run_cmd() {
  local cmd="$1"
  shift || true

  # Special handling for privileged package manager commands
  if [ "$cmd" = "apt" ] || [ "$cmd" = "apt-get" ]; then
    if [ "$IS_ROOT" = true ]; then
      "$cmd" "$@"
    elif [ "$CAN_SUDO" = true ]; then
      sudo "$cmd" "$@"
    else
      echo "Skipping privileged command (no root/sudo): $cmd $*"
      return 0
    fi
    return 0
  fi

  # Non-privileged or generic commands
  if [ "$IS_ROOT" = true ]; then
    "$cmd" "$@"
  elif [ "$CAN_SUDO" = true ]; then
    sudo "$cmd" "$@"
  else
    "$cmd" "$@"
  fi
}

# Interactivity and prompt helpers
INTERACTIVE="${INTERACTIVE:-true}"
if [ "$IS_CI" = true ] || [ ! -t 0 ]; then
  INTERACTIVE=false
fi
# Determine ASSUME_YES default based on environment, unless explicitly set by user
if [ -z "${ASSUME_YES+x}" ]; then
  if [ "$IS_CI" = true ] || [ "$IS_DOCKER" = true ]; then
    ASSUME_YES=true
  else
    ASSUME_YES=false
  fi
fi

prompt_yes_no() {
  # usage: prompt_yes_no "Question?" "default[y|n]"
  local prompt="$1"
  local default="${2:-y}"
  local answer
  if [ "$ASSUME_YES" = true ] && [ "$INTERACTIVE" = true ]; then
    echo ">> $prompt [auto-yes]"
    return 0
  fi
  if [ "$INTERACTIVE" != true ]; then
    echo ">> $prompt [non-interactive default: $default]"
    [ "$default" = "y" ] && return 0 || return 1
  fi
  local suffix="[y/N]"
  [ "$default" = "y" ] && suffix="[Y/n]"
  while true; do
    read -r -p "$prompt $suffix " answer
    answer="${answer:-$default}"
    case "$answer" in
      [Yy]*) return 0 ;;
      [Nn]*) return 1 ;;
      *) echo "Please answer y or n." ;;
    esac
  done
}

# Parse truthy/falsey env values into "true"/"false"
parse_bool() {
  case "$1" in
    1|y|Y|yes|YES|true|TRUE) echo true ;;
    0|n|N|no|NO|false|FALSE) echo false ;;
    *) echo "" ;;
  esac
}

# Decide a boolean with environment override, interactive prompt, and non-interactive default
# usage: decide_bool VAR_NAME "Question?" default_when_interactive[y|n] default_when_noninteractive[y|n]
decide_bool() {
  local var_name="$1"; shift
  local question="$1"; shift
  local def_int="$1"; shift
  local def_non="$1"; shift

  # If explicitly set in env, honor it
  local env_val="${!var_name}"
  local parsed
  parsed="$(parse_bool "$env_val")"
  if [ -n "$parsed" ]; then
    [ "$parsed" = true ] && echo true || echo false
    return 0
  fi

  # Interactive flow
  if [ "$INTERACTIVE" = true ]; then
    if prompt_yes_no "$question" "$def_int"; then
      echo true
    else
      echo false
    fi
    return 0
  fi

  # Non-interactive default
  if [ "$def_non" = "y" ]; then
    echo true
  else
    echo false
  fi
}

# Architecture/Platform detection (used for asset URLs and packages)
ARCH="$(uname -m)"
OS="$(uname -s)"
# Defaults
YQ_ASSET="yq_linux_amd64.tar.gz"
YQ_BIN_NAME="yq_linux_amd64"
WASM2OCI_NAME="linux-amd64-wasm-to-oci"
AWSCLI_ZIP="awscli-exe-linux-x86_64.zip"
CARGO_BINSTALL_TGZ="cargo-binstall-x86_64-unknown-linux-gnu.tgz"
RUSTUP_TRIPLE="x86_64-unknown-linux-gnu"

case "$ARCH" in
  x86_64|amd64)
    YQ_ASSET="yq_linux_amd64.tar.gz"
    YQ_BIN_NAME="yq_linux_amd64"
    WASM2OCI_NAME="linux-amd64-wasm-to-oci"
    AWSCLI_ZIP="awscli-exe-linux-x86_64.zip"
    CARGO_BINSTALL_TGZ="cargo-binstall-x86_64-unknown-linux-gnu.tgz"
    RUSTUP_TRIPLE="x86_64-unknown-linux-gnu"
    ;;
  aarch64|arm64)
    YQ_ASSET="yq_linux_arm64.tar.gz"
    YQ_BIN_NAME="yq_linux_arm64"
    WASM2OCI_NAME="linux-arm64-wasm-to-oci"
    AWSCLI_ZIP="awscli-exe-linux-aarch64.zip"
    CARGO_BINSTALL_TGZ="cargo-binstall-aarch64-unknown-linux-gnu.tgz"
    RUSTUP_TRIPLE="aarch64-unknown-linux-gnu"
    ;;
  *)
    # Fallback to amd64 assets
    YQ_ASSET="yq_linux_amd64.tar.gz"
    YQ_BIN_NAME="yq_linux_amd64"
    WASM2OCI_NAME="linux-amd64-wasm-to-oci"
    AWSCLI_ZIP="awscli-exe-linux-x86_64.zip"
    CARGO_BINSTALL_TGZ="cargo-binstall-x86_64-unknown-linux-gnu.tgz"
    RUSTUP_TRIPLE="x86_64-unknown-linux-gnu"
    ;;
esac

# Ensure local bin on PATH for non-privileged installs
mkdir -p "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"

# Ensure local bin on PATH for non-privileged installs
mkdir -p "$HOME/.local/bin"
export PATH="$HOME/.local/bin:$PATH"

# Heavy step default: skip in CI or when no sudo/root unless explicitly overridden
if [ -z "${SKIP_HEAVY+x}" ]; then
  if [ "$IS_CI" = true ] || [ "$CAN_SUDO" = false ]; then
    SKIP_HEAVY=true
  else
    SKIP_HEAVY=false
  fi
fi

# DRY_RUN: print decisions and exit before making changes
if [ "${DRY_RUN:-false}" = true ]; then
  INSTALL_GH="$(decide_bool INSTALL_GH "Install GitHub CLI (gh)?" "n" "n")"
  INSTALL_DOTFILES="$(decide_bool INSTALL_DOTFILES "Setup dotfiles from elasticdotventures/dotfiles?" "n" "y")"
  USE_STOW="$(decide_bool USE_STOW "Link dotfiles using GNU stow?" "y" "y")"
  echo "DRY_RUN decisions:"
  echo "  IS_CI=${IS_CI} IS_DOCKER=${IS_DOCKER} INTERACTIVE=${INTERACTIVE} ASSUME_YES=${ASSUME_YES}"
  echo "  INSTALL_GH=${INSTALL_GH}"
  echo "  INSTALL_DOTFILES=${INSTALL_DOTFILES}"
  echo "  USE_STOW=${USE_STOW}"
  exit 0
fi

# 🥾 migrated from promptexecution/infrastructure
# this is the *minimal* setup for _b00t_ on bash
# the first few lines can be cut and paste onto a fresh server, then run ~/.dotfiles/setup.sh

# 🤓 my goal is to make this idempotent, so it will both install+upgrade (**someday)

run_cmd apt update && run_cmd apt upgrade -y
run_cmd apt install software-properties-common -y

# Decide GitHub CLI (gh) installation
# Defaults per Docker/CI preference: non-interactive => INSTALL_GH=false
INSTALL_GH="$(decide_bool INSTALL_GH "Install GitHub CLI (gh)?" "n" "n")"
if [ "$INSTALL_GH" = true ] && ! command -v gh >/dev/null 2>&1; then
  ## github cli (ubuntu)
  # 🤓 https://github.com/cli/cli/blob/trunk/docs/install_linux.md
  type -p curl >/dev/null || (run_cmd apt update && run_cmd apt install curl -y)
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | run_cmd dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
  && run_cmd chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
  && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | run_cmd tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
  && run_cmd apt update \
  && run_cmd apt install git gh -y
fi

# Optional GitHub authentication and extensions
if command -v gh >/dev/null 2>&1; then
  if [ -f /run/secrets/GH_TOKEN ]; then
    if prompt_yes_no "Use /run/secrets/GH_TOKEN to authenticate gh?" "y"; then
      echo "attempting gh auth login"
      gh auth login --with-token < /run/secrets/GH_TOKEN || true
    fi
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

  if check_github_auth && prompt_yes_no "Install gh extensions (gh-act, gh-copilot)?" "n"; then
    gh extension install https://github.com/nektos/gh-act || true
    gh extension install github/gh-copilot --force || true
  fi
fi


# Dotfiles setup (optional)
# Defaults per Docker/CI preference: non-interactive => INSTALL_DOTFILES=true, USE_STOW=true
INSTALL_DOTFILES="$(decide_bool INSTALL_DOTFILES "Setup dotfiles from elasticdotventures/dotfiles?" "n" "y")"
if [ "$INSTALL_DOTFILES" = true ]; then
  DOTFILES_DIR="$HOME/.dotfiles"
  if [ ! -d "$DOTFILES_DIR" ]; then
    if command -v gh >/dev/null 2>&1; then
      gh repo clone elasticdotventures/dotfiles "$DOTFILES_DIR" -- --depth 1 || git clone --depth 1 https://github.com/elasticdotventures/dotfiles.git "$DOTFILES_DIR"
    else
      git clone --depth 1 https://github.com/elasticdotventures/dotfiles.git "$DOTFILES_DIR"
    fi
  fi

  USE_STOW="$(decide_bool USE_STOW "Link dotfiles using GNU stow?" "y" "y")"
  if [ "$USE_STOW" = true ]; then
    if ! command -v stow >/dev/null 2>&1; then
      # In non-interactive Docker/CI flows, install automatically
      run_cmd apt install -y stow
    fi
    if command -v stow >/dev/null 2>&1; then
      stow -d "$DOTFILES_DIR" -t "$HOME" bash
    else
      echo "GNU stow unavailable — applying minimal symlink fallback for common bash files"
      for f in .bashrc .bash_profile .bash_aliases; do
        if [ -f "$DOTFILES_DIR/bash/$f" ]; then
          ln -sf "$DOTFILES_DIR/bash/$f" "$HOME/$f"
        fi
      done
    fi
  else
    echo "USE_STOW=false — applying minimal symlink fallback for common bash files"
    for f in .bashrc .bash_profile .bash_aliases; do
      if [ -f "$DOTFILES_DIR/bash/$f" ]; then
        ln -sf "$DOTFILES_DIR/bash/$f" "$HOME/$f"
      fi
    done
  fi
fi

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
  if [ "$IS_ROOT" = true ] || [ "$CAN_SUDO" = true ]; then
    run_cmd ntpdate pool.ntp.org
  else
    echo "Skipping ntpdate (no root/sudo)"
  fi
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


# 🦀 rust (robust, conflict-safe)
install_rustup() {
  # If cargo exists in ~/.cargo/bin without rustup, back it up to avoid 'bin/cargo' conflict
  if [ ! -x "$HOME/.cargo/bin/rustup" ] && [ -e "$HOME/.cargo/bin/cargo" ]; then
    DO_BACKUP=true
    if [ "$INTERACTIVE" = true ]; then
      if prompt_yes_no "Found $HOME/.cargo/bin/cargo without rustup. Backup to cargo.bak and continue?" "y"; then
        DO_BACKUP=true
      else
        DO_BACKUP=false
      fi
    fi
    if [ "$DO_BACKUP" = true ]; then
      ts="$(date +%s)"
      mv "$HOME/.cargo/bin/cargo" "$HOME/.cargo/bin/cargo.bak.$ts" || true
      echo "Backed up existing cargo -> $HOME/.cargo/bin/cargo.bak.$ts"
    fi
  fi

  # Install rustup with minimal profile; do not modify PATH (we source env below)
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path

  # Source cargo env if present
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
}

# Ensure stable toolchain present; remediate common cargo/rustc conflicts automatically
ensure_rust_stable() {
  set +e
  local list_out
  list_out="$(rustup toolchain list 2>/dev/null || true)"
  if ! echo "$list_out" | grep -q '^stable'; then
    local install_out
    install_out="$(rustup toolchain install stable --profile minimal 2>&1)"; local rc=$?
    if [ $rc -ne 0 ]; then
      echo "$install_out" >&2
      if echo "$install_out" | grep -q "detected conflict: 'bin/cargo'"; then
        echo "Detected cargo binary conflict; applying remediation..."
        ts="$(date +%s)"
        for b in cargo rustc; do
          if [ -e "$HOME/.cargo/bin/$b" ]; then
            mv "$HOME/.cargo/bin/$b" "$HOME/.cargo/bin/$b.bak.$ts" || true
          fi
        done
        rustup toolchain uninstall stable || true
        rustup toolchain install stable --profile minimal --force
      else
        set -e
        return $rc
      fi
    fi
  else
    rustup update stable || true
  fi
  rustup default stable || true
  set -e
}

if command -v rustup >/dev/null 2>&1; then
  rustup self update || true
else
  install_rustup
fi
# Ensure cargo env is loaded (some shells don't inherit PATH changes)
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
ensure_rust_stable


#curl -sS https://starship.rs/install.sh | sh
run_cmd apt-get update
run_cmd apt install -y cmake pkg-config
if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping starship install (SKIP_HEAVY=true)"
else
  cargo install starship --locked
  if [ -f ~/.bashrc ] && ! grep -q "starship init bash" ~/.bashrc; then
      echo 'eval "$(starship init bash)"' >> ~/.bashrc
  fi
fi

# dotenv
if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping Bun install and dotenv package (SKIP_HEAVY=true)"
else
  if ! command -v bun &> /dev/null; then
    curl -fsSL https://bun.sh/install | bash # install Bun
    export PATH="$HOME/.bun/bin:$PATH"
  fi
  if command -v bun &> /dev/null; then
      bun add dotenv
  fi
fi

# dotenvy
if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping dotenvy CLI install (SKIP_HEAVY=true)"
else
  cargo install dotenvy --bin dotenvy --features cli
fi

# tree but ignores .git (useful for chatgpt dumps)
if ! command -v rg &> /dev/null; then
  run_cmd apt-get install -y ripgrep
fi
alias itree='rg --files | tree --fromfile'

# just is a command runner
if ! command -v just &> /dev/null; then
  if [ "$IS_ROOT" = true ] || [ "$CAN_SUDO" = true ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | run_cmd bash -s -- --to /usr/local/bin
  else
    curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | bash -s -- --to "$HOME/.local/bin"
  fi
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

~/.local/bin/eget mikefarah/yq --upgrade-only --tag v4.44.1 --asset "${YQ_ASSET}" --all
mkdir -p ~/.local/bin
mv "${YQ_BIN_NAME}" ~/.local/bin/yq


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
#if ! command -v wasm-to-oci &> /dev/null; then
#  wget "https://github.com/engineerd/wasm-to-oci/releases/download/v0.1.2/${WASM2OCI_NAME}"
#  mv "${WASM2OCI_NAME}" wasm-to-oci
#  chmod +x wasm-to-oci
#  if [ "$IS_ROOT" = true ] || [ "$CAN_SUDO" = true ]; then
#    run_cmd cp wasm-to-oci /usr/local/bin
#  else
#    mv wasm-to-oci "$HOME/.local/bin/wasm-to-oci"
#  fi
#fi

# azure
alias az="docker run -it -v ${HOME}/.ssh:/root/.ssh mcr.microsoft.com/azure-cli"

# aws
if ! command -v aws &> /dev/null; then
  if [ "$IS_ROOT" = true ] || [ "$CAN_SUDO" = true ]; then
    cd /tmp
    curl "https://awscli.amazonaws.com/${AWSCLI_ZIP}" -o "awscliv2.zip"
    unzip -q awscliv2.zip
    run_cmd ./aws/install
    cd -
  else
    echo "Skipping AWS CLI install (no root/sudo)"
  fi
fi

# gcloud
if ! command -v gcloud &> /dev/null; then
  if [ "$IS_ROOT" = true ] || [ "$CAN_SUDO" = true ]; then
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
  else
    echo "Skipping Google Cloud CLI install (no root/sudo)"
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
#if [ "$IS_CI" = false ] && ! kubectl krew > /dev/null 2>&1; then
#(
#  set -x; cd "$(mktemp -d)" &&
#  OS="$(uname | tr '[:upper:]' '[:lower:]')" &&
#  ARCH="$(uname -m | sed -e 's/x86_64/amd64/' -e 's/\\(arm\\)\\(64\\)\\?.*/\\1\\2/')" &&
#  KREW="krew-${OS}_${ARCH}" &&
#  curl -fsSLO "https://github.com/kubernetes-sigs/krew/releases/latest/download/${KREW}.tar.gz" &&
#  tar zxvf "${KREW}.tar.gz" &&
#  ./"${KREW}" install krew
#)
#fi

# inotify-tools
if ! command -v inotifywait &> /dev/null; then
  run_cmd apt-get install -y inotify-tools
fi

if ! command -v uv &> /dev/null; then
  # On macOS and Linux. .. now perhaps i think uv > rye
  curl -LsSf https://astral.sh/uv/install.sh | sh
fi

# datafusion
if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping DataFusion CLI install (SKIP_HEAVY=true)"
else
  if ! command -v datafusion-cli &> /dev/null; then
    cargo install datafusion-cli
  fi
fi

# https://github.com/bodo-run/yek
if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping yek install (SKIP_HEAVY=true)"
else
  cargo install --git https://github.com/bodo-run/yek
fi

# Direnv
if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping direnv install (SKIP_HEAVY=true)"
else
  if ! command -v direnv &> /dev/null; then
      curl -sfL https://direnv.net/install.sh | bash
  fi
fi

if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping pixi install (SKIP_HEAVY=true)"
else
  if ! command -v pixi &> /dev/null; then
      curl -fsSL https://pixi.sh/install.sh | bash
  fi
fi

if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping cargo-binstall and podlet install (SKIP_HEAVY=true)"
else
  if ! command -v cargo-binstall &> /dev/null; then
    curl -LsSf "https://github.com/cargo-bins/cargo-binstall/releases/latest/download/${CARGO_BINSTALL_TGZ}" | tar -xz -C ~/.cargo/bin
  fi
  cargo binstall podlet
fi

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

#!uv tool install huggingface_hub[cli]
#!uv tool install ramalama

# procfile apps, https://honcho.readthedocs.io/en/latest/
#!uvx honcho version

# https://github.com/aristocratos/bpytop
#!uvx bpytop -v

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

if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping tmate/entr/nmap installs (SKIP_HEAVY=true)"
else
  # https://tmate.io/
  # tmate is a fork of tmux. tmate and tmux can coexist on the same system.
  run_cmd apt-get -y install tmate

  # Run arbitrary commands when files change
  run_cmd apt-get install -y entr

  # we use nmap to see if ports are open
  run_cmd apt-get install -y nmap
fi

if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping probe and multiplex installs (SKIP_HEAVY=true)"
else
  # https://github.com/buger/probe
  if command -v bun &> /dev/null; then
    bun install -g @buger/probe
  fi
  uv tool install multiplex-sh
  multiplex --help || true
fi


# ClaudeCode Plugins https://github.com/brennercruvinel/CCPlugins
# https://www.reddit.com/r/ClaudeAI/comments/1mb37uj/found_claude_code_plugins_that_actually_work/?share_id=TG81kGPK8f9feDN_7ymoC&utm_content=1&utm_medium=android_app&utm_name=androidcss&utm_source=share&utm_term=1
if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping CCPlugins install (SKIP_HEAVY=true)"
else
  curl -sSL https://raw.githubusercontent.com/brennercruvinel/CCPlugins/main/install.sh | bash
fi

# TODO: https://github.com/mcp-use
# https://github.com/awslabs/mcp
# https://github.com/browser-use/browser-use


# TILT:
# https://github.com/aryan-agrawal-glean/tilt-mcp

if [ "${SKIP_HEAVY:-false}" = true ]; then
  echo "Skipping OpenCode AI install (SKIP_HEAVY=true)"
else
  # Install OpenCode AI (prefer user-local install when no sudo/root)
  if command -v npm >/dev/null 2>&1; then
    if [ "$IS_ROOT" = true ] || [ "$CAN_SUDO" = true ]; then
      npm i -g opencode-ai@latest
    else
      mkdir -p "$HOME/.local"
      npm i -g --prefix "$HOME/.local" opencode-ai@latest
      export PATH="$HOME/.local/bin:$PATH"
      if [ -f "$HOME/.bashrc" ] && ! grep -q 'export PATH="$HOME/.local/bin:$PATH"' "$HOME/.bashrc"; then
        echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$HOME/.bashrc"
      fi
    fi
  fi
fi

run_cmd apt install -y zip

