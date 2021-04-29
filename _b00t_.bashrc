
# usage:
#   source "./_b00t_.bashrc"
#   may also *eventually* run via commandline. 

# https://misc.flogisoft.com/bash/tip_colors_and_formatting
# https://github.com/awesome-lists/awesome-bash

# set -o errexit    # Used to exit upon error, avoiding cascading errors
set -o nounset    # Exposes unset variables, strict mode. 

## 小路 \\
## Xiǎolù :: Path or Directory
# THINGS YOU CAN EDIT: 
export _B00T_C0DE_Path="/c0de/_b00t_"        
export _B00T_C0NFIG_Path="$HOME/.b00t"
## 小路 //


_b00t_exists=`type -t "_b00t_init_🥾_开始"`
if [ "$_b00t_exists" == "function" ] ; then 
    # short circuit using rand0() function 
    return
fi



## 记录 \\
## Jìlù :: Record (Log)
# 🤓 write to a log if you want using >> 
function log_📢_记录() {
    echo "$@"
}
export -f log_📢_记录
## 记录 //



## 进口 \\  
## Kāishǐ :: Start
# init should be run by every program. 
# this is mostly here for StoryTime
function _b00t_init_🥾_开始() {
    # earlier versions, sunset: 
    #🌆 ${0}/./${0*/}"   
    #🌆 export _b00t_="$(basename $0)"
    export _b00t_="$0" 
    PARENT_COMMAND=$(ps -o comm= $PPID)

    if [ "$PARENT_COMMAND" == "bash" ] ; then
        # most common case can be summarized
        log_📢_记录 "🥾👵:🔨"
    else 
        log_📢_记录 "🥾👵 from: $PARENT_COMMAND"
    fi


    log_📢_记录 "🥾 init: $_b00t_"
    if [ -z "${@}" ] ; then 
        log_📢_记录 "🥾 args: ${@}"  
    fi 
}
export -f _b00t_init_🥾_开始

_b00t_init_🥾_开始
## 进口 //



## 加载 * * * * * *\\
## Jiāzài :: Load
function bash_source_加载() {
    local file=$1

    log_📢_记录 "."
    log_📢_记录 "."

    # Bash Shell Parameter Expansion:
    # 🤓 https://www.gnu.org/software/bash/manual/html_node/Shell-Parameter-Expansion.html
    # The ‘$’ character introduces parameter expansion, command substitution, or arithmetic expansion. 
    # {} are optional, but protect enclosed variable
    # when {} are used, the matching ending brace is the first ‘}’ not escaped by a backslash or within a quoted string, and not within an embedded arithmetic expansion, command substitution, or parameter expansion.
    # 🍰 https://www.xaprb.com/media/2007/03/bash-parameter-expansion-cheatsheet.pdf
    
    function expand { for arg in "$@"; do [[ -f $arg ]] && echo $arg; done }

    if [ ! -x "$file" ] ; then
        # .bashrc file doesn't exist, so let's try to find it. 
        # trythis="${trythis:-$file}"        
        # trythis=$file
        # ${trythis:-$file}
        # 
        log_📢_记录 "🧐 expand $file"
        file=$( expand $file )
        
        if [ -x "$file" ] ; then
            log_📢_记录 "🧐 using $file"
        else 
            log_📢_记录 "😲 NOT EXECUTABLE $file"
        fi

    fi

    if [ ! -x "$file" ] ; then
        log_📢_记录 "🗄️🔃😲🍒 NOT EXECUTABLE: $file" && exit 
    else
        log_📢_记录 "🗄️🔃😏  START: $file"
        source "$file" 
        if [ $? -gt 0 ] ; then
            echo "☹️🛑🛑🛑 ERROR: $file had runtime error! 🛑🛑🛑"
        fi
        log_📢_记录 "🗄️🔚😁 FINISH: $file"
    fi

    return $?
}
export -f bash_source_加载


## 好不好 \\
## Hǎo bù hǎo :: Good / Not Good 
## is_file readable? 
# n0t_file_📁_好不好 result: 
#   0 : file is okay
#   1 : file is NOT okay
function n0ta_xfile_📁_好不好() {

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


function selectEditVSCode_experiment() {
    filename=$1
    # select file
    selectedFile="${ fzf $filename }"
    code -w $selectedFile
    
}



### - -   is_WSLv2_🐧💙🪟v2   - - \\
## Microsoft Windows Linux Subsystem II  WSL2
## 🤓 https://docs.microsoft.com/en-us/windows/wsl/install-win10
#
function is_WSLv2_🐧💙🪟v2() {
    return `cat /proc/version | grep -c "microsoft-standard-WSL2"`
}
### - -  ..  - - //


# 🍰 https://stackoverflow.com/questions/3963716/how-to-manually-expand-a-special-variable-ex-tilde-in-bash/29310477#29310477
# converts string ~/.b00t to actual path
# usage: path=$(expandPath '~/hello')
function expandPath() {
  local path
  local -a pathElements resultPathElements
  IFS=':' read -r -a pathElements <<<"$1"
  : "${pathElements[@]}"
  for path in "${pathElements[@]}"; do
    : "$path"
    case $path in
      "~+"/*)
        path=$PWD/${path#"~+/"}
        ;;
      "~-"/*)
        path=$OLDPWD/${path#"~-/"}
        ;;
      "~"/*)
        path=$HOME/${path#"~/"}
        ;;
      "~"*)
        username=${path%%/*}
        username=${username#"~"}
        IFS=: read -r _ _ _ _ _ homedir _ < <(getent passwd "$username")
        if [[ $path = */* ]]; then
          path=${homedir}/${path#*/}
        else
          path=$homedir
        fi
        ;;
    esac
    resultPathElements+=( "$path" )
  done
  local result
  printf -v result '%s:' "${resultPathElements[@]}"
  printf '%s\n' "${result%:}"
}


## there's time we need to know reliably if we can run SUDO
SUDO_CMD="/usr/bin/sudo"
if [ -f "./dockerfile" ] ; then
    log_📢_记录 "🐳😁 found DOCKER"  
elif [ -f "$SUDO_CMD" ] ; then 
    log_📢_记录 "🥳 found sudo"  
else 
    log_📢_记录 "🐭 missed SUDO, try running _b00t_ inside docker."
    SUDO_CMD=""
fi
export SUDO_CMD

if [ -z "$(whereis crudini)" ] ; then 
    log_📢_记录 "🥳 need crudini to save data, installing now"  
    $SUDO_CMD apt-get install crudini
fi

##
export _user="$(id -u -n)" 
export _uid="$(id -u)" 
echo "🙇‍♂️ \$_user: $_user  \$_uid : $_uid"
