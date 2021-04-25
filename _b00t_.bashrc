
# usage:
#   source "./_b00t_.bashrc"
#   may also *eventually* run via commandline. 

# https://misc.flogisoft.com/bash/tip_colors_and_formatting
# https://github.com/awesome-lists/awesome-bash


## 小路 \\
## Xiǎolù :: Path or Directory
# THINGS YOU CAN EDIT: 
export _B00T_C0DE_Path="/c0de/_b00t_"        
export _B00T_C0NFIG_Path="$HOME/.b00t"
## 小路 //


## 记录 \\
## Jìlù :: Record (Log)
# 🤓 write to a log if you want using >> 
function log_📢_记录() {
    echo "$1"
}
export -f log_📢_记录
## 记录 //

## 进口 \\  
## Kāishǐ :: Start
# init should be run by every program. 
function _b00t_init_🥾_开始() {
    # earlier versions, sunset: 
    #🌆 ${0}/./${0*/}"   
    #🌆 export _b00t_="$(basename $0)"
    export _b00t_="$0" 
    log_📢_记录 "🥾 init: $_b00t_"
    log_📢_记录 "🥾 args: ${@}"
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
