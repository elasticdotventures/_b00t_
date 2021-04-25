#!/bin/bash

## 🎙️💙🪟 * * * * * * * * * * * \\
#*
#* Purpose: Microsoft Visual Studio Code for Windows
#*
## 🎙️💙🪟 * * * * * * * * * * * //

#* 进口v1 🥾 ALWAYS load c0re Libraries!
if [ `type -t "_b00t_init_🥾_开始"` != "function" ]; then 
    # not loaded, so load it _b00t_ environment 
    source "../_b00t_.bashrc"
fi
_b00t_init_🥾_开始
#* /进口



if [ is_WSLv2_🐧💙🪟v2() ] ; then
    # fixes some buggy behaviors in early wsl's
    sudo apt-get install realpath
fi 

# set vscode as default editor

git config --global core.editor code

# Setup commands for VS CODE 
