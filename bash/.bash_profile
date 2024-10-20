#!/bin/bash

# Detect if running in WSL
if grep -qEi "(Microsoft|WSL)" /proc/version &>/dev/null; then
    export IS_WSL=true
    echo "🐧 Running in WSL"
else
    export IS_WSL=false
fi

# b00t is a collection of environment detection
if [ -f ~/.dotfiles/_b00t_/_b00t_.bashrc ] ; then
    echo "🥾 _b00t_"
    . ~/.dotfiles/_b00t_/_b00t_.bashrc
    echo "/🥾"
fi

# when .bash_profile exists then it runs before .bashrc and must call .bashrc
if [ -f ~/.bashrc ]; then
    echo "🐚 ~/.bashrc"
    . ~/.bashrc
    echo "/🐚 ~/.bashrc end"
fi

# check for .code-connect directory in home
if [[ $IS_WSL == true ]] ; then
    [[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"
elif [[ ! -d ~/.dotfiles/vscode.🆚/code-connect ]]; then
    # replace /home/brian/ with
    # test code alias
    if alias code &>/dev/null; then
        unalias code
    fi
    #alias code
    #unalias code

    # . "$(code --locate-shell-integration-path bash)"
    alias code='~/.dotfiles/vscode.🆚/.code-connect/bash/code.sh'
    alias code-connect='~/.dotfiles/vscode.🆚/.code-connect/bash/code-connect.sh'

    # 🤓 https://code.visualstudio.com/docs/terminal/shell-integration

    # git config --global core.editor "'{path to editor}' -n -w"
    export GIT_EDITOR="code -w -r"
    export EDITOR='code -w -r'
    git config --global core.editor "code --wait"
    # vscode
    # [[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"

    echo "✅🆚 vscode"
else
    echo "🙈🆚 no vscode"
fi

# vscode!

# FIX firefox doesn't work in wsl2 (but xeyes does)
# https://unix.stackexchange.com/questions/674214/x11-connection-rejected-because-of-wrong-authentication/709787#709787

export XAUTHORITY=$HOME/.Xauthority



# kubectl shell completion
if [ -f ~/.kube/completion.bash.inc ]; then
    source ~/.kube/completion.bash.inc
fi


# Check if the SSH agent is already running
if [ -z "$SSH_AUTH_SOCK" ]; then
    eval "$(ssh-agent -s)"
fi

# Add SSH keys to the agent
ssh-add -l &>/dev/null
if [ $? -ne 0 ]; then
    if [ -f ~/.ssh/id_rsa ]; then
        ssh-add ~/.ssh/id_rsa
    fi
    if [ -f ~/.ssh/id_ed25519 ]; then
        ssh-add ~/.ssh/id_ed25519
    fi
fi

# starship: prompt customizer
eval "$(starship init bash)"

# kubectl krew
export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"

# detect podman
if command -v podman &> /dev/null; then
    alias docker=podman
    export PODMAN_MACHINE_NAME=$( podman machine list --format '{{.Name}}' | grep '*' | tr -d '*' )
    # settings on sm3lly before return to docker.
    #export PODMAN_SOCKET=$(ls $XDG_RUNTIME_DIR/podman/podman.sock)
    export PODMAN_SOCKET=$(podman machine inspect ${PODMAN_MACHINE_NAME} | jq -r '.[].ConnectionInfo.PodmanSocket.Path')
    export PODMAN_HOST="unix://${PODMAN_SOCKET}"
    export DOCKER_HOST=$PODMAN_HOST
    export DOCKER_HOST=unix://$(podman info --format '{{.Host.RemoteSocket.Path}}');
    # export DOCKER_HOST='unix:///home/brianh/.local/share/containers/podman/machine/qemu/podman.sock'


    echo "✅🐳 podman"
elif command -v docker &> /dev/null; then
    echo "🥲🐳 docker"
else
    echo "🙈🐳 no docker"
fi

# detect bun
if command -v bun &> /dev/null; then
    # bun
    export BUN_INSTALL="$HOME/.bun"
    export PATH=$BUN_INSTALL/bin:$PATH
fi


source "$HOME/.rye/env"

if [ -f ~/.huggingface/token ] ; then
 export HUGGING_FACE_HUB_TOKEN=$(cat ~/.huggingface/token)
 echo "🤗 HUGGING_FACE_HUB_TOKEN is set"
fi

if [ -f /usr/local/cuda ] ; then
    # TODO: more nvidia detection.
    export CUDA_HOME=/usr/local/cuda
fi

# Created by `pipx` on 2024-01-10 08:51:49
export PATH="$PATH:/home/brianh/.local/bin"
