# should be run by _b00t_
source "/c0de/_b00t_/_b00t_.bashrc"

## HashiCorp Vault 

## * * * *// 
#* 💠 Hashicorp Vault
## * * * *\\


vault_bin="$(whereis vault)"
# Add the HashiCorp GPG Key
curl -fsSL https://apt.releases.hashicorp.com/gpg | $SUDO_CMD apt-key add -

# Add official HashiCorp Linux Repo
$SUDO_CMD apt-add-repository "deb [arch=amd64] https://apt.releases.hashicorp.com $(lsb_release -cs) main"

# Update and install
# BROKEN: 
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

mkdir -p /vault/data


# Not using Packer
# sudo apt-get update && sudo apt-get install Packer

# 
# sudo apt-get update && sudo apt-get install nomad
vault server -dev
