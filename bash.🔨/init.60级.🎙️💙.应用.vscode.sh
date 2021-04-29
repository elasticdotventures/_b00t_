#!/bin/bash

## 🎙️💙🪟 * * * * * * * * * * * \\
#*
#* Purpose: Microsoft Visual Studio Code for Windows
#*
## 🎙️💙🪟 * * * * * * * * * * * //

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "./_b00t_.bashrc"

# Command line management for vs `code`
# 🤓 https://code.visualstudio.com/docs/editor/extension-marketplace#_workspace-recommended-extensions
# code --extensions-dir <dir>
#    Set the root path for extensions.
# code --list-extensions
#    List the installed extensions.
# code --show-versions
#    Show versions of installed extensions, when using --list-extension.
# code --install-extension (<extension-id> | <extension-vsix-path>)
#    Installs an extension.
# code --uninstall-extension (<extension-id> | <extension-vsix-path>)
#    Uninstalls an extension.
# code --enable-proposed-api (<extension-id>)
#    Enables proposed API features for extensions. Can receive one or more extension IDs to enable individually.


if [ is_WSLv2_🐧💙🪟v2() ] ; then
    # fixes some buggy behaviors in early wsl's
    sudo apt-get install realpath
fi 

# 🍰 https://stackoverflow.com/questions/30024353/how-to-use-visual-studio-code-as-default-editor-for-git
# set vscode as default editor
export EDITOR="code --wait"
git config --global core.editor "code --new-window --wait"

# $PROJECT_ROOT/devcontainer.json
# Remote-Containers extension automatically starts/stops
# "shutdownAction": "none"
# 

# 🤓 Developing VS Code inside a container
# https://code.visualstudio.com/docs/remote/containers


# 🤓 Custom Windows Disctionary
# https://www.windowscentral.com/how-edit-custom-spell-check-dictionary-windows-10


# Setup commands for VS CODE 
