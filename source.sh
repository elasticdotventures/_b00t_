#!/bin/bash

args=("$@")

source "/c0de/_b00t_/_b00t_.bashrc"

export FILENAME=$(expandPath ${args[0]})
echo "SOURCE: $FILENAME"
#find 
#ls ./bash.🔨/
ls $(expandPath "$FILENAME")
echo `expandPath "$FILENAME"`
if n0ta_xfile_📁_好不好 "$_B00T_C0DE_Path/./$FILENAME" ; then
    chmod +x "$_B00T_C0DE_Path/./$FILENAME"
fi
bash_source_加载 "$_B00T_C0DE_Path/./$FILENAME"
exit 0 


