#!/bin/bash

# 🥾 migrated from promptexecution/infrastructure
# this is the *minimal* setup for _b00t_ on bash
# the first few lines can be cut and paste onto a fresh server, then run ~/.dotfiles/setup.sh

# 🤓 my goal is to make this idempotent, so it will both install+upgrade (**someday)

sudo apt update && sudo apt upgrade -y
sudo apt install software-properties-common -y

if ! command -v gh; then
  ## github cli (ubuntu)
  # 🤓 https://github.com/cli/cli/blob/trunk/docs/install_linux.md
  type -p curl >/dev/null || (sudo apt update && sudo apt install curl -y)
  curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg \
  && sudo chmod go+r /usr/share/keyrings/githubcli-archive-keyring.gpg \
  && echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null \
  && sudo apt update \
  && sudo apt install git gh -y
fi

## setup extensions now.
# 🤓
gh extension install https://github.com/nektos/gh-act
# 🤓 https://github.com/github/gh-copilot
gh extension install github/gh-copilot --force



# if stow is not installed, install it
if ! command -v stow; then
  sudo apt install stow -y
fi

if [ ! -d ~/.dotfiles ]; then
  gh repo clone elasticdotventures/dotfiles ~/.dotfiles -- --depth 1
fi
# stow is idempotent
stow -d ~/.dotfiles -t ~ bash



## now setup dotfiles, and run:
## ~/.dotfiles/setup.sh



sudo apt install -y build-essential joe libnotify-bin bc

## TODO: config file? -- for now change these settings to yours
git config --global user.email "brianh@elastic.ventures"
git config --global user.name "Brian H"


sudo apt install -y ntpdate
sudo ntpdate pool.ntp.org

sudo apt update -yy
sudo apt-get install -y jq fzf ripgrep tree

if ! command -v tofu; then
  # https://opentofu.org/docs/intro/install/deb
  curl --proto '=https' --tlsv1.2 -fsSL 'https://packages.opentofu.org/install/repositories/opentofu/tofu/script.deb.sh?any=true' -o /tmp/tofu-repository-setup.sh
  # Inspect the downloaded script at /tmp/tofu-repository-setup.sh before running
  sudo bash /tmp/tofu-repository-setup.sh
  rm /tmp/tofu-repository-setup.sh
  sudo apt-get install -y tofu
fi
alias tf=tofu


# 🦀 rust
if ! command -v rustc &> /dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  . "$HOME/.cargo/env"
else
  rustup update
fi


#curl -sS https://starship.rs/install.sh | sh
sudo apt-get update
sudo apt install -y cmake pkg-config
cargo install starship --locked
echo eval "$(starship init bash)" >> ~/.bashrc


# dotenvy
cargo install dotenvy --bin dotenvy --features cli


# tree but ignores .git (useful for chatgpt dumps)
if ! command -v rg &> /dev/null; then
  sudo apt-get install -y ripgrep
fi
alias itree='rg --files | tree --fromfile'

# just is a command runner
if ! command -v just &> /dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | sudo bash -s -- --to /usr/local/bin
fi

# 💩 yq
# https://mikefarah.gitbook.io/yq/v/v3.x/
# docker run --rm -v "${PWD}":/workdir mikefarah/yq
## ubuntu noble missing ppa
# https://github.com/mikefarah/yq/issues/2081
# sudo add-apt-repository -y ppa:rmescandon/yq

#sudo apt update
#sudo apt install yq bat -y

# check for file if it exists delete it
if [ -f /etc/apt/sources.list.d/rmescandon-ubuntu-yq-noble.sources ]; then
  # the ppa was abandoned, so we can't use it after ubuntu 20.04
  sudo rm /etc/apt/sources.list.d/rmescandon-ubuntu-yq-noble.sources
fi

# eget is a programmatic way to install stuff directly from github
curl https://zyedidia.github.io/eget.sh | sh
mv eget ~/.local/bin/eget

# alt to eget
# https://github.com/marverix/gah

./eget mikefarah/yq --upgrade-only --tag v4.44.6
mv yq ~/.local/bin/yq


## someday..
# alias yq="podman run --rm -v \"${PWD}\":/workdir docker.io/mikefarah/yq"
# https://kislyuk.github.io/yq/

# ubuntu installs bat as batcat
sudo apt install bat -y
mkdir -p ~/.local/bin
ln -s /usr/bin/batcat ~/.local/bin/bat


## DEV workstation
# for pgrx, llvm
 sudo apt install -y libclang-dev

# wasm to oci
if ! command -v wasm-to-oci &> /dev/null; then
  wget https://github.com/engineerd/wasm-to-oci/releases/download/v0.1.2/linux-amd64-wasm-to-oci
  mv linux-amd64-wasm-to-oci wasm-to-oci
  chmod +x wasm-to-oci
  sudo cp wasm-to-oci /usr/local/bin
fi

# azure
alias az="docker run -it -v ${HOME}/.ssh:/root/.ssh mcr.microsoft.com/azure-cli"

# aws
# sudo apt-get install -y awscli
if ! command -v aws &> /dev/null; then
  cd /tmp
  curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
  unzip awscliv2.zip
  sudo ./aws/install
fi
# aws configure
# OR
# mkdir -p ~/.aws
# copy config & credentials from another server

# gcloud
# https://cloud.google.com/sdk/docs/install#deb
#sudo apt-get install apt-transport-https ca-certificates gnupg curl
#curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg
#echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | sudo tee -a /etc/apt/sources.list.d/google-cloud-sdk.list
#sudo apt-get update && sudo apt-get install google-cloud-cli
#sudo apt-get install google-cloud-cli-gke-gcloud-auth-plugin
#gcloud init

# gcloud
if ! command -v gcloud &> /dev/null; then
  # https://cloud.google.com/sdk/docs/install#deb
  sudo apt-get install apt-transport-https ca-certificates gnupg curl -y
  curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg
  echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | sudo tee -a /etc/apt/sources.list.d/google-cloud-sdk.list
  sudo apt-get update && sudo apt-get install google-cloud-cli -y
  if ! gcloud config configurations list | grep -q 'NAME'; then
    gcloud init
  fi
  gcloud config set compute/zone australia-southeast2-c
fi

# The docker route probably won't work with terraform.
#docker pull gcr.io/google.com/cloudsdktool/google-cloud-cli:latest
#docker run --rm gcr.io/google.com/cloudsdktool/google-cloud-cli:latest gcloud version
#docker run -ti --name gcloud-config gcr.io/google.com/cloudsdktool/google-cloud-cli gcloud auth login
#docker run --rm --volumes-from gcloud-config gcr.io/google.com/cloudsdktool/google-cloud-cli gcloud compute instances list --project your_project



# kubectl command completion



# Installing bash completion on Linux
if [ ! -f "$HOME/.kube/completion.bash.inc" ]; then
  ## If bash-completion is not installed on Linux, install the 'bash-completion' package
  ## via your distribution's package manager.
  ## Load the kubectl completion code for bash into the current shell
  source <(kubectl completion bash)
  ## Write bash completion code to a file and source it from .bash_profile
  kubectl completion bash > "$HOME/.kube/completion.bash.inc"
  printf "
  # kubectl shell completion
  source '$HOME/.kube/completion.bash.inc'
  " >> $HOME/.bash_profile
  source $HOME/.bash_profile
fi

## 🤔 .. i might remove this.
# krew kubectl plugin
(
  set -x; cd "$(mktemp -d)" &&
  OS="$(uname | tr '[:upper:]' '[:lower:]')" &&
  ARCH="$(uname -m | sed -e 's/x86_64/amd64/' -e 's/\(arm\)\(64\)\?.*/\1\2/' -e 's/aarch64$/arm64/')" &&
  KREW="krew-${OS}_${ARCH}" &&
  curl -fsSLO "https://github.com/kubernetes-sigs/krew/releases/latest/download/${KREW}.tar.gz" &&
  tar zxvf "${KREW}.tar.gz" &&
  ./"${KREW}" install krew
)

# kubectl krew install cilium
# 💩 @jamesc says n0 to kubeseal (anti-pattern)

# ## kubeseal
# ## https://github.com/bitnami-labs/sealed-secrets
# KUBESEAL_VERSION='' # Set this to, for example, KUBESEAL_VERSION='0.23.0'
# curl -OL "https://github.com/bitnami-labs/sealed-secrets/releases/download/v${KUBESEAL_VERSION:?}/kubeseal-${KUBESEAL_VERSION:?}-linux-amd64.tar.gz"
# tar -xvzf kubeseal-${KUBESEAL_VERSION:?}-linux-amd64.tar.gz kubeseal
# sudo install -m 755 kubeseal /usr/local/bin/kubeseal

# inotify-tools
if ! command -v inotifywait &> /dev/null; then
  sudo apt-get install -y inotify-tools
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
# sudo eget bodo-run/yek  --asset musl --to /usr/local/bin
cargo install --git https://github.com/bodo-run/yek


# Direnv
curl -sfL https://direnv.net/install.sh | bash

curl -fsSL https://pixi.sh/install.sh | bash

cargo binstall podlet
