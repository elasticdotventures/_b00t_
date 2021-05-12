
## * * * *// 
#* Purpose: imports standard bash behaviors
#*          for consistent handling of parameters
#*
## * * * *//

# Boot functions
source "/c0de/_b00t_/_b00t_.bashrc"
ARCH="$(uname -m | cut -b 1-6)"

# moved to _b00t_.bashrc
# source "$_B00T_C0DE_Path/./bash.🔨/.bash_aliases"


function checkOS() {
    IS_supported=`cat /etc/os-release | grep "Ubuntu 20.04.2 LTS"`
    if [ -z "$IS_supported" ] ; then
        cat /etc/os-release
        log_📢_记录 "👽不支持  OS not yet supported." && exit 0
        return 1
    else
        log_📢_记录 "👍 OS is supported"
    fi

    return 0 
}
checkOS_result="$(checkOS)"
#echo "checkOS_result: $checkOS_result"


#func(){
#    echo "Username: $USER"
#    echo "    EUID: $EUID"
#}
# 🤓 "Exporting" export -f creates an env variable with the function body.
# export -f func

##* * * * * *\\
if [ "$EUID" -ne 0 ]
  then echo "👽 Please run as root, use sudo or enter root password" 
  # su "$SUDO_USER" -c 'func'
fi
##* * * * * *//

## 命令 \\
# Mìnglìng // Command   (mkdir)
function mkdir_命令() {
    args=("$@")
    dir=${args[0]}
    #dir=$1
    cmd="/bin/mkdir -p \"$dir\""
    result=$( "$cmd" )
    echo "🚀 $cmd"
    
    if [ ! -d "$dir" ] ; then 
        log_📢_记录 "👽:不支持 failed. probably requires permission!" 

        log_📢_记录 "😇.run: sudo $cmd"
        `/usr/bin/sudo $cmd`
        if [ ! -d "$dir" ] ; then 
            log_📢_记录 "👽:不支持 sudo didn't work either."
        fi
    fi
    }

#export mkdir_命令
#mkdir_命令 "$HOME/.b00t_"
#mkdir_命令 "$HOME/.b00t_/c0re"
#chmod 711 "$HOME/._00t_/c0re"
## 命令 // 


##* * * * * *\\
# Install Some Tooling
# 🍰 fzf - menu, file choose  https://github.com/junegunn/fzf#using-git
# 🍰 jq  - JSON config i/o    https://stedolan.github.io/jq/
# 🍰 wget- HTTP i/o 
# 🍰 curl- HTTP i/o 

if  [ ! -x "/usr/bin/fzf" ] || \
     [ ! -x "/usr/bin/jq" ] || \
     [ ! -x "/usr/bin/wget" ]  ; then
    $SUDO_CMD apt-get install -y fzf jq wget curl
fi

if n0ta_xfile_📁_好不好 "/usr/bin/fdfind"  ; then
    ## some other applications we'll need
    # 🤓 https://github.com/sharkdp/fd#installation
    $SUDO_CMD apt-get install -y fd-find
    log_📢_记录 "😇.install fd-find helper (fd)"

    $SUDO_CMD mkdir -p ~/.local/bin
    $SUDO_CMD ln -s $(which fdfind) ~/.local/bin/fd
    alias fd="/usr/bin/fdfind"
fi

if n0ta_xfile_📁_好不好 "/usr/bin/rg" ; then
    # RipGrep, needs something higher than v10 included with ubuntu
    # 🤓 https://github.com/BurntSushi/ripgrep#installation
    pwdwas=`pwd`
    tmpdir=$(mktemp -d)
    cd $tmpdir
    log_📢_记录 "😇.install ripgrep (rg) $ARCH"
    case "$ARCH" in
        "x86_64")
            curl -LO https://github.com/BurntSushi/ripgrep/releases/download/12.1.1/ripgrep_12.1.1_amd64.deb
            $SUDO_CMD dpkg -i "$tmpdir/ripgrep_12.1.1_amd64.deb"
            ;;
        "armv7l")
            curl -LO https://github.com/BurntSushi/ripgrep/releases/download/12.1.1/ripgrep-12.1.1-arm-unknown-linux-gnueabihf.tar.gz
            tar -xvzf ripgrep-12.1.1-arm-unknown-linux-gnueabihf.tar.gz
            $SUDO_CMD cp -v ripgrep-12.1.1-arm-unknown-linux-gnueabihf/rg /usr/local/bin/rg
            ;;
        *)
            log_📢_记录 "😇👽.ripgrep $ARCH is unsupported!"
            ;;
    esac
    cd $pwdwas
    #OR .. sudo apt-get install ripgrep
fi

## not presently using whiptail. 
#if n0ta_xfile_📁_好不好 "/usr/bin/whiptail" ; then 
#    # whiptail makes interactive menus
#    # 🤓 https://en.wikibooks.org/wiki/Bash_Shell_Scripting/Whiptail
#    log_📢_记录 "😇.install whiptail menus"
#    $SUDO_CMD apt-get install -y whiptail
#fi

if n0ta_xfile_📁_好不好 "/usr/bin/batcat" ; then 
    log_📢_记录 "😇.install batcat (bat), replaces cat"
    URL=""
    case $ARCH in
        armv7l)
            URL="https://github.com/sharkdp/bat/releases/download/v0.18.0/bat_0.18.0_armhf.deb" 
            ;;
        arm64) 
            URL="https://github.com/sharkdp/bat/releases/download/v0.18.0/bat_0.18.0_arm64.deb"
            ;;
        x86_64)
            URL="https://github.com/sharkdp/bat/releases/download/v0.18.0/bat_0.18.0_amd64.deb"
            ;;
        *) 
            log_📢_记录 "😇🐛 unsupported platform $ARCH"
            ;;
    esac
    if [ -n "$URL" ] ; then 
        log_📢_记录 "😇batcat URL: $URL "
        pwdwas=`pwd`
        tmpdir=$(mktemp -d)
        cd $tmpdir && curl -LO $URL
        FILENAME=$(basename $URL)
        dpkg -i "$tmpdir/$FILENAME"
        cd $pwdwas
        #$SUDO_CMD apt install -y ./$FILENAME
        # $SUDO_CMD apt-get install -y bat
        #$SUDO_CMD mkdir -p ~/.local/bin
        #ln -s /usr/bin/batcat ~/.local/bin/bat
        # example how to use batcat with fzf:
        # fzf --preview 'batcat --style numbers,changes --color=always {} | head -50'
    fi
fi

##### 
## after a lot of moving around, it's clear 
## yq needs to be here, since it's used in a variety of menus
## for d1rd1ct (next)
installYQ=false
YQ_VERSION="v4.7.0"
YQ_BINARY="yq_linux_amd64"  # TODO: multiarch 
YQ_MIN_VERSION="4.0.0"
YQ_INSTALL_PATH="/usr/local/bin/yq"

if n0ta_xfile_📁_好不好 "$YQ_INSTALL_PATH" ; then
    log_📢_记录 "😲 yq does not appear to be installed, f1x1ng."
    # missing yq
    installYQ=true
else 
    # check yq version 
    log_📢_记录 "🧐 checking yq"
    currentYQver="$(yq -V | cut -f 2 -d ' ')"
    isYQokay=$(is_v3rs10n_大于 "$YQ_MIN_VERSION" $currentYQver)
    if [ ! "$isYQokay" = false ] ; then
        # TODO: consent
        log_📢_记录 "👻👼 insufficient yq --version $1, f1x1ng."
        installYQ=true
        $SUDO_CMD snap remove yq
        $SUDO_CMD apt-get remove yq
        $SUDO_CMD rm /usr/bin/yq 
        $SUDO_CMD rm /usr/local/bin/yq 
    fi
fi

# 🍰 yq  - YAML config i/o    https://github.com/mikefarah/yq
# not using yq via snap.  way too old!! 
#if n0ta_xfile_📁_好不好 "/usr/bin/yq" ; then
#    systemctl status snapd.service
#    snap install yq
#fi
if [ "$installYQ" = true ] ; then 
    log_📢_记录 "🐧😇 upgrading $YQ_INSTALL_PATH"
    tmpdir=$(mktemp -d)
    pwdwas=`pwd`
    cd $tmpdir && \
     wget https://github.com/mikefarah/yq/releases/download/${YQ_VERSION}/${YQ_BINARY}.tar.gz -O - |\
     tar xz && $SUDO_CMD cp ${YQ_BINARY} "$YQ_INSTALL_PATH" && \
     rm -f $YQ_BINARY
    cd $pwdwas
    
    currentYQver="$(yq -V | cut -f 2 -d ' ')"
    isYQokay=$(is_v3rs10n_大于 "$YQ_MIN_VERSION" $currentYQver)
    if n0ta_xfile_📁_好不好 "$YQ_INSTALL_PATH" ; then
        log_📢_记录 "💩 STILL missing $YQ_INSTALL_PATH (required for d1ctd1r)"
        exit
    elif [ "$isYQokay" = true ] ; then
        log_📢_记录 "😇🎉 yq installed"
    else
        log_📢_记录 "💩🍒 yq installed, but version still insufficient (wtf)"
    fi 
fi



log_📢_记录 "🥾😇.install dialog & apt-utils"
$SUDO_CMD apt-get install -y dialog apt-utils

# _b00t_ cli - "/usr/local/bin/b00t"

## 
if [ ! -f "/usr/local/bin/b00t" ] ; then
    $SUDO_CMD ln -s /c0de/_b00t_/up-cli.sh /usr/local/bin/b00t
fi

##* * * * * *//



