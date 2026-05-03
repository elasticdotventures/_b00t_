# install Git & Utilities
source "$_B00T_C0DE_Path/_b00t_.bashrc"

# _b00t_ plans to make extensive use of:
# https://github.com/actions/toolkit/
# for deploying: 
#   * azure static webpages


log_📢_记录 "🐙😇 installing git"
# $SUDO_CMD apt install -y git-all
$SUDO_CMD apt-get install -y git

# Set git to use the credential memory cache
log_📢_记录 "🐙😇 setting up credential helper cache"
git config --global credential.helper cache
git config --global credential.helper 'cache --timeout=3600'
#git config --global credential.helper 'store --file ~/.my-credentials'


# use vs code for Git commits. 
# https://stackoverflow.com/questions/30024353/how-to-use-visual-studio-code-as-default-editor-for-git

# official PPA for GitHub CLI and update your system packages accordingly.
# 🍰 https://github.com/cli/cli
# 🍰 https://www.techiediaries.com/install-github-cli-ubuntu-20/

## Github CLI
log_📢_记录 "🐙🔐 adding keyserver.ubuntu.com to apt-key"
$SUDO_CMD apt-key adv --keyserver keyserver.ubuntu.com --recv-key C99B11DEB97541F0
log_📢_记录 "🐙😇 adding github cli as trusted repository for gh cli tool"
$SUDO_CMD apt-add-repository https://cli.github.com/packages
$SUDO_CMD apt update -y  # TODO: make -y optional
$SUDO_CMD apt install -y gh


# Install GH on rasperrypi
#https://lindevs.com/install-github-cli-on-raspberry-pi/

# Git Automation with OAuth Tokens
# https://docs.github.com/en/github/extending-github/git-automation-with-oauth-tokens

## VS Code suggests
## * If you clone using a Git credential manager,
## * your container should already have access to your credentials!
# Shared Credential Storage
# https://git-scm.com/book/en/v2/Git-Tools-Credential-Storage


# gist
# gist-paste
# $SUDO_CMD apt install -y gist
# gist --login
# TODO: use the oauth
# https://tools.ietf.org/html/rfc6749#section-4.1


## SSH key sharing, not working yet. 
#SSH_FINGERPRINTv1="🤫 _b00t_ ssh agent v1"
#if [ $(/usr/bin/grep "$SSH_FINGERPRINTv1" -c "~/.bash_profile") -eq 0 ] ; then]
#    # start ssh agent in background
#    eval "$(ssh-agent -s)"
#    # now setup to run always
#    cat EOF>> ~/.b00t_profile
#
## $SSH_FINGERPRINTv1
#if [ -z "$SSH_AUTH_SOCK" ]; then
#   # Check for a currently running instance of the agent
#   RUNNING_AGENT="`ps -ax | grep 'ssh-agent -s' | grep -v grep | wc -l | tr -d '[:space:]'`"
#   if [ "$RUNNING_AGENT" = "0" ]; then
#        # Launch a new instance of the agent
#        ssh-agent -s &> $HOME/.ssh/ssh-agent
#   fi
#   eval `cat $HOME/.ssh/ssh-agent` << 
## /$SSH_FINGERPRINTv1
#EOF; 
#    fi

##
## 🧙 GVFS, use git as a virtual local caching filesystem
#```bash :: for windows (based on ProjFS)
# https://github.com/microsoft/VFSForGit/releases/download/v1.0.21085.9/SetupGVFS.1.0.21085.9.exe
#* REQUIRES
#* no git clean/smudge filters
#.gitattributes 
#* -text

#.gitattributes 
#project.pbxproj filter=munge-project-identifier

#github.com/elasticdotventures/_b00t_/blob/branch

#powershell 
#gvfs.ps1
#cache-serversclone
#configuration
#dehyrdate
#mount
#health
#Status
# Git Smudge and Clean Filters: Making Changes So You Don’t Have To

## PROJFS Windows
#```ps :: How to enable ProjFS using PowerShell
#Enable-WindowsOptionalFeature -Online -FeatureName Client-ProjFS -NoRestart
#```
#LifeCycle:
#Creation
#Startup
#Runtime
#Shutdown


##
## 🧙 ProjFS, a virtual filesystem layer in windows i.e. /proc
# ProjFS
# https://github.com/microsoft/ProjFS-Managed-API
# https://github.com/danielhodson/pyprojfs/blob/master/example.py



