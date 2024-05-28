#!/bin/bash

# this is the boot-cli (according to bats/test.bas)

# TODO: xauth list 
export DISPLAY=":10"
export XAUTHORITY=~/.Xauthority


export BASHRCPATH=$(expandPath "~/_b00t_/_b00t_.bashrc")
source $BASHRCPATH

## NOTE: this is for future use. 
exit 

##* * * *\\ 
#* Purpose: 
#*   cli _b00t_ interface, run by bin/b00t.sh or _b00t_.sh 
##* * * *//

_version="1.0.0"

if [ false ] ; then 
#if [ "$#" == 0 ] ; then  
#fi 

#case "$1" in 
#  '')
      
# save any positional arguments into for later
#PARAMS=""               
#while (( "$#" )); do
#  # eval the length of the args array and exits when zero
#  case "$1" in
#     # pass the first element in the arguments array through a 
#     # case statement looking for either a custom flag or some 
#     # default flag patterns
#    -V|--version)
#      echo $_version
#      # MY_FLAG=0
#      shift
#      ;;
#    -b|--my-flag-with-argument)
#      if [ -n "$2" ] && [ ${2:0:1} != "-" ]; then
#        MY_FLAG_ARG=$2
#        shift 2
#      else
#        echo "Error: Argument for $1 is missing" >&2
#        exit 1
#      fi
#      ;;
#    -*|--*=) # unsupported flags
#      echo "Error: Unsupported flag $1" >&2
#      exit 1
#      ;;
#    *) # preserve positional arguments
#      PARAMS="$PARAMS $1"
#      shift
#      ;;
#  esac
#done
# set positional arguments in their proper place
#eval set -- "$PARAMS"


# 问问题 wèntí - ask question. 
# if you want to disable specific features then change to 没有
cat >/dev/shm/_b00t_.yaml <<🏁
---
  _b00t_w41137_3RC20: "问题"
  _b00t_azur3: "问题"
🏁

# cat /dev/shm/_b00t_.yaml | yq | jq --arg a xyz '.bar=$a
while read -rd $'' line
do
    export "$line"
done < <(jq -r <<<"$values" \
         'to_entries|map("\(.key)=\(.value)\u0000")[]')

# convert yaml config to environment variables
# 🍰 https://stackoverflow.com/questions/48512914/exporting-json-to-environment-variables
export $(echo $values | jq -r "to_entries|map(\"\(.key)=\(.value|tostring)\")|.[]")


# export $(cat /dev/shm/_b00t_.yaml | yq | jq -r "to_entries|map(\"\(.key)=\(.value|tostring)\")|.[]")


echo $_b00t_w41137_3RC20
exit

## setup environment-vars
_b00t_W41137=$( crudini_get "b00t" "W41137_3RC20" )
if [ -z "$_b00t_W41137" ] ; then
  log_📢_记录 "🥯🙀 no erc20 wallet!, please provide 0x.... or enter to skip crypto"
  read _b00t_W41137
  if [ -n "$_b00t_W41137" ] ; then
      log_📢_记录 "🥯👽 no wallet set!"
      crudini_set "b00t" "W41137_3RC20" "没有"
  else 
      log_📢_记录 "🥯🥰 .env _b00t_W41137: $_b00t_W41137_3RC20"
      crudini_set "b00t" "W41137_3RC20" "$_b00t_W41137_3RC20"
  fi
fi 




## * * * * \\
# Get filename and --parameters
filename=$1
while getopts ":g:i:n:m:e:" arg; do
  case $arg in
    g) resourceGroup=$OPTARG;;
    i) ID=$OPTARG;;
    n) Name=$OPTARG;;
    m) Manufacturing_date=$OPTARG;;
    e) Expire_date=$OPTARG;;
  esac
done
echo -e "\n$ID $Name\n"
# ./bash-cmd.新.sh -g -i p001 -n 'Hot Cake' -m '01-01-2018' -e '06-01-2018'
## * * * * //

fi

## make a cifs share
#docker volume create \
#	--driver local \
#	--opt type=cifs \
#	--opt device=//uxxxxx.your-server.de/backup \
#	--opt o=addr=uxxxxx.your-server.de,username=uxxxxxxx,password=*****,file_mode=0777,dir_mode=0777 \
#	--name cif-volume

  
