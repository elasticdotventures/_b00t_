
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
_b00t_INSPIRATION_FILE="$_B00T_C0DE_Path/./r3src_资源/inspiration.json"
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


##* * * * * *\\
## 📽️ Pr0J3ct1D
## uses inspiration
##* * * * * *//
function Pr0J3ct1D {
    local wordCount=$( cat $_b00t_INSPIRATION_FILE | jq '. | length' )
    # echo "wordCount: $wordCount"

    local word1=$( rand0 $wordCount )
    # echo "word1: $word1"
    local wordOne=$( cat $_b00t_INSPIRATION_FILE | jq ".[$word1].word" -r )
    local word2=$( rand0 $wordCount )
    # echo "word2: $word2"
    local wordTwo=$( cat $_b00t_INSPIRATION_FILE | jq ".[$word2].word" -r )
    local result="${wordOne}_${wordTwo}"

    ## todo: substitute 
    if [ $( rand0 10 ) -lt 5 ] ; then 
        result=$( echo $result | sed 's/l/1/g' )
    fi

    if [ $( rand0 10 ) -lt 2 ] ; then 
        result=$( echo $result | sed 's/o/0/g' )
    elif [ $( rand0 10 ) -lt 2 ] ; then 
        result=$( echo $result | sed 's/oo/00/g' )
    fi

    if [ $( rand0 10 ) -lt 2 ] ; then 
        result=$( echo $result | sed 's/e/3/g' )
    elif [ $( rand0 10 ) -lt 8 ] ; then 
        result=$( echo $result | sed 's/ee/33/g' )
    fi

    # Todo: fix first letter is a number. naming issue.

    echo $result
    return 0
}



##* * * * * *\\
## generates a random number between 0 and \$1
# usage: 
# rand0_result="$(rand0 100)"
# echo \$rand0_result

function rand0() {
    local args=("$@")
    local max=${args[0]}
    rand0=$( bc <<< "scale=2; $(printf '%d' $(( $RANDOM % $max)))" ) ;
    # rand0=$( echo $RANDOM % $max ) ; 
    echo $rand0
}

##* * * * * *//


## there's time we need to know reliably if we can run SUDO
function has_sudo() {
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
}
has_sudo()

if [ -z "$(whereis crudini)" ] ; then 
    log_📢_记录 "🥳 need crudini to save data, installing now"  
    $SUDO_CMD apt-get install crudini
fi

## Have FZF use fdfind "fd" by default
export PS_FORMAT="pid,ppid,user,pri,ni,vsz,rss,pcpu,pmem,tty,stat,args"
export FD_OPTIONS="--follow -exlude .git --exclude node_modules"
# export FZF_COMPLETION_OPTS='--border --info=inline'
if ! n0ta_xfile_📁_好不好 "/usr/bin/fdfind"  ; then
    # export FZF_DEFAULT_COMMAND="git ls-files --cached --others --exclude-standard | /usr/bin/fdfind --type f --type l $FD_OPTIONS"
    # export FZF_DEFAULT_OPTS="--no-mouse --height 50% -1 --reverse --multi --inline-info --preview=[[ \$file --mine{}) =~ binary ]] && echo {} is a binary file || (bat --style=numbers --color=always {} || cat {}) 2> /dev/null | head -300' --preview-window='right:hidden:wrap' --bind'f3:execute(bat --style=numbers {} || less -f {}),f2:toggle-preview,ctrl-d:half-page-down,ctrl-u:half-page-up,ctrl-a:select-all+accept,ctrl-y:execute-silent(echo {+} | pbcopy)'"
    # from video: https://www.youtube.com/watch?v=qgG5Jhi_Els
    export FZF_DEFAULT_COMMAND="/usr/bin/fdfind --type f"
fi


# CRUDINI is used to store b00t config:
# 
# CRUDINI examples
# 🤓 https://github.com/pixelb/crudini/blob/master/EXAMPLES
export CRUDINI_CFGFILE=$(expandPath "~/.b00t/config.ini")
if [ ! -d $CRUDINI_CFGFILE ] ; then
    log_📢_记录 "🐭 no local $CRUDINI_CFGFILE"  
    CRUDINI_DIR=`dirname $CRUDINI_CFGFILE`
    log_📢_记录 "🥳 local dir $CRUDINI_DIR"  
    if [ ! -d "$CRUDINI_DIR" ] ; then
        log_📢_记录 "🧝 creating CRUDINI dir $CRUDINI_DIR"  
        /bin/mkdir -p $CRUDINI_DIR
        /bin/chmod 750 $CRUDINI_DIR
        crudini --set $CRUDINI_CFGFILE '_syntax' "1"
    else
        log_📢_记录 "😃 CRUDINI local dir $CRUDINI_DIR exists"
    fi
fi

function crudini_set() {
    local args=("$@")
    local topic=${args[0]}
    local key=${args[1]}
    local value=${args[2]}
    crudini --set $CRUDINI_CFGFILE "${key}" "${value}"
    return $?
}

function crudini_get() {
    local args=("$@")
    local topic=${args[0]}
    local key=${args[1]}
    echo $( crudini --get $CRUDINI_CFGFILE "${key}" )
    return $?
}

##
export _user="$(id -u -n)" 
export _uid="$(id -u)" 
echo "🙇‍♂️ \$_user: $_user  \$_uid : $_uid"

