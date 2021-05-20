# should be run by _b00t_
source "$_B00T_C0DE_Path/_b00t_.bashrc"

terraform_bin="$(whereis terraform)"
vault_bin="$(whereis vault)"

if [ ! -x $terraform_bin ] ; then
    # Add the HashiCorp GPG Key
    $SUDO_CMD curl -fsSL https://apt.releases.hashicorp.com/gpg | $SUDO_CMD apt-key add -
    # Add official HashiCorp Linux Repo
    $SUDO_CMD apt-add-repository "deb [arch=amd64] https://apt.releases.hashicorp.com $(lsb_release -cs) main"
fi

## * * * *// 
#* 💠 Hashicorp HashiCorp TerraForm
# Vault unseal operation requires a quorum of existing unseal keys split by Shamir's Secret sharing algorithm. This is done so that the "keys to the kingdom" won't fall into one person's hand
# Vault supports opt-in automatic unsealing via cloud technologies: AliCloud KMS, Amazon KMS, Azure Key Vault, and Google Cloud KMS. 
# 🤓 https://learn.hashicorp.com/tutorials/vault/autounseal-azure-keyvault?in=vault/day-one-consul
## * * * *\\

# https://www.terraform.io/downloads.html
# https://learn.hashicorp.com/tutorials/terraform/install-cli?in=terraform/azure-get-started

if [ -x $terraform ] ; then 
    $SUDO_CMD apt-get update && $SUDO_CMD apt-get install -y gnupg software-properties-common curl
    $SUDO_CMD apt-get update && $SUDO_CMD apt-get install -y terraform
fi 

## * * * *// 
#* 💠 Hashicorp Vault
# Vault unseal operation requires a quorum of existing unseal keys split by Shamir's Secret sharing algorithm. This is done so that the "keys to the kingdom" won't fall into one person's hand
# Vault supports opt-in automatic unsealing via cloud technologies: AliCloud KMS, Amazon KMS, Azure Key Vault, and Google Cloud KMS. 
# 🤓 https://learn.hashicorp.com/tutorials/vault/autounseal-azure-keyvault?in=vault/day-one-consul
## * * * *\\

# Update and install
# 🍒 vault was BROKEN in apt repo as of Apr 20th, 2021
#$SUDO_CMD apt-get update && $SUDO_CMD apt-get install vault

# https://learn.hashicorp.com/collections/vault/getting-started
if [ ! -x $vault_bin ] ; then
    pwdWas=`pwd`
    tmpdir=`mktemp -d`
    cd $tmpdir
    wget https://releases.hashicorp.com/vault/1.7.1/vault_1.7.1_linux_amd64.zip
    apt-get install -y unzip 
    unzip -d vault_1.7.1_linux_amd64.zip
    cp -fv vault /usr/bin/vault
    cd $pwdWas
fi

#mkdir -p /vault/data

# Not using Packer
# sudo apt-get update && sudo apt-get install Packer

# Not using nomad
# sudo apt-get update && sudo apt-get install nomad

# for now, dev mode is fine. 
# vault server -dev

# https://registry.hub.docker.com/_/vault/
#docker run --cap-add=IPC_LOCK -d --name=dev-vault vault
# 😁 docker run --cap-add=IPC_LOCK -e 'VAULT_DEV_ROOT_TOKEN_ID=myroot' -e 'VAULT_DEV_LISTEN_ADDRESS=0.0.0.0:1234' vault
#docker run --cap-add=IPC_LOCK -e 'VAULT_LOCAL_CONFIG={"backend": {"file": {"path": "/vault/file"}}, "default_lease_ttl": "168h", "max_lease_ttl": "720h"}' vault server
# At startup,
# * the server will read configuration HCL and JSON files 
# * from /vault/config (any information passed into VAULT_LOCAL_CONFIG is written into local.json in this directory and read as part of reading the directory for configuration files). 
# Please see Vault's configuration documentation for a full list of options.

