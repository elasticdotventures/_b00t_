
#
#  DOCKER bashrc extensions
#

# 🍰 https://calbertts.medium.com/docker-and-fuzzy-finder-fzf-4c6416f5e0b5
# Running containers
# The first command is runc, which is from now an amazing command to run a new container by selecting the docker image from a interactive menu
function runc() {
  export FZF_DEFAULT_OPTS='--height 90% --reverse --border'
  local image=$(docker images --format '{{.Repository}}:{{.Tag}}' | fzf-tmux --reverse --multi)
  if [[ $image != '' ]]; then
    echo -e "\n  \033[1mDocker image:\033[0m" $image
    read -e -p $'  \e[1mOptions: \e[0m' -i "-it --rm" options

    printf "  \033[1mChoose the command: \033[0m"
    local cmd=$(echo -e "/bin/bash\nsh" | fzf-tmux --reverse --multi)
    if [[ $cmd == '' ]]; then
        read -e -p $'  \e[1mCustom command: \e[0m' cmd
    fi
    echo -e "  \033[1mCommand: \033[0m" $cmd

    export FZF_DEFAULT_COMMAND='find ./ -type d -maxdepth 1 -exec basename {} \;'
    printf "  \033[1mChoose the volume: \033[0m"
    local volume=$(fzf-tmux --reverse --multi)
    local curDir=${PWD##*/}
    if [[ $volume == '.' ]]; then
        echo -e "  \033[1mVolume: \033[0m" $volume
        volume="`pwd`:/$curDir -w /$curDir"
    else
        echo -e "  \033[1mVolume: \033[0m" $volume
        volume="`pwd`/$volume:/$volume -w /$volume"
    fi

    export FZF_DEFAULT_COMMAND=""
    export FZF_DEFAULT_OPTS=""

    history -s runc
    history -s docker run $options -v $volume $image $cmd
    echo ''
    docker run $options -v $volume $image $cmd
  fi
}


# 🍰 https://calbertts.medium.com/docker-and-fuzzy-finder-fzf-4c6416f5e0b5
# Running containers
# The first command is runc, which is from now an amazing command to run a new container by selecting the docker image from a interactive menu
runinc() {
  export FZF_DEFAULT_OPTS='--height 90% --reverse --border'
  local container=$(docker ps --format '{{.Names}} => {{.Image}}' | fzf-tmux --reverse --multi | awk -F '\\=>' '{print $1}')
  if [[ $container != '' ]]; then
    echo -e "\n  \033[1mDocker container:\033[0m" $container
    read -e -p $'  \e[1mOptions: \e[0m' -i "-it" options
    if [[ $@ == '' ]]; then
				read -e -p $'  \e[1mCommand: \e[0m' cmd
    else
				cmd="$@"
    fi
    echo ''
    history -s runinc "$@"
    history -s docker exec $options $container $cmd
    docker exec $options $container $cmd
    echo ''
  fi
  export FZF_DEFAULT_OPTS=""
}


# 🛑 Stopping/Removing containers
# Stops and/or removes a docker container
stopc() {
  export FZF_DEFAULT_OPTS='--height 90% --reverse --border'
  local container=$(docker ps --format '{{.Names}} => {{.Image}}' | fzf-tmux --reverse --multi | awk -F '\\=>' '{print $1}')
  if [[ $container != '' ]]; then
    echo -e "\n  \033[1mDocker container:\033[0m" $container
    printf "  \033[1mRemove?: \033[0m"
    local cmd=$(echo -e "No\nYes" | fzf-tmux --reverse --multi)
    if [[ $cmd != '' ]]; then
      if [[ $cmd == 'No' ]]; then
        echo -e "\n  Stopping $container ...\n"
        history -s stopc
        history -s docker stop $container
        docker stop $container > /dev/null
      else
        echo -e "\n  Stopping $container ..."
        history -s stopc
        history -s docker stop $container
        docker stop $container > /dev/null

        echo -e "  Removing $container ...\n"
        history -s stopc
        history -s docker rm $container
        docker rm $container > /dev/null
      fi
    fi
  fi
  export FZF_DEFAULT_OPTS=""
}

# Getting the container’s IP Address
# Inspect the IP address quickly choosing the container from the menu by running showipc
showipc() {
  export FZF_DEFAULT_OPTS='--height 90% --reverse --border'
  local container=$(docker ps -a --format '{{.Names}} => {{.Image}}' | fzf-tmux --reverse --multi | awk -F '\\=>' '{print $1}')

  if [[ $container != '' ]]; then
    local network=$(docker inspect $container -f '{{.NetworkSettings.Networks}}' | awk -F 'map\\[|:' '{print $2}')
    echo -e "\n  \033[1mDocker container:\033[0m" $container
    history -s showipc
    history -s docker inspect -f "{{.NetworkSettings.Networks.${network}.IPAddress}}" $container
    echo -e "  \033[1mNetwork:\033[0m" $network
    echo -e "  \033[1mIP Address:\033[0m" $(docker inspect -f "{{.NetworkSettings.Networks.${network}.IPAddress}}" $container) "\n"
  fi
}

log_📢_记录 "🥾🐳 $ runc, runinc, stopc, showipc"


