#!/bin/bash


## * * * * * * * * * * * \\
#*
#* Purpose: ⚠️ 
#* THIS IS AN EXAMPLE/TEMPLATE! (code in here doesn't run)
#* 
## * * * * * * * * * * * //

#Q: When should you use Bash arrays instead of other 
#scripting languages such as Python?
#A: Dependencies vs. Size & Execution Speed
 
#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "$_B00T_C0DE_Path/_b00t_.bashrc"

# README / bash scripting tips: 

# Bash Cheat Sheet:
# https://devhints.io/bash

# bash !$ notes: 
# It's worth nothing the distinction between this !$ token 
# and the special shell variable $_. Indeed, both expand to 
# the last argument of the previous command. However, !$ is 
# expanded during history expansion, while $_ is expanded 
# during parameter expansion. One important consequence of 
# this is that, when you use !$, the expanded command is 
# saved in your history.


# declare
# https://www.computerhope.com/unix/bash/declare.htm

# Bats: Bash Automated Testing System
# https://github.com/sstephenson/bats

# parameters:
args=("$@")
echo $# arguments passed
echo ${args[0]} ${args[1]} ${args[2]}

# good for testing: 
# Run a command for specified time using timeout:
# timeout 2 ping google.com
yes - spams yes
seq - outputs a sequence
watch -n 5 free -m

# display a csv file
column -t -s , filename.csv


# String Library
# https://github.com/zombieleet/bashify

# Bash Arrays
# @ means "all" elements
# 🍰 https://opensource.com/article/18/5/you-dont-know-bash-intro-bash-arrays
allThreads=(1 2 4 8 16 32 64 128)
for t in ${allThreads[@]}; do
  ./command --threads $t
done

# Retrieve output of a Bash command
output=$( ./my_script.sh )

# Retrieve the output of a Json File (using JQ)
arr=( $(jq -r '.[].item2' json) )
printf '%s\n' "${arr[@]}"

# Parameter Expansion
# 🤓 https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_06_02
if [ -z ${var+x} ]; then echo "var is unset"; else echo "var is set to '$var'"; fi



## Bash Array Syntax:

declare -p anyArray freeze-dumps contents
declare -a anyArray <magic>  unfreeze

arr=()	Create an empty array
arr=(1 2 3)	Initialize array
${arr[2]}	Retrieve third element
${arr[@]}	Retrieve all elements  
${!arr[@]}	Retrieve array indices
${#arr[@]}	Calculate array size // 🤓 LENGTH! 
arr[0]=3	Overwrite 1st element
arr+=(4)	Append value(s)
str=$(ls)	Save ls output as a string
arr=( $(ls) )	Save ls output as an array of files
${arr[@]:s:n}	Retrieve n elements starting at index s
 
# readarray to read lines 
readarray -t arr2 < <(exec )
# readarray 2d dimensional array 
https://stackoverflow.com/questions/26634978/how-to-use-readarray-in-bash-to-read-lines-from-a-file-into-a-2d-array
# use jq to make a bash array
read -a bash_array < <(jq -r .|arrays|select(.!=null)|@tsv)


## JQ
# fill environment vars
export $(jq -r '@sh "FOO=\(.foo) BAZ=\(.baz)"')
# uri encode
date | jq -sRr @uri
# test types
echo '[true, null, 42, "hello", []]' | ./jq 'map(type)'
["boolean","null","number","string","array"]

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


#get today's files
ls -al --time-style=+%D | grep `date +%D`

#top 10 most frequently used commands
history | awk '{a[$2]++}END{for(i in a){print
a[i] " " i}}' | sort -rn | head
