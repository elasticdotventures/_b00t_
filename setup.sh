#!/bin/bash

# a lot of this was migrated from promptexecution/infrastructure

gh extension install https://github.com/nektos/gh-act


sudo apt install -y build-essential

git config --global user.email "brianh@elastic.ventures"
git config --global user.name "Brian H"


sudo apt install ntpdate
sudo ntpdate pool.ntp.org

sudo apt update
sudo apt-get install -y jq fzf ripgrep tree

# https://opentofu.org/docs/intro/install/deb
curl --proto '=https' --tlsv1.2 -fsSL 'https://packages.opentofu.org/install/repositories/opentofu/tofu/script.deb.sh?any=true' -o /tmp/tofu-repository-setup.sh
# Inspect the downloaded script at /tmp/tofu-repository-setup.sh before running
sudo bash /tmp/tofu-repository-setup.sh
rm /tmp/tofu-repository-setup.sh

sudo apt-get install -y tofu
alias tf=tofu




curl -sS https://starship.rs/install.sh | sh

echo eval "$(starship init bash)" >> ~/.bashrc


# 🦀 rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# tree but ignores .git (useful for chatgpt dumps)
alias itree='rg --files | tree --fromfile'
cargo install ripgrep


# just is a command runner
curl --proto '=https' --tlsv1.2 -sSf https://just.systems/install.sh | sudo bash -s -- --to /usr/local/bin

# 💩 yq
# https://mikefarah.gitbook.io/yq/v/v3.x/
# docker run --rm -v "${PWD}":/workdir mikefarah/yq
sudo add-apt-repository -y ppa:rmescandon/yq
sudo apt update
sudo apt install yq bat -y
## someday..
# alias yq="podman run --rm -v \"${PWD}\":/workdir docker.io/mikefarah/yq"
# https://kislyuk.github.io/yq/

# ubuntu installs bat as batcat
mkdir -p ~/.local/bin
ln -s /usr/bin/batcat ~/.local/bin/bat


## DEV workstation
# for pgrx, llvm
 sudo apt install -y libclang-dev

# aws configure
wget https://github.com/engineerd/wasm-to-oci/releases/download/v0.1.2/linux-amd64-wasm-to-oci
mv linux-amd64-wasm-to-oci wasm-to-oci
chmod +x wasm-to-oci
sudo cp wasm-to-oci /usr/local/bin

# azure
alias az="docker run -it -v ${HOME}/.ssh:/root/.ssh mcr.microsoft.com/azure-cli"

# aws



# gcloud
# https://cloud.google.com/sdk/docs/install#deb
#sudo apt-get install apt-transport-https ca-certificates gnupg curl
#curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg
#echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | sudo tee -a /etc/apt/sources.list.d/google-cloud-sdk.list
#sudo apt-get update && sudo apt-get install google-cloud-cli
#sudo apt-get install google-cloud-cli-gke-gcloud-auth-plugin
#gcloud init

# gcloud
# https://cloud.google.com/sdk/docs/install#deb
sudo apt-get install apt-transport-https ca-certificates gnupg curl
curl https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg
echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" | sudo
sudo apt-get update && sudo apt-get install google-cloud-cli
#sudo apt-get install google-cloud-cli-gke-gcloud-auth-plugin
gcloud init

# The docker route probably won't work with terraform.
#docker pull gcr.io/google.com/cloudsdktool/google-cloud-cli:latest
#docker run --rm gcr.io/google.com/cloudsdktool/google-cloud-cli:latest gcloud version
#docker run -ti --name gcloud-config gcr.io/google.com/cloudsdktool/google-cloud-cli gcloud auth login
#docker run --rm --volumes-from gcloud-config gcr.io/google.com/cloudsdktool/google-cloud-cli gcloud compute instances list --project your_project


# s3 mount
# 🤓 terraform/aws/s3-mountpoint-ubuntu-x86.sh
sudo mount-s3 --region ap-southeast-4  promptexecution-tank /mnt/aws/tank


