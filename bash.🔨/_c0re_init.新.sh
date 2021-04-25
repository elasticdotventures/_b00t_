#!/bin/bash

## * * * * * * * * * * * \\
#*
#* Purpose: ⚠️ THIS IS AN EXAMPLE/TEMPLATE! (code in here doesn't run)
#*
## * * * * * * * * * * * //

#* 进口 (Jìnkǒu) 🚀 *ALWAYS* load c0re Libraries!
# should be run by _b00t_
if [ `type -t "_b00t_init_🥾_开始"` == "function" ]; then 
    # detect _b00t_ environment 
    _b00t_init_🥾_开始
fi


## * * * * \\
# Example Function
my_function () {
  echo "some result"
  return 55
}
my_function
echo $?
## * * * * //


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


function_name () {
  commands
}



