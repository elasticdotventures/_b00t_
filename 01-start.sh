#!/bin/bash

## * * * *\\ 
#* Purpose: 
#*   boot-strap common bash libraries
#*   access Azure
#*   
#* 
## * * * *//

##* * * * \\ 🚀 
# 进口 (Jìnkǒu) Import c0re Libraries Alpha
if [ ! -x "./bash/🚀_c0re-lib.sh" ] ; then
    echo "missing ./bash/🚀_c0re-lib.sh" && exit 
else
    source "./bash/🚀_c0re-lib.sh" 
fi
##* * * * // 🚀 

if [ -d "/c0re/$project_dir" ] ; then
    PROJECT_dirExist=``
else
    PROJECT_dirExists
fi
mkdir -p /c0re/$project

##* * * * \\
# 怎么样 (zěnme yàng) Present,How & What:
# - $AZ_resourceGroup is set
# - c0re-lib verifies az cli is installed 
source "./bash/👾_AZ_CLI_init.sh"
##* * * * //

##* * * * \\ 
# 🤖 微软 Wēiruǎn (Microsoft) Zzure
# 🤖 微软 Azure Login, verify credentials
# az login --verbose
source "./bash/👾_AZ_init.sh"
##* * * * //

##* * * * \\
source "./bash/👾_AZ_todo.sh"


##* * * * * * * *//
#*  🐳 docker setup.
##* * * * * * * *\\
WHATIS_DOCKER_VERSION=`docker -v`
if [ $? -ne 0 ]; then
    ##* * * * \\
    #* 🤓 Before you install Docker Engine for the first time on a new host machine, 
    #* you need to set up the Docker repository. Afterward, you can install and update 
    #* Docker from the repository.

    # docker not installed
    # https://docs.docker.com/engine/install/ubuntu/
    # 🐳 Remove Old Versions
    sudo apt-get remove -y docker docker-engine docker.io containerd runc
    # 🐳🧹
    sudo apt-get -y update
    # 🐳 Install required modules 
    sudo apt-get install \
        apt-transport-https \
        ca-certificates \
        curl \
        gnupg \
        lsb-release
    # 🐳 Add Dockers official GPG Key
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg  
    # 🐳 Use the following command to set up the stable repository
    echo \
        "deb [arch=amd64 signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu \
        $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
    # 🐳🧹
    sudo apt-get update -y
    # 🐳
    sudo apt-get install docker-ce docker-ce-cli containerd.io
    ##* * * * // 
fi
# 🐳💥
DOCKER_isHappy=`sudo docker run hello-world`
if [ -n "$DOCKER_isHappy" ] ; then
    echo "🐳💥 docker is br0ked. plz fix."
fi
#🐳⚠️ Adding a user to the “docker” group grants them the ability to run 
# containers which can be used to obtain root privileges on the Docker host. 
# Refer to Docker Daemon Attack Surface for more information.
sudo usermod -aG docker `whoami`

# TODO: link to the Elasticdotventures repository
# 

# create AzAppConfig
# create AzSecureVault
# create AzStorage


