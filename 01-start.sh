#!/bin/bash

##* * * *\\ 
#* Purpose: 
#*   boot-strap common bash libraries
#*   access Azure
##* * * *//

# 🍰 https://stackoverflow.com/questions/192319/how-do-i-know-the-script-file-name-in-a-bash-script
# ------------- SCRIPT ------------- #
#echo
#echo "# arguments called with ---->  ${@}     "
#echo "# \$1 ---------------------->  $1       "
#echo "# \$2 ---------------------->  $2       "
#echo "# path to me --------------->  ${0}     "
#echo "# parent path -------------->  ${0%/*}  "
#echo "# my name ------------------>  ${0##*/} "
#echo
# ------------- CALLED ------------- #


## 小路 \\
## Xiǎolù :: Path or Directory
# THINGS YOU CAN EDIT: 
export _B00T_C0DE_Path="/c0de/_b00t_"        
export _B00T_C0NFIG_Path="$HOME/.b00t"
## 小路 //

## 记录 \\
## Jìlù :: Record (Log)
function log_📢_记录() {
    echo "$1"
}
export -f log_📢_记录
## 记录 //

## 进口 \\  
## Kāishǐ :: Start
# init should be run by every program. 
function _b00t_init_🥾_开始() {
    log_📢_记录 "🥾 init: ${0}/./${0##*/}"
    log_📢_记录 "🥾 args: ${@}"
}
export -f _b00t_init_🥾_开始
_b00t_init_🥾_开始
## 进口 //




## 加载 * * * * * *\\
## Jiāzài :: Load
function bash_source_加载() {
    file="$1"
    if [ ! -x "$file" ] ; then
        log_📢_记录 "missing $file" && exit 
    else
        log_📢_记录 "🤓 source $file"
        source "$file" 
        if [ $? -gt 0 ] ; then
            echo "☹️ $file had error. 🛑"
        fi
    fi

    return $?
}
export -f bash_source_加载


## 进口 * * * \\ 
## Jìnkǒu :: Import/Load

# Bin shell & helpers
bash_source_加载 "$_B00T_C0DE_Path/./bash.🔨/c0re_init.🥾.sh"

## 进口 >>
## minimal c0re Python 🐍
# + establish .venv
bash_source_加载 "$_B00T_C0DE_Path/./bash.🔨/c0re_init.🐍.sh"
source .venv/bin/activate

bash_source_加载 "$_B00T_C0DE_Path/./bash.🔨/c0re_init.🦄.sh"

bash_source_加载 "$_B00T_C0DE_Path/./bash.🔨/c0re_init.🐙.sh"

bash_source_加载 "$_B00T_C0DE_Path/./bash.🔨/c0re_init.🐳.sh"
## 进口 * * * // 

echo "stage 2 still in progress. "
exit

## 项目 * * * * \\  
# (Xiàngmù) Project Id

export c0re_pr0j3ct_id="moist_monkey"
##* * * * //


## !TODO: Do you need a project name?
## !TODO: Do we have an AZ tenant Id?
## 要不要　
## !TODO: Do you need a resource Group?
## !TODO: 
🐙

##* * * * \\
az_resGroupId=$(az group show --name $az_groupName --query id --output tsv)
# $echo groupId
# /subscriptions/{###}/resourceGroups/{groupName}
az ad sp create-for-rbac \
  # --scopes  # !TODO
  --scope $az_resGroupId --role Contributor \
  --name $az_projectId-🤴校长_principal \
  --sdk-auth
##* * * * //


##* * * * \\
# 目录 (Mùlù) Directory
if [ -d "$c0dePath/./pr0j3cts/./$project_dir" ] ; then
    export PROJECT_dirExist=`$c0dePath/./pr0j3cts/./$project_dir`
    echo "🥾 the $c0dePath/./pr0j3cts/./$project_dir already exists use --force"
else
    export PROJECT_dirExists=""
fi
mkdir -p "$c0dePath/./pr0j3cts/./$project"
##* * * * // 

##* * * * \\
## 怎么样 (zěnme yàng) Present,How & What
#* - $AZ_resourceGroup is set
#* - c0re-lib verifies az cli is installed 
source "./bash/AZ_CLI_init.🤖.sh"
##* * * * //

##* * * * \\ 
# 🤖 微软 Wēiruǎn (Microsoft) Zzure
# 🤖 微软 Azure Login, verify credentials
# az login --verbose
source "./bash/AZ_init.🤖.sh"
##* * * * //

##* * * * \\
source "./bash/AZ_todo.🤖.sh"
##* * * * //


#*  🐳 docker setup.


# TODO: link to the Elasticdotventures repository
# 
docker build -t cowsay .
# 🐳♻️ It’s a good habit to use --rm to avoid filling up your system with stale Docker containers.
docker run --rm cowsay 


🐛 If you didn't get a cowsay, let me know. 

🤓 at this point you can start to build using EV _b00t_ or 
your own _b00t_.  

type:
git clone https://github.com/elasticdotventures/_b00t_/generate

echo "* if you just saw a talking cow, everything is fine!"
echo "run ./02_t00ls_.sh

