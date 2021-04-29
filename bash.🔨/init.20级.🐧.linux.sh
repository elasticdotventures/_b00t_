# should be run by _b00t_

source "/c0de/_b00t_/_b00t_.bashrc"

## * * * *// 
#* 🐧 Purpose: b00tstraps t0rvalds t00ls. ;-) 
#* should be called directly from ./01-start.sh 
## * * * *\\


# todo: setup io_streams, ebpf intercepts. 

# yq, part II - Windows
## For WSL - snapd won't work properly unless we also: 
if  is_WSLv2_🐧💙🪟v2  ; then
    $SUDO_CMD apt-get update && $SUDO_CMD apt-get install -yqq daemonize dbus-user-session fontconfig
fi

# 🍰 yq  - YAML config i/o    https://github.com/mikefarah/yq
if n0ta_xfile_📁_好不好 "/usr/bin/yq" ; then
    systemctl status snapd.service
    snap install
fi

if n0ta_xfile_📁_好不好 "/usr/bin/yq" ; then
    YQ_VERSION="v4.7.0"
    YQ_BINARY="yq_linux_amd64"
    wget https://github.com/mikefarah/yq/releases/download/${YQ_VERSION}/${YQ_BINARY}.tar.gz -O - |\
        tar xz && cp ${YQ_BINARY} /usr/bin/yq

    if n0ta_file_📁_好不好 "/usr/bin/yq" ; then
        log_📢_记录 "💩 STILL missing /usr/bin/yq"
        exit
    fi
fi

# software-properties-common tools is reuqired by git
$SUDO_CMD apt install -y software-properties-common
