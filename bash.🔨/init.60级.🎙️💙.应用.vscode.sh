#!/bin/bash

## 🎙️💙🪟 * * * * * * * * * * * \\
#*
#* Purpose: Microsoft Visual Studio Code for Windows
#*
## 🎙️💙🪟 * * * * * * * * * * * //

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"



if [ is_WSLv2_🐧💙🪟v2() ] ; then
    # fixes some buggy behaviors in early wsl's
    sudo apt-get install realpath
fi 

# 🍰 https://stackoverflow.com/questions/30024353/how-to-use-visual-studio-code-as-default-editor-for-git
# set vscode as default editor
export EDITOR="code --wait"
git config --global core.editor "code --new-window --wait"

# Setup commands for VS CODE 
