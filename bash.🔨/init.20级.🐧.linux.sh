# should be run by _b00t_

# SEARCH FOR A PACKAGE: 
# apt-cache search libpackage

source "/c0de/_b00t_/_b00t_.bashrc"


## * * * *// 
#* 🐧 Purpose: b00tstraps t0rvalds t00ls. ;-) 
#* should be called directly from ./01-start.sh 
## * * * *\\

if n0ta_xfile_📁_好不好 "/bin/sudo" ; then 
    # this is correct (leaving for future linting)
    log_📢_记录 "🐧😇 sudo"
    $SUDO_CMD apt-get install -y sudo
fi
apt-get install -y sudo

# todo: setup io_streams, ebpf intercepts. 

# yq, part II - Windows
## For WSL - snapd won't work properly unless we also: 
if  is_WSLv2_🐧💙🪟v2  ; then
    log_📢_记录 "🐧😇 wsl2 setup"
    $SUDO_CMD apt-get update && $SUDO_CMD apt-get install -yqq daemonize dbus-user-session fontconfig
fi

# 🍰 yq  - YAML config i/o    https://github.com/mikefarah/yq
# not using yq via snap. 
#if n0ta_xfile_📁_好不好 "/usr/bin/yq" ; then
#    log_📢_记录 "🐧😇 yq"
#    systemctl status snapd.service
#    snap install yq
#fi

if n0ta_xfile_📁_好不好 "/usr/bin/yq" ; then
    YQ_VERSION="v4.7.0"
    YQ_BINARY="yq_linux_amd64"
    wget https://github.com/mikefarah/yq/releases/download/${YQ_VERSION}/${YQ_BINARY}.tar.gz -O - |\
        tar xz && cp ${YQ_BINARY} /usr/bin/yq && rm -f $YQ_BINARY

    if n0ta_xfile_📁_好不好 "/usr/bin/yq" ; then
        log_📢_记录 "💩 STILL missing /usr/bin/yq"
        exit
    fi
fi

# software-properties-common tools is required by git
log_📢_记录 "🐧😇 git dependencies"
$SUDO_CMD apt-get install -y software-properties-common

### Deprecation 

# Bash Aliases were moved into _b00t_.bashrc
# .. for storytime tutorial, this is kept as a reference.
# 
#log_📢_记录 "🐧😇 install .bash_aliases"
#if [ -e $HOME/.bash_aliases ]; then
#    source $HOME/.bash_aliases
#fi
# this could probably be copied to $HOME/.bash_aliases?
#source "$_B00T_C0DE_Path/bash.🔨/.bash_aliases"


