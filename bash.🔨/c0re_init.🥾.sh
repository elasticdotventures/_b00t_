
## * * * *// 
#* Purpose: imports standard bash behaviors
#*          for consistent handling of parameters
#*
## * * * *//




function checkOS() {
    IS_supported=`cat /etc/os-release | grep "Ubuntu 20.04.2 LTS"`
    if [ -z "$IS_supported" ] ; then
        cat /etc/os-release
        echo "👽不支持  OS not yet supported." && exit 0
    fi
    return "🙂"
}
checkOS_result="$(checkOS)"
echo "checkOS_result: $checkOS_result"

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
    dir=$1
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
export mkdir_命令
mkdir_命令 "$HOME/._b00t_"
mkdir_命令 "$HOME/._b00t_/c0re"
chmod 711 "$HOME/._b00t_/c0re"
## 命令 // 

## 好不好 \\
## Hǎo bù hǎo :: Good / Not Good 
## is_file readable? 
# n0t_file_📁_好不好 result: 
#   0 : file is okay
#   1 : file is NOT okay
function n0ta_file_📁_好不好() {

    if [ ! -f "$1" ] ; then
        log_📢_记录 "👽:不支持 $1 is both required AND missing. 👽:非常要!"
        return 0
    elif [ ! -x "$1" ] ; then
        log_📢_记录 "👽:不支持 $1 is not executable. 👽:非常要!"
        return 0
    else
        # success
        log_📢_记录 "👍 $1"
        return 1
    fi
}
## 好不好 // 

### - -   is_WSLv2_🐧💙🪟v2   - - \\
## Microsoft Windows Linux Subsystem II  WSL2
## 🤓 https://docs.microsoft.com/en-us/windows/wsl/install-win10
#
function is_WSLv2_🐧💙🪟v2() {
    return `cat /proc/version | grep -c "microsoft-standard-WSL2"`
}
### - -  ..  - - //



##* * * * * *\\
# Install Some Tooling
# 🍰 fzf - menu, file choose  https://github.com/junegunn/fzf#using-git
# 🍰 jq  - JSON config i/o    https://stedolan.github.io/jq/
# 🍰 wget- HTTP i/o 
# 🍰 curl- HTTP i/o 

sudo apt-get install -y fzf jq wget curl

# yq, part II - Windows
## For WSL - snapd won't work 
if is_WSLv2_🐧💙🪟v2 ; then
    sudo apt-get update && sudo apt-get install -yqq daemonize dbus-user-session fontconfig
fi

# 🍰 yq  - YAML config i/o    https://github.com/mikefarah/yq
if n0ta_file_📁_好不好 "/usr/bin/yq" ; then
    systemctl status snapd.service
    snap install
fi

if n0ta_file_📁_好不好 "/usr/bin/yq" ; then
    YQ_VERSION="v4.7.0"
    YQ_BINARY="yq_linux_amd64"
    wget https://github.com/mikefarah/yq/releases/download/${YQ_VERSION}/${YQ_BINARY}.tar.gz -O - |\
        tar xz && cp ${YQ_BINARY} /usr/bin/yq

    if n0ta_file_📁_好不好 "/usr/bin/yq" ; then
        log_📢_记录 "💩 STILL missing /usr/bin/yq"
        exit
    fi
fi

# _b00t_ cli - "/usr/local/bin/b00t"
## 
if n0ta_file_📁_好不好 "/usr/local/bin/b00t" ; then
    sudo ln -s /c0de/_b00t_/_b00t_-cli.sh /usr/local/bin/b00t
fi

##* * * * * *//


##* * * * * *\\
## generates a random number between 0 and \$1
# usage: 
# rand0_result="$(rand0 100)"
# echo \$rand0_result

function rand0() {
    max="$1"
    rand0=$( bc <<< "scale=2; $(printf '%d' $(( $RANDOM % $max)))" ) ;
    # rand0=$( echo $RANDOM % $max ) ; 
    echo $rand0
}

##* * * * * *//

