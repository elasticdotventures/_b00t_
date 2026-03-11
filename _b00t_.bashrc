#
# Purpose: universal bash b00t-strap for environment & tooling
#   once run in an enviroment will attempt to validate & construct
#   bash shortcuts, menus, etc. 
#


# usage:
#   source "./_b00t_.bashrc"
#   may also *eventually* run via commandline. 

# https://misc.flogisoft.com/bash/tip_colors_and_formatting
# https://github.com/awesome-lists/awesome-bash

# set -o errexit    # Used to exit upon error, avoiding cascading errors
set -o nounset    # Exposes unset variables, strict mode. 
trap "set +o nounset" EXIT  # restore nounset at exit, even in crash!

# 🤔 trial: 
umask 000


# mark variables which are modified or created for export
set -a 


## 小路 \\
## Xiǎolù :: Path or Directory
# THINGS YOU CAN EDIT: 
_B00T_C0DE_Path="/c0de/_b00t_"
if [ -d "$HOME/_b00t_" ] ; then 
    _B00T_C0DE_Path="$HOME/_b00t_"
fi 
export _B00T_C0DE_Path
export _B00T_C0NFIG_Path="$HOME/.b00t"
_b00t_INSPIRATION_FILE="$_B00T_C0DE_Path/./r3src_资源/inspiration.json"
## 小路 //



## 记录 \\
## Jìlù :: Record (Log)
# 🤓 write to a log if you want using >> 
# mostly, this is for future opentelemetry & storytime log
unset -f log_📢_记录
function log_📢_记录() {
    echo "$@"
}
export -f log_📢_记录

# ASCII-safe logger wrapper. Some shells/sessions may not retain unicode function names.
unset -f log_b00t
function log_b00t() {
    log_📢_记录 "$@"
}
export -f log_b00t

# Always repair logger symbols before loading optional modules.
unset -f ensure_b00t_log
function ensure_b00t_log() {
    if [ "$(LC_ALL=C type -t log_📢_记录 2>/dev/null)" != "function" ]; then
        function log_📢_记录() {
            echo "$@"
        }
    fi
    if [ "$(LC_ALL=C type -t log_b00t 2>/dev/null)" != "function" ]; then
        function log_b00t() {
            log_📢_记录 "$@"
        }
    fi
    export -f log_📢_记录
    export -f log_b00t
}
export -f ensure_b00t_log
ensure_b00t_log
## 记录 //

## this will allow b00t to restart itself. 
unset -f reb00t
function reb00t() {
    unset -f _b00t_init_🥾_开始
    log_📢_记录 "🥾 restarting b00t"
    source "$_B00T_C0DE_Path/_b00t_.bashrc"
}





## * * * * * \\
## pathAdd 
unset pathAdd
function pathAdd() {
    if [ -d "$1" ] && [[ ":$PATH:" != *":$1:"* ]]; then
        PATH="${PATH:+"$PATH:"}$1"
    fi
}
# webi tools
pathAdd "$HOME/.local/bin"
pathAdd "$HOME/.yarn/bin"
## * * * * * //

if [ "/usr/bin/docker" ] ; then 
    echo "🐳 has d0cker! loading docker extensions"
    source "$_B00T_C0DE_Path/docker.🐳/_bashrc.sh"

    ## 😔 docker context? 
    ## https://docs.docker.com/engine/context/working-with-contexts/
    # export DOCKER_CONTEXT=default
    # log_📢_记录 "🐳 CONTEXT: $DOCKER_CONTEXT"  
    # docker context ls

fi

## * * * * * \\
## is_version_大于
## compared #.#.# version (but could detect other formats)
## usage: is_version_大于 $requiredver $currentver
unset -f is_v3rs10n_大于
function is_v3rs10n_大于()   # $appversion $requiredversion
{
    # echo "hello $1 $2"
    # printf '%s\n%s\n' "$2" "$1" | sort --check=quiet --version-sort
    local appver=$1     # i.e. 1.0.0
    local requiredver=$2 # i.e. 1.0.1
    if [ "$(printf '%s\n' "$requiredver" "$appver" | sort -V | head -n1)" = "$requiredver" ]; then 
        # version is insufficient
        result="false"
    else
        # version is sufficient (大于 greater than equal)
        result="true"
    fi

    echo $result
    return 0
}
export -f is_v3rs10n_大于
## * * * * * * //


## 
# does a bash/function exist?
# 🍰 https://stackoverflow.com/questions/85880/determine-if-a-function-exists-in-bash
# returns: 
#     0 on yes/success (function defined/available)
#     1 for no (not available)
function has_fn_✅_存在() {
    local exists=$(LC_ALL=C type -t $1)
    # echo "exists: $exists"
    if [ -n "$exists" ] ; then 
        # exists, not empty, success
        return 0 
    fi 
    # fail (function does not exist)
    return 1
    # result=[[ -n "$exists" ]] || 1 
}
export -f has_fn_✅_存在




## All the logic below figures out who is calling & hot-reload
## bail earlier is better, 
_b00t_exists=`type -t "_b00t_init_🥾_开始"`
_b00t_VERSION_was=0
if [ "$_b00t_exists" == "function" ] ; then 
    # are we re-entry, i.e. reb00t
    export _b00t_VERSION_was="$_b00t_VERSION"
fi
# -------------- CONFIGURABLE SETTING -----------------
export _b00t_VERSION="1.0.15"
# -----------------------------------------------------

# syntax: current required
# echo "v3r: $_b00t_VERSION "
upgradeB00T=$(is_v3rs10n_大于 "$_b00t_VERSION_was" "$_b00t_VERSION")
# echo "upgradeB00T: $upgradeB00T"

# 🦨 need consent!
if [ "$upgradeB00T" ==  true ] && [ -n "$_b00t_VERSION_was" ] ; then 
    # welcome! (clean environment)
    log_📢_记录 "🥾🧐 b00t version | now: $_b00t_VERSION"
elif [ "$upgradeB00T" ==  true ] ; then 
    ## upgrade b00t in memory (this doesn't work awesome, but useful during dev)
    ## $ reb00t
    log_📢_记录 "🥾🧐 (re)b00t version | now: $_b00t_VERSION | was: $_b00t_VERSION_was | upgrade: $upgradeB00T"
    # TODO: consent
elif [ "$_b00t_exists" == "function" ] ; then 
    # SILENT, don't reload unless _b00t_VERSION is newer
    # short circuit using rand0() function 
    # log_📢_记录 "👻 short-circuit"
    set +o nounset 
    return
    ## 🍒 short circuit! 
fi

## Have FZF use fdfind "fd" by default
export PS_FORMAT="pid,ppid,user,pri,ni,vsz,rss,pcpu,pmem,tty,stat,args"
export FD_OPTIONS="--follow -exlude .git --exclude node_modules"

## OPINIONATED ALIASES

# ☁️ cloud -cli's
function az_cli () {
    # local args=("$@")
    docker run --rm -it -v $HOME/.azure:/root/.azure -v $(pwd):/root mcr.microsoft.com/azure-cli:latest az $@
}
alias az="az_cli"
alias aws='docker run --rm -it -v ~/.aws:/root/.aws -v $(pwd):/aws amazon/aws-cli'
alias gcp="docker run --rm -ti --name gcloud-config google/cloud-sdk gcloud "


alias ls='ls -F  --color=auto'
# pretty = ll
#-rw-rw-r-- 1 1000 1000  100 May  5 23:51 requirements.txt
#-rw-rw-r-- 1 1000 1000  144 May  5 20:01 requirements.层_b00t_.txt
#-rw-rw-r-- 1 1000 1000  221 Apr 25 20:27 requirements.层_c0re.txt
alias ll='ls -lh'

#4.0K src/
#4.0K requirements.层_test.txt
#4.0K requirements.层_c0re.txt
alias lt='ls --human-readable --size -1 -S --classify'


alias s=ssh
alias c=clear
alias cx='chmod +x'
alias myp='ps -fjH -u $USER'

#mcd() { mkdir -p $1; cd $1 }
#export -f mcd
#cdl() { cd $1; ls}
#export -f cdl

# FUTURE: 
# https://github.com/GochoMugo/msu

# TODO: test for pipx
# 
if [ -n "$(whereis register-python-argcomplete3)" ] ; then 
    echo "🦨++ installing python3-argcomplete + pipx"
    sudo apt install python3-argcomplete pipx -y
fi 
if [ -n "$(whereis register-python-argcomplete3)" ] ; then 
    eval "$(register-python-argcomplete3 pipx)"
    # pipx run
fi 

# bat - a pretty replacement for cat.
alias bat="batcat"

# bats - bash testing system (in a docker container)
# 🦨 need consent before running docker
# 🦨 this is not the proper way to run bats. 
# alias bats='docker run -it -v bats/bats:latest'

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
alias yyyymmdd="date +'%Y%m%d'"
alias ymd="date +'%Y%m%d'"
alias ymd_hm="date +'%Y%m%d.%H%M'"
alias ymd_hms="date +'%Y%m%d.%H%M%S'"
##################




# order of magnitude
#function oom () {
#    # todo: detect an order of magnitude transition. 
#}





## 进口 \\  
## Kāishǐ :: Start
# init should be run by every program. 
# this is mostly here for StoryTime and future hooks. 
unset -f _b00t_init_🥾_开始
function _b00t_init_🥾_开始() {
    local args=("$@")
    local param=${args[0]}

#    if [ param="version" ] ; then
#        echo "🥾v: $currentB00TVersion"
#    fi 
    
    # earlier versions, sunset: 
    #🌆 ${0}/./${0*/}"   
    #🌆 export _b00t_="$(basename $0)"
    export _b00t_="$0" 

    if [ $_b00t_ == "/c0de/_b00t_/_b00t_.bashrc" ] ; then
        log_📢_记录 ""
        log_📢_记录 "usage: source /c0de/_b00t_/_b00t_.bashrc"
        exit 
    fi

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


    log_📢_记录 "🥾 -V: $_b00t_VERSION  init: $_b00t_"
    if [ -n "${@}" ] ; then 
        log_📢_记录 "🥾 args: ${@}"  
    fi 
}
export -f _b00t_init_🥾_开始
#_b00t_init_🥾_开始

## 进口 //


# alpine container support
# https://github.com/ethereum/solidity/issues/875
# returns 0 for "true" (not alpine linux), non-zero for false (is alpine linux)
function iz_n0t_alpine_linux_🐧🌲() {
   return $(cat /etc/os-release | grep "NAME=" | grep -ic "Alpine")
}
if [ ! iz_n0t_alpine_linux ] ; then
    # gh issue 
    echo "🥾🤮 🐧🌲 alpine linux not fully supported yet"
fi 


# this is intended to catch & report errors
function barf_🤮 () {
    gh issue create
    # gh issue create --title $1
}



# Webi, presently breaks alpine config! 
# https://github.com/elasticdotventures/webi-installers
webi=$(whereis webi)
if [ -z "$webi" ] ; then 
    curl https://webinstall.dev/webi | bash
    # Should install to $HOME/.local/opt/<package>-<version> or $HOME/.local/bin
    # Should install to $HOME/.local/opt/<package>-<version> or $HOME/.local/bin
    # Should not need sudo (except perhaps for a one-time setcap, etc) 
fi 


#
# Returns seconds until a relative time i.e. "today 4pm"
#
function secondsUntil () {
    # pass "today 5:25pm"
    # same "today 17:25"
    # .. "tomorrow"
    WHEN=$1
    echo $(( $(date +%s -d "$WHEN") - $( date +%s ) ))
}


## 加载 * * * * * *\\
## Jiāzài :: Load
unset -f bash_source_加载
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

##
## Modular bashrc drop-in loader
## Source readable *.sh files from colon-separated directories.
##
unset -f _b00t_source_modular_bashrc
function _b00t_source_modular_bashrc() {
    local dir=""
    local file=""
    local start_ms=0
    local end_ms=0
    local elapsed_ms=0
    local old_ifs="$IFS"
    local -a dirs=()

    IFS=':' read -r -a dirs <<< "${_B00T_BASH_MODULE_DIRS:-}"
    IFS="$old_ifs"

    for dir in "${dirs[@]}"; do
        [ -n "$dir" ] || continue
        [ -d "$dir" ] || continue

        shopt -s nullglob
        for file in "$dir"/*.sh; do
            [ -r "$file" ] || continue
            case "$(basename "$file")" in
                .*|_*) continue ;;
            esac

            ensure_b00t_log
            if [ "${B00T_BASH_PROFILE:-0}" = "1" ]; then
                start_ms="$(date +%s%3N)"
            fi

            source "$file"

            if [ "${B00T_BASH_PROFILE:-0}" = "1" ]; then
                end_ms="$(date +%s%3N)"
                elapsed_ms="$((end_ms - start_ms))"
                log_b00t "🥾 rc.d loaded: $file (${elapsed_ms}ms)"
            fi
        done
        shopt -u nullglob
    done
}
export -f _b00t_source_modular_bashrc



## 好不好 \\
## Hǎo bù hǎo :: Good / Not Good 
## is_file readable? 
# n0t_file_📁_好不好 result: 
#   0 : file is okay
#   1 : file is NOT okay
## if passed two or more files, will try all.
function n0ta_xfile_📁_好不好() {
    local args=("$@")
    xfile=${args[0]}
    if [ $# -gt 1 ] ; then
        # more than one file. try many
        while [ ! -x "$xfile" ] && [ "$#" -gt 1 ] ; do 
            shift
            xfile=$1
        done
    fi 
    
    if [ ! -f "$xfile" ] ; then
        log_📢_记录 "👽:不支持 $xfile is both required AND missing. 👽:非常要!"
        return 0
    elif [ ! -x "$xfile" ] ; then
        log_📢_记录 "👽:不支持 $xfile is not executable. 👽:非常要!"
        return 0
    else

        if ! has_fn_✅_存在 "crudini_get" ; then 
            :   # crudini_get doesn't exist.
        elif [[ $( crudini_get "b00t" "has.$xfile" ) = "" ]] ; then 
            log_📢_记录 "👍 $xfile"
            crudini_set "b00t" "has.$xfile" $( yyyymmdd )
        fi
        return 1
    fi
}
## 好不好 // 

## future artificat, 
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
#  if is_n0t_aliased "az" ; then echo "true - not aliased!"; else echo "false"; fi
function is_n0t_aliased() {
    local args=("$@")
    local hasAlias=${args[0]}
    local exists=$(alias -p | grep "alias $hasAlias=")
    # echo "exists: $exists"
    if [ -z "$exists" ] ; then
        # 🙄 exists: alias fd='/usr/bin/fdfind'
        return 0;  # "true", unix success
    else 
        return 1;  #  "false", unix error
    fi
}


##
## A pretty introduction to the system. 
##
function motd() {
    # count motd's
    # 🍰 https://unix.stackexchange.com/questions/485221/read-lines-into-array-one-element-per-line-using-bash
    SED_PATH=$(whereis -b sed | cut -f 2 -d ' ')

    if is_n0t_aliased "fd" ; then
        # no fd, incomplete environemnt
        log_📢_记录 "🥾🍒 No fd alias, incomplete environment"
        if [ ! -f "/tmp/motd.txt" ] ; then 
            printf "\nb00t basic motd. generated %s\n\n" $(ymd_hms) > "/tmp/motd.txt"
        fi 
        motdz=('/tmp/motd.txt')
    else 
        readarray -t motdz < <(/usr/bin/fdfind .txt "$_B00T_C0DE_Path/./ubuntu.🐧/etc/")
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

    local glitchCMDz=''
    if [ $(rand0 10) -gt 1 ] ; then
        glitchCMDz="$glitchCMDz | $SED_PATH 's/1/0/g' "
    fi
    #if [ $(rand0 10) -gt 5 ] ; then
    #    glitchCMDz=" | $SED_PATH 's/0/1/g' $glitchCMDz"
    #fi
    #if [ $(rand0 10) -gt 5 ] ; then
    #    glitchCMDz=" | $SED_PATH 's/8/🥾/g' $glitchCMDz"
    #fi

    #if [ $motdLength -gt $(echo $(tput rows) - 3 | bc) ] ; then
    #    showWithCMD="cat"
    #fi 

    if [ -n "$showWithCMD" ] ; then
        motdTmpFile=$( mktemp "_b00t_.日$(ymd).一时XXXXXXXXXX.motd" )
        # echo "motdFile: $motdTmpFile"
        # echo $(rand0 10)
        ## glitch effects 
        cp -v ${motdz[motdzQ]} $motdTmpFile
        if [ $(rand0 10) -gt 5 ] ; then
            $SED_PATH -i 's/1/0/g' $motdTmpFile
            $SED_PATH -i 's/8/🥾/g' $motdTmpFile
        fi 
        if [ $(rand0 10) -gt 5 ] ; then
            $SED_PATH -i 's/\*/🥾/g' $motdTmpFile
            $SED_PATH -i 's/[\!\-\@]./😁/g' $motdTmpFile
        fi
        if [ $(rand0 10) -gt 5 ] ; then
            $SED_PATH -i 's/#/_/g' $motdTmpFile
            $SED_PATH -i 's/0/🐛/g' $motdTmpFile
        fi 
        if [ $(rand0 10) -gt 5 ] ; then
            $SED_PATH -i 's/1/l/g' $motdTmpFile
            $SED_PATH -i 's/[\@l\#]/🐛/g' $motdTmpFile
        fi 
        $showWithCMD $motdTmpFile
        /bin/rm -f $motdTmpFile
    fi

    # part of motd

    log_📢_记录 "lang: $LANG"
    log_📢_记录 "🥾📈 motd project stats, cleanup, tasks goes here. "


    if [ -d "./.git" ] ; then 
        log_📢_记录 "🥾🐙😁 found .git repo"
        # github client 
        gh issue list

        local skunk_x=$(git grep "🦨" | wc -l)
        log_📢_记录 "🦨: $skunk_x"
    else 
        log_📢_记录 "🥾🐙😔 no .git dir "`pwd`
    fi 

}

if [ "${container+}" == "docker" ] ; then
    motd
elif ! is_n0t_aliased fd ; then 
    motd
else 
    motd
fi





# export FZF_COMPLETION_OPTS='--border --info=inline'
if ! n0ta_xfile_📁_好不好 "/usr/bin/fdfind"  ; then
    # export FZF_DEFAULT_COMMAND="git ls-files --cached --others --exclude-standard | /usr/bin/fdfind --type f --type l $FD_OPTIONS"
    # export FZF_DEFAULT_OPTS="--no-mouse --height 50% -1 --reverse --multi --inline-info --preview=[[ \$file --mine{}) =~ binary ]] && echo {} is a binary file || (bat --style=numbers --color=always {} || cat {}) 2> /dev/null | head -300' --preview-window='right:hidden:wrap' --bind'f3:execute(bat --style=numbers {} || less -f {}),f2:toggle-preview,ctrl-d:half-page-down,ctrl-u:half-page-up,ctrl-a:select-all+accept,ctrl-y:execute-silent(echo {+} | pbcopy)'"
    # from video: https://www.youtube.com/watch?v=qgG5Jhi_Els
    export FZF_DEFAULT_COMMAND="/usr/bin/fdfind --type f"
fi

####
# CRUDINI examples
# 🤓 https://github.com/pixelb/crudini/blob/master/EXAMPLES
# CRUDINI is used to store b00t config:

if n0ta_xfile_📁_好不好 "/usr/bin/crudini" ; then 
    log_📢_记录 "🥳 need crudini to save data, installing now"  
    $SUDO_CMD apt-get install -y crudini bc
fi

## CRUDINI helper functions:
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

    #if [[ "$#" -ne "2" ]] ; then 
    #    log_📢_记录 "crudini_get topic key"
    #    exit 0 
    # fi 

    local topic=${args[0]}
    local key=${args[1]}
    echo $( crudini --get "$CRUDINI_CFGFILE" "${topic}" "${key}" )
    return $?
}

# _seq: get a number from a sequence in b00t
function crudini_seq() {
    local args=("$@")
    local seqlabel=${args[0]}
    
    local x=$( crudini_get "b00t" "$seqlabel" )
    if [ -z "$x" ] ; then x="0"; fi 
    x=$(echo "$x" + 1 | bc)
    crudini_set "b00t" "$seqlabel" "$x"
    echo $x
    return 0
}

# verify integrity of crudini system
function crudini_init() {
    export CRUDINI_CFGFILE=$(expandPath "~/.b00t/config.ini")
    local CRUDINI_DIR=`dirname $CRUDINI_CFGFILE`
    if [ ! -d "$CRUDINI_DIR" ] ; then
        log_📢_记录 "🐭 no local $CRUDINI_CFGFILE"  
        log_📢_记录 "🐭🥳 local dir $CRUDINI_DIR"  
        if [ ! -d "$CRUDINI_DIR" ] ; then
            log_📢_记录 "🐭 creating CRUDINI dir $CRUDINI_DIR"  
            /bin/mkdir -p $CRUDINI_DIR
            /bin/chmod 750 $CRUDINI_DIR
            log_📢_记录 "🐭 init CRUDINI file $CRUDINI_CFGFILE"  
            crudini --set $CRUDINI_CFGFILE '_seq' "1"
        else        
            #local x=$( crudini_get "b00t" "crudini_check" )
            # x=$( [ -z "$x" ] && echo "0" )
            local x=$( crudini_seq "crudini_check" )
            log_📢_记录 "🐭😃CRUDINI _seq: #$x dir: $CRUDINI_DIR existed."
        fi
    fi
    return 0
}
#  creates an export for $CRUDINI_CFGFILE
crudini_init

function crudini_ok () {
if [ -f $CRUDINI_CFGFILE ] ; then 
    x=$( crudini_seq "crudini_check" )
    log_📢_记录 "🐭🥾 CRUDINI _seq: #$x $CRUDINI_CFGFILE"
    return 0
else 
    log_📢_记录 "🐭🍒 CRUDINI br0ked. file: $CRUDINI_CFGFILE"
    # todo: maybe some failsafe, i.e. redis or something. 
    return 1
fi
}
crudini_ok


##
## there's time we need to know reliably if we can run SUDO
##
function has_sudo() {
    SUDO_CMD="/usr/bin/sudo"

    ## 🦨 TODO: ask for consent to run sudo? 

    if [ "$EUID" -eq 0 ] ; then
        # r00t doesn't require sudo 
        # https://stackoverflow.com/questions/18215973/how-to-check-if-running-as-root-in-a-bash-script
        log_📢_记录 "👹 please don't b00t as r00t"
        SUDO_CMD=""
    elif [ -f "./dockerenv" ] ; then
        # https://stackoverflow.com/questions/23513045/how-to-check-if-a-process-is-running-inside-docker-container#:~:text=To%20check%20inside%20a%20Docker,%2Fproc%2F1%2Fcgroup%20.
        log_📢_记录 "🐳😁 found DOCKER"  
    elif [ -f "$SUDO_CMD" ] ; then 
        if [[ -z $( crudini_get "b00t" "has.sudo" )  ]] ; then 
            log_📢_记录 "🥳 found sudo"  
            crudini_set "b00t" "has.sudo" `ymd_hms`
        fi 
    else 
        log_📢_记录 "🐭 missed SUDO, try running _b00t_ inside docker."
        SUDO_CMD=""
    fi
    export SUDO_CMD
}
has_sudo 

# Modular drop-ins for shell customization without editing core bootstrap.
export _B00T_BASH_MODULE_DIRS="${_B00T_BASH_MODULE_DIRS:-$_B00T_C0DE_Path/bash.🐚/rc.d:$HOME/.b00t/bashrc.d:$HOME/.bashrc.d}"
_b00t_source_modular_bashrc



#############################
###
# 🍰 https://superuser.com/questions/427318/test-if-a-package-is-installed-in-apt
#if debInst "$1"; then
#    printf 'Why yes, the package %s _is_ installed!\n' "$1"
#else
#    printf 'I regret to inform you that the package %s is not currently installed.\n' "$1"
#fi
function debInst() {
    dpkg-query -Wf'${db:Status-abbrev}' "$1" 2>/dev/null | grep -q '^i'
}

if debInst "moreutils" ; then
    # only show moreutils once. 
    if [ $( crudini_get "b00t" "has.moreutils" ) -eq "0" ] ; then 
        log_📢_记录 "👍 debian moreutils is installed!"
        crudini_set "b00t" "has.moreutils" $(yyyymmdd)
    fi 
else
    log_📢_记录  "😲 install moreutils (required)"
    $SUDO_CMD apt-get install -y moreutils
fi






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
