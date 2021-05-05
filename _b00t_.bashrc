
# usage:
#   source "./_b00t_.bashrc"
#   may also *eventually* run via commandline. 

# https://misc.flogisoft.com/bash/tip_colors_and_formatting
# https://github.com/awesome-lists/awesome-bash

# set -o errexit    # Used to exit upon error, avoiding cascading errors
set -o nounset    # Exposes unset variables, strict mode. 
trap "set +o nounset" EXIT  # restore nounset at exit, even in crash!

set -a # mark variables whcih are modified or created for export
## 小路 \\
## Xiǎolù :: Path or Directory
# THINGS YOU CAN EDIT: 
export _B00T_C0DE_Path="/c0de/_b00t_"        
export _B00T_C0NFIG_Path="$HOME/.b00t"
_b00t_INSPIRATION_FILE="$_B00T_C0DE_Path/./r3src_资源/inspiration.json"
## 小路 //


## Have FZF use fdfind "fd" by default
export PS_FORMAT="pid,ppid,user,pri,ni,vsz,rss,pcpu,pmem,tty,stat,args"
export FD_OPTIONS="--follow -exlude .git --exclude node_modules"

## OPINIONATED ALIASES

alias ls='ls -F  --color=auto'
alias ll='ls -lh'
alias lt='ls --human-readable --size -1 -S --classify'


alias s=ssh
alias c=clear
alias cx='chmod +x'
alias myp='ps -fjH -u $USER'

#mcd() { mkdir -p $1; cd $1 }
#export -f mcd
#cdl() { cd $1; ls}
#export -f cdl

# bat - a pretty replacement for cat.
alias bat="batcat"

# count the files in a directory or project
alias count='find . -type f | wc -l'
# copy verbose, see rsync. or use preferred backup command. 
alias cpv='rsync -ah --info=progress2'

# cd to _b00t_ (or current repo)
alias cg='cd `git rev-parse --show-toplevel`'
alias ..='cd ..'
alias ...='cd ../../'
alias mkdir='mkdir -pv'

# time & date
alias path='echo -e ${PATH//:/\\n}'
alias now='date +"%T"'
alias nowtime=now
alias nowdate='date +"%d-%m-%Y"'

# 🐙 git 
alias gitstatus='git -C . status --porcelain | grep "^.\w"'

# 🐍 Python ve = create .venv, va = activate!
alias ve='python3 -m venv ./venv'
alias va='source ./venv/bin/activate'

# use fzf to find a file and open it in vs code
alias c='code $(fzf --height 40% --reverse)'

# fd is same-same like unix find, but alt-featureset
# for example - fd respects .gitignore (but output like find)
if [ -f "/usr/bin/fdfind" ] ; then
    alias fd="/usr/bin/fdfind"
fi

# handy for generating dumps, etc..
# $ script.sh >> foobar.`ymd`
alias ymd="date +'%Y%m%d'"
alias ymd_hm="date +'%Y%m%d.%H%M'"
alias ymd_hms="date +'%Y%m%d.%H%M%S'"

##################

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

# order of magnitude
#function oom () {
#    # todo: detect an order of magnitude transition. 
#}

## 进口 \\  
## Kāishǐ :: Start
# init should be run by every program. 
# this is mostly here for StoryTime
function _b00t_init_🥾_开始() {
    # earlier versions, sunset: 
    #🌆 ${0}/./${0*/}"   
    #🌆 export _b00t_="$(basename $0)"
    export _b00t_="$0" 

    local PARENT_COMMAND_STR="👽env-notdetected"
    if [ $PPID -eq 0 ] ; then
        if [ "$container" == "docker" ] ; then
            PARENT_COMMAND_STR="🐳 d0ck3r!"
        
        else 
            PARENT_COMMAND_STR="👽env-unknown"
        fi
    else 
        # lookup parent application by process id. 
        PARENT_COMMAND_STR=$(ps -o comm=$PPID)
    fi

    if [ "$PARENT_COMMAND_STR" == "bash" ] ; then
        # most common case can be summarized
        log_📢_记录 "🥾👵:🔨"
    else 
        log_📢_记录 "🥾👵 from: $PARENT_COMMAND_STR"
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
## checks to see if an alias has been defined. 
function is_n0t_aliased() {
    local args=("$@")
    local hasAlias=${args[0]}
    exists=$(alias -p | grep "alias $hasAlias=")
    if [ -z "$exists" ] ; then
        return 0;
    else 
        return 1;
    fi
}

##
## A pretty introduction to the system. 
##
function motd() {
    # count motd's
    # 🍰 https://unix.stackexchange.com/questions/485221/read-lines-into-array-one-element-per-line-using-bash
    if is_n0t_aliased "fd" ; then
        # no fd, incomplete environemnt
        log_📢_记录 "🥾🍒 No fd alias, incomplete environment"
        if [ ! -f "/tmp/motd.txt" ] ; then 
            printf "\nb00t basic motd. generated %s\n\n" $(ymd_hms) > "/tmp/motd.txt"
        fi 
        motdz=('/tmp/motd.txt')
    else 
        readarray -t motdz < <(fd .txt 'ubuntu.🐧/etc/')
    fi
    local motdzQ=$( rand0 ${#motdz[@]} )
    # declare -p motdz

    local showWithCMD="/usr/bin/batcat"
    
    f=${motdz[motdzQ]}
    local motdWidth=$(awk 'length > max_length { max_length = length; longest_line = $0 } END { print max_length }' $f)
    # local motdWidth=$(cat "${motdz[motdzQ]}" | tail -n 1)
    local motdLength=$(cat $f | wc -l)

    local myWidth=$(tput cols)
    local myHeight=$(tput rows)
    if [ -z "$myHeight" ] ; then myHeight='🍒😑' ; fi
    log_📢_记录 "🥾🖥️ motd .cols: $motdWidth  .rows:$motdLength"
    log_📢_记录 "🤓🖥️ user .cols: $myWidth  .rows:$myHeight"
        
    if [ $motdWidth -gt "$myWidth" ] ; then 
        echo "👽:太宽 bad motd. too wide."
        showWithCMD=""
    elif [ $motdWidth -gt $(echo $myWidth - 13 | bc) ] ; then
        # bat needs +13 columns
        showWithCMD="cat"
    else
        # *auto*, full, plain, changes, header, grid, numbers, snip.
        showWithCMD="batcat --pager=never --style=plain --wrap character"
        if [ $(rand0 100) -gt 69 ] ; then 
            showWithCMD="batcat --pager=never --wrap character"
        fi
    fi
    # if it's still too big, try again!

    ## sometimes, cat is nice!
    if [ -z "$showWithCMD" ] ; then
        echo "👽💩: 烂狗屎 cannot motd."
    elif [ "$(rand0 10)" -gt 5 ] ; then 
        showWithCMD="cat"
    fi 

    #if [ $motdLength -gt $(echo $(tput rows) - 3 | bc) ] ; then
    #    showWithCMD="cat"
    #fi 

    if [ -n "$showWithCMD" ] ; then
        $showWithCMD ${motdz[motdzQ]}
    fi
    
    log_📢_记录 "🥾📈 FYTYRE goes here. "

    # echo ${#arr[@]}
    #  
}

if [ "${container+}" == "docker" ] ; then
    motd
elif ! is_n0t_aliased fd ; then 
    motd
fi



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
has_sudo 



# export FZF_COMPLETION_OPTS='--border --info=inline'
if ! n0ta_xfile_📁_好不好 "/usr/bin/fdfind"  ; then
    # export FZF_DEFAULT_COMMAND="git ls-files --cached --others --exclude-standard | /usr/bin/fdfind --type f --type l $FD_OPTIONS"
    # export FZF_DEFAULT_OPTS="--no-mouse --height 50% -1 --reverse --multi --inline-info --preview=[[ \$file --mine{}) =~ binary ]] && echo {} is a binary file || (bat --style=numbers --color=always {} || cat {}) 2> /dev/null | head -300' --preview-window='right:hidden:wrap' --bind'f3:execute(bat --style=numbers {} || less -f {}),f2:toggle-preview,ctrl-d:half-page-down,ctrl-u:half-page-up,ctrl-a:select-all+accept,ctrl-y:execute-silent(echo {+} | pbcopy)'"
    # from video: https://www.youtube.com/watch?v=qgG5Jhi_Els
    export FZF_DEFAULT_COMMAND="/usr/bin/fdfind --type f"
fi


# CRUDINI is used to store b00t config:
if n0ta_xfile_📁_好不好 "/usr/bin/crudini" ; then 
    log_📢_记录 "🥳 need crudini to save data, installing now"  
    $SUDO_CMD apt-get install -y crudini bc
fi



function crudini_set() {
    local args=("$@")
    local topic=${args[0]}
    local key=${args[1]}
    local value=${args[2]}
    crudini --set $CRUDINI_CFGFILE "${topic}" "${key}" "${value}"
    return $?
}

function crudini_get() {
    local args=("$@")
    local topic=${args[0]}
    local key=${args[1]}
    echo $( crudini --get "$CRUDINI_CFGFILE" "${topic}" "${key}" )
    return $?
}


# CRUDINI examples
# 🤓 https://github.com/pixelb/crudini/blob/master/EXAMPLES
function crudini_init() {
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
            #local x=$( crudini_get "b00t" "crudini_check" )
            # x=$( [ -z "$x" ] && echo "0" )
            local x=$( crudini_get "b00t" "crudini_check" )
            if [ -z "$x" ] ; then x="0"; fi 
            x=$(echo "$x" + 1 | bc)
            crudini_set "b00t" "crudini_check" "$x"
            log_📢_记录 "😃 #$x CRUDINI local dir $CRUDINI_DIR exists"

        fi
    fi
}
# does not need an export. 
crudini_init 
#motd

# 60秒 Miǎo seconds
# 3分钟 Fēnzhōng minutes
# 3小时 Xiǎoshí seconds

export _b00t_JS0N_filepath=$(expandPath "~/.b00t/config.json")

#function jqAddConfigValue () {
#    echo '{ "names": ["Marie", "Sophie"] }' |\
#    jq '.names |= .+ [
#        "Natalie"
#    ]'   
#}

## ???
## https://docs.docker.com/engine/context/working-with-contexts/
#export DOCKER_CONTEXT=default
#log_📢_记录 "🐳 CONTEXT: $DOCKER_CONTEXT"  
#docker context ls


# 🍰 https://lzone.de/cheat-sheet/jq
#function jqSetConfigValue () {
#
#    echo '{ "a": 1, "b": 2 }' |\
#jq '. |= . + {
#  "c": 3
#}'
#}


##
export _user="$(id -u -n)" 
export _uid="$(id -u)" 
echo "🙇‍♂️ \$_user: $_user  \$_uid : $_uid"
set +o nounset 

