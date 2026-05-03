# should be run by _b00t_
source "$_B00T_C0DE_Path/_b00t_.bashrc"

## THIS COMMAND SEEMS TO WORK FOR DOCKER IN DOCKER. 
# docker run -d --name systemd-ubuntu --tmpfs /tmp --tmpfs /run --tmpfs /run/lock  --mount type=bind,source="/c0de",target="/c0de"  --privileged -v /var/run/docker.sock:/var/run/docker.sock -v /sys/fs/cgroup:/sys/fs/cgroup:ro jrei/systemd-ubuntu
# requires systemd-ubuntu base image. 

## * * * *// 
#* 🐳Docker!
## * * * *\\

# Arm32v7 docker hub (for rpi🥧)
#https://hub.docker.com/u/arm32v7/

# REMEMBER:
#  * Obsolete: Swarm => K8, C-Groups => Systemd

## 鲸 \\
# Jīng :: Whale

log_📢_记录 "🤓 normal for docker Not Be Found:"
WHATIS_DOCKER_VERSION=`docker -v`
if [ $? -ne 0 ]; then
    log_📢_记录 "💙 installing Docker"
    ##* * * * \\
    #* 🤓 Before you install Docker Engine for the first time on a new host machine, 
    #* you need to set up the Docker repository. Afterward, you can install and update 
    #* Docker from the repository.

    # docker not installed
    # https://docs.docker.com/engine/install/ubuntu/
    # 🐳 Remove Old Versions
    $SUDO_CMD apt-get remove -y docker docker-engine docker.io containerd runc
    # 🐳🧹
    $SUDO_CMD apt-get -y update
    # 🐳 Install required modules 
    $SUDO_CMD apt-get -y install \
        apt-transport-https \
        ca-certificates \
        curl \
        gnupg \
        lsb-release
    # 🐳 Add Dockers official GPG Key
    DOCKER_GPG_KEYRING="/usr/share/keyrings/docker-archive-keyring.gpg"
    if [ ! -f $DOCKER_GPG_KEYRING ] ; then
        curl -fsSL https://download.docker.com/linux/ubuntu/gpg | $SUDO_CMD gpg --dearmor -o $DOCKER_GPG_KEYRING  
    fi 
    # 🐳 Use the following command to set up the stable repository
    echo \
        "deb [arch=amd64 signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu \
        $(lsb_release -cs) stable" | $SUDO_CMD tee /etc/apt/sources.list.d/docker.list > /dev/null
    # 🐳🧹
    $SUDO_CMD apt-get update -y
    # 🐳
    $SUDO_CMD apt-get install -y docker-ce docker-ce-cli containerd.io
    ##* * * * // 
fi
# 🐳💥
DOCKER_isHappy=`$SUDO_CMD docker run hello-world`
if [ -n "$DOCKER_isHappy" ] ; then
    echo "🐳💥 docker is br0ked. plz fix."
fi


#🐳⚠️ Adding a user to the “docker” group grants them the ability to run 
# containers which can be used to obtain root privileges on the Docker host. 
# Refer to Docker Daemon Attack Surface for more information.
# sudo usermod -aG docker `whoami`

# TODO: link to the Elasticdotventures repository

# doesn't work presently:  
#docker build -t cowsay .
# 🐳♻️ It’s a good habit to use --rm to avoid filling up your system with stale Docker containers.
#docker run --rm cowsay 

# Azure ACI & Docker Compose Docs: 
# https://docs.docker.com/cloud/aci-integration/

# TODO: DOcker Hub Access Tokens
# https://docs.docker.com/docker-hub/access-tokens/

# Docker
## 鲸 //

# enable on startup: 
$SUDO_CMD systemctl enable docker.service
$SUDO_CMD systemctl enable containerd.service

# TO DISABLE:
# sudo systemctl disable docker.service
# sudo systemctl disable containerd.service

 #docker service create \
 #   --mount 'type=volume,src=b00t,dst=/c0de/b00t,volume-driver=local'
 #   --name b00t \
 #   <IMAGE>

# todo: https://docs.docker.com/storage/bind-mounts/
# docker --mount


#docker service create \
#    --mount type=bind,source=/c0de/b00t,target=/c0de/b00t \
#    --name b00t 

# https://www.instructables.com/Build-Docker-Image-for-Raspberry-Pi/
# --platform linux/amd64,linux/arm 
# https://www.docker.com/blog/multi-arch-images/ 


# to run dev  instance in azure aci
#service principle: 
#docker login azure --client-id xx --client-secret yy --tenant-id zz

## this should have been done back in _b00t_

docker run -it bats/bats:latest --version