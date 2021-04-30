#!/bin/bash

## * * * * * * * * * * * \\
#*
#* Purpose: ⚠️ THIS IS AN EXAMPLE/TEMPLATE! (code in here doesn't run)
#*
## * * * * * * * * * * * //

#* 进口v2 🥾 ALWAYS load c0re Libraries!
source "/c0de/_b00t_/_b00t_.bashrc"


## * * * * \\
# Example Function

# AZ CLI Beta
pip3 install --pre --extra-index-url https://azcliprod.blob.core.windows.net/beta/simple/ azure-cli

# AZ fzf ?? 
# https://docs.microsoft.com/en-us/cli/azure/fzf?view=azure-cli-latest
az config set auto-upgrade.enable=yes
az upgrade
az fzf install 




