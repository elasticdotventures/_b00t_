#!/bin/bash

#function d1rd1ct () {}
# args = .. 
# get a list of files, output to yaml   
# convert 
# https://stackoverflow.com/questions/15978504/add-text-at-the-end-of-each-line

# bash menu
# fd --type f . bash.🔨/ | grep -e 'sh$'| grep -v "新"

#  << `fd . bash.🔨/ | sed`

# convert a directory to yaml
#fd --type f . bash.🔨/ | grep -e 'sh$'| grep -v "新" \
# | sed --expression '/:[0-9a-zA-Z]*$/ ! s/$/: ""/'  | yq r - -d '*'
 # | yq m ./bash.🔨/README.yaml

# 🍰 https://gitlab.com/tukusejssirs/lnx_scripts/-/blob/master/bash/functions/alert.sh


rawurlencode() {
  local string="${1}"
  local strlen=${#string}
  local encoded=""
  local pos c o

  for (( pos=0 ; pos<strlen ; pos++ )); do
     c=${string:$pos:1}
     case "$c" in
        [-_.~a-zA-Z0-9] ) o="${c}" ;;
        * )               printf -v o '%%%02x' "'$c"
     esac
     encoded+="${o}"
  done
  echo "${encoded}"    # You can either set a return variable (FASTER) 
  REPLY="${encoded}"   #+or echo the result (EASIER)... or both... :p
}

# Returns a string in which the sequences with percent (%) signs followed by
# two hex digits have been replaced with literal characters.
rawurldecode() {
  # This is perhaps a risky gambit, but since all escape characters must be
  # encoded, we can replace %NN with \xNN and pass the lot to printf -b, which
  # will decode hex for us
  printf -v decoded '%b' "${1//%/\\x}"

  echo "${decoded}"
  return 0
}


#echo -e '---
##foo:bar
#---
##buzz:42
#' | yq r - -d '*'
function d1rd1ct() {
    args=("$@")
    dir=$1
    # https://mikefarah.gitbook.io/yq/
    
    yamlFile="$(basename $dir).yaml"
    #STR=$( rawurlencode `fd --type f . $dir | grep -e 'sh$'| grep -v "新"` )
    #rawurldecode $STR

    #
    # list of files as yaml
    #fd --type f . $dir | \
    #    grep -e 'sh$'| grep -v "新" | \
    #    sed --expression '/:[0-9a-zA-Z]*$/ ! s/^\(.*\)$/"\1": ""/' | \
    #    sponge > /dev/shm/$yamlFile
    
    # cat "/dev/shm/$yamlFile"
    #while IFS= read -r line; do echo "$line" | base64; done < "/dev/shm/$yamlFile"
    fd --type f . $dir | grep -e 'sh$'| grep -v "新" | \
    while IFS= read -r line; do echo "$line" | base64; done | \
    sponge | \
    sed --expression '/:[0-9a-zA-Z]*$/ ! s/^\(.*\)$/"\1": ""/' | \
    sponge

    
    # STR=$(cat /dev/shm/$yamlFile)
    # rawurlencode $STR
     # yq -C eval-all 'select(fileIndex == 0) * select(fileIndex == 1)'  "/dev/shm/$yamlFile" "./$dir/README.yaml"

}



d1rd1ct "bash.🔨/"


# cat ./bash.🔨/README.yaml  | yq . -y

# function _b00t_sc0re () 