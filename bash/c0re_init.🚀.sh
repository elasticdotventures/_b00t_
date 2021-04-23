
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


##* * * * * *\\
function mkdir_命令() {
    dir=$1
    cmd="/bin/mkdir -p \"$dir\""
    result=$( "$cmd" )
    echo "🚀 $cmd"
    
    if [ ! -d "$dir" ] ; then 
        echo "👽不支持 failed. probably requires permission!" 

        echo "😇 running: sudo $cmd"
        `/usr/bin/sudo $cmd`
        if [ ! -d "$dir" ] ; then 
            echo "👽不支持 sudo didn't work either."
        fi
    fi
    }

mkdir_命令 "$HOME/._b00t_"
mkdir_命令 "$HOME/._b00t_/c0re"
chmod 711 "$HOME/._b00t_/c0re"

##* * * * * *//

##* * * * * *\\
# Install Some Tooling
# 🍰 fzf - menu, file choose  https://github.com/junegunn/fzf#using-git
# 🍰 yq  - YAML config i/o    https://github.com/mikefarah/yq
# 🍰 jq  - JSON config i/o    https://stedolan.github.io/jq/

sudo apt-get install fzf jq

if [ ! -f "/usr/bin/yq"] ; then
    wget https://github.com/mikefarah/yq/releases/download/${VERSION}/${BINARY}.tar.gz -O - |\
    tar xz && mv ${BINARY} /usr/bin/yq
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

