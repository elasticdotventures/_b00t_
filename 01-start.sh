#!/bin/bash

##* * * *\\ 
#* Purpose: 
#*   boot-strap common bash libraries
#*   access Azure
##* * * *//

# 小路 : Xiǎolù Path or Directory
# THINGS YOU CAN EDIT: 
export _B00T_C0DE_Path="/c0de/_b00t_"        
export _B00T_C0NFIG_Path="$HOME/.b00t"

##* * * * \\  
# 进口 (Jìnkǒu) Import c0re Libraries Alpha 
function bash_source_加载() {
    file="$1"
    if [ ! -x "$file" ] ; then
        echo "missing $file" && exit 
    else
        source "$file" 
        if [ $? -gt 0 ] ; then
            echo "☹️ $file had error. 🛑"
        fi
    fi

    return $?
}

# Bin shell & helpers
bash_source_加载 "$_B00T_C0DE_Path/./bash/c0re_init.🚀.sh"

## 进口 * * * \\ 
## [Jìnkǒu] Import
## minimal c0re Python 🐍
##   establish .venv
#   we also do some 
bash_source_加载 "$_B00T_C0DE_Path/./bash/c0re_init.🐍.sh"
source .venv/bin/activate
## 进口 * * * // 


## 项目 * * * * \\  
# (Xiàngmù) Project Id
EmojiWords +=(
    # shuf -n5 /usr/share/dict/american-english | cut -d$'\t' -f1   
    )

export c0re_pr0j3ct_id="moist_monkey"
##* * * * //


## !TODO: Do you need a project name?
## !TODO: Do we have an AZ tenant Id?
## 要不要　
## !TODO: Do you need a resource Group?
## !TODO: 


##* * * * \\
az_resGroupId=$(az group show --name $az_groupName --query id --output tsv)
# $echo groupId
# /subscriptions/{###}/resourceGroups/{groupName}
az ad sp create-for-rbac \
  # --scopes  # !TODO
  --scope $az_resGroupId --role Contributor \
  --name $az_projectId-🤴校长_principal \
  --sdk-auth
##* * * * //


##* * * * \\
# 目录 (Mùlù) Directory
if [ -d "$c0dePath/./pr0j3cts/./$project_dir" ] ; then
    export PROJECT_dirExist=`$c0dePath/./pr0j3cts/./$project_dir`
    echo "🚀 the $c0dePath/./pr0j3cts/./$project_dir already exists use --force"
else
    export PROJECT_dirExists=""
fi
mkdir -p "$c0dePath/./pr0j3cts/./$project"
##* * * * // 

##* * * * \\
## 怎么样 (zěnme yàng) Present,How & What
#* - $AZ_resourceGroup is set
#* - c0re-lib verifies az cli is installed 
source "./bash/AZ_CLI_init.🤖.sh"
##* * * * //

##* * * * \\ 
# 🤖 微软 Wēiruǎn (Microsoft) Zzure
# 🤖 微软 Azure Login, verify credentials
# az login --verbose
source "./bash/AZ_init.🤖.sh"
##* * * * //

##* * * * \\
source "./bash/AZ_todo.🤖.sh"
##* * * * //


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
docker build -t cowsay .
# 🐳♻️ It’s a good habit to use --rm to avoid filling up your system with stale Docker containers.
docker run --rm cowsay 

echo "* if you just saw a talking cow, everything is fine!"
echo "run ./02_t00ls_.sh
