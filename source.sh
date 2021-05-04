#!/bin/bash

args=("$@")
source "/c0de/_b00t_/_b00t_.bashrc"
export FILENAME=$(expandPath ${args[0]})
echo "FILENAME: $FILENAME"
bash_source_加载 "$_B00T_C0DE_Path/./$FILENAME"
exit 0 


