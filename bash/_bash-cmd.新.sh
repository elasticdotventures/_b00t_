#!/bin/bash

## * * * *// 
#* Purpose: copy me, edit this. 
#*
## * * * *//

#* 进口 (Jìnkǒu) c0re Libraries Alpha
if [ ! -x "./bash/c0re-lib.sh" ] ; then
    echo "missing ./bash/c0re-lib.sh" && exit 
else
    source "./bash/c0re-lib.sh" 
fi


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
echo -e "\n$ID  $Name   $Manufacturing_date $Expire_date\n"

# bash-cmd.sh -g -i p001 -n 'Hot Cake' -m '01-01-2018' -e '06-01-2018'

function_name () {
  commands
}
