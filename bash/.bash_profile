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

    # Check if running in VS Code integrated terminal
    # Returns 0 (success) if VSCODE_GIT_IPC_HANDLE is set, 1 (failure) otherwise
    # Usage: if is_vscode_shell; then echo "In VS Code terminal"; fi
    source ~/.dotfiles/vscode.🆚/vscode-detection.sh
    if ! type is_vscode_shell &>/dev/null; then
        echo "🙈🥾 is_vscode_shell not defined"
    elif is_vscode_shell; then
        log_📢_记录 "🥾💻 hi VS Code! running b00t-cli"
        # b00t-cli vscode
    else
        log_📢_记录 "Not inside VSCODE"
    fi



    ## TODO Claude Code detection
    ## detect is_claudecode
    if is_claudecode; then
        log_📢_记录 "🥾🦞 hi Claude Code! b00t ready!"
        # b00t-cli claudecode
    else
        log_📢_记录 "Not inside Claude Code"
    fi

    echo "/🥾"
fi



# when .bash_profile exists then it runs before .bashrc and must call .bashrc
if [ -f ~/.bashrc ]; then
    echo "🐚 ~/.bashrc"
    . ~/.bashrc
    echo "/🐚"
fi

# read .env and export each var
if [ -f ~/.env ]; then
    while IFS= read -r line; do
        # Ignore comments and empty lines
        [[ "$line" =~ ^#.*$ || -z "$line" ]] && continue

        # Validate KEY=VALUE format
        if [[ "$line" =~ ^[^=]+=[^=]+$ ]]; then
            export "$line"
        else
            echo "Invalid line in .env: $line"
        fi
    done < ~/.env
fi



# check for .code-connect directory in home
if [[ $IS_WSL == true ]] ; then
    echo "🐧💌💙 WSL"
    # https://docs.roocode.com/features/shell-integration
    # . "$(code --locate-shell-integration-path bash)"    #

    if [[ "$TERM_PROGRAM" == "vscode" ]] || [[ -n "$VSCODE_INJECTION" ]]; then
        integration_script="$(code --locate-shell-integration-path bash 2>/dev/null)"
        if [[ -f "$integration_script" ]]; then
            . "$integration_script"
        fi
    fi

    # [[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"

elif [[ ! -d ~/.dotfiles/vscode.🆚/code-connect ]]; then
    # replace /home/brian/ with
    # test code alias
    if alias code &>/dev/null; then
        unalias code
    fi
    #alias code
    #unalias code

    # . "$(code --locate-shell-integration-path bash)"
    ## !?
    alias code='~/.dotfiles/vscode.🆚/.code-connect/bash/code.sh'
    alias code-connect='~/.dotfiles/vscode.🆚/.code-connect/bash/code-connect.sh'

    # 🤓 https://code.visualstudio.com/docs/terminal/shell-integration

    # vscode
    # [[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"
    echo '✅🆚 vscode (remote)'
else
    echo "🙈🆚 no vscode"
fi

# git config --global core.editor "'{path to editor}' -n -w"
export GIT_EDITOR="code -w -r"
export EDITOR='code -w -r'
if [ -f ~/.dotfiles/vscode.sh ]; then
    # kubectl edit deployment nginxproxy doesn't use alias!
    EDITOR=~/.dotfiles/vscode.sh
fi

git config --global core.editor "code --wait"


# vscode!

# FIX firefox doesn't work in wsl2 (but xeyes does)
# https://unix.stackexchange.com/questions/674214/x11-connection-rejected-because-of-wrong-authentication/709787#709787

export XAUTHORITY=$HOME/.Xauthority




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


# detect podman
if command -v podman &> /dev/null; then
    echo "✅🐳 podman"
    alias docker=podman
    export PODMAN_MACHINE_NAME=$( podman machine list --format '{{.Name}}' | grep '*' | tr -d '*' )
    if [ -z "$PODMAN_MACHINE_NAME" ]; then
        echo '🙈🐳 no podman machine found (this is fine)'
    else
        export PODMAN_SOCKET=$(podman machine inspect ${PODMAN_MACHINE_NAME} | jq -r '.[].ConnectionInfo.PodmanSocket.Path')
        #export PODMAN_SOCKET=$(ls $XDG_RUNTIME_DIR/podman/podman.sock)
        export PODMAN_HOST="unix://${PODMAN_SOCKET}"
        export DOCKER_HOST=$PODMAN_HOST
    fi
    # settings on sm3lly before return to docker.
    export DOCKER_HOST=unix://$(podman info --format '{{.Host.RemoteSocket.Path}}');
    # export DOCKER_HOST='unix:///home/brianh/.local/share/containers/podman/machine/qemu/podman.sock'


elif command -v docker &> /dev/null; then
     echo "🥲🐳 docker"
#     # https://docs.docker.com/engine/install/linux-postinstall/
#     # TODO: check group
#     # sudo usermod -aG docker $USER
#     # newgrp docker ??
else
    echo "🙈🐳 no docker"
fi



if [ -f ~/.huggingface/token ] ; then
 export HUGGING_FACE_HUB_TOKEN=$(cat ~/.huggingface/token)
 echo "🤗 HUGGING_FACE_HUB_TOKEN is set"
fi

if [ -f /usr/local/cuda ] ; then
    # TODO: more nvidia detection.
    export CUDA_HOME=/usr/local/cuda
fi


# check for uv
if [ -f "$HOME/.local/bin/uv" ]; then
    # Created by `pipx` on 2024-01-10 08:51:49
    export PATH="$PATH:$HOME/.local/bin"
else
    echo "🙈🐍 no uv"
fi


if [ -f ~/.cargo/env ]; then
    . "$HOME/.cargo/env"
else
    echo "🙈🦀 no cargo"
fi


############################################B
## Source of truth for k8s


# detect minikube
if [ -f ~/.kube/config ]; then
    # special
    export KUBECONFIG=~/.kube/config
fi
# kubectl shell completion
if [ -f ~/.kube/completion.bash.inc ]; then
    source ~/.kube/completion.bash.inc
fi
if command -v kubectl &> /dev/null; then
    echo "☸ 💯 kubectl"
    alias k=kubectl
    source <(kubectl completion bash)
    complete -o default -F __start_kubectl k
elif command -v minikube &> /dev/null; then
    echo "☸️🤏🏻minikube" ️
    alias kubectl="minikube kubectl --"
    alias k=kubectl

    source <(kubectl completion bash)
    complete -o default -F __start_kubectl k

    if [ -f ~/.kube/minikube-config ]; then
      # prefer alt
      export KUBECONFIG=~/.kube/minikube-config
    fi

    # bash completion
    #echo 'source <(kubectl completion bash)' >>~/.bashrc
    #echo 'alias k=kubectl' >>~/.bashrc
    #echo 'complete -o default -F __start_kubectl k' >>~/.bashrc
fi
echo "☸  KUBECONFIG: $KUBECONFIG"




# detect bun
if command -v bun &> /dev/null; then
    # bun
    echo "✅🍞 bun"
    export BUN_INSTALL="$HOME/.bun"
    export PATH=$BUN_INSTALL/bin:$PATH
fi

# detect nvm
if command -v nvm &> /dev/null; then
    # nvm
    nvm alias default node
    nvm use stable --lts
    # nvm use default

    NODE_VERSION=$(node --version)
    echo "✅🦄 has nvm node $NODE_VERSION"
    export NVM_DIR="$HOME/.nvm"
    [ -s "$NVM_DIR/nvm.sh" ] && \. "$NVM_DIR/nvm.sh" # This loads nvm
    [ -s "$NVM_DIR/bash_completion" ] && \. "$NVM_DIR/bash_completion" # This loads nvm bash_completion
else
    echo "🙈🦄 no nvm"
fi

# check for latest _b00t_ version
_B00T_LOCAL_VERSION=$(git -C ~/.dotfiles/ describe --tags --abbrev=0)
_B00T_LATEST_VERSION=$(gh release --repo elasticdotventures/dotfiles view --json tagName -q .tagName)
if [ -z "$_B00T_LOCAL_VERSION" ]; then
    echo "🙈🥾 no _b00t_ version"
elif [ "$_B00T_LOCAL_VERSION" != "$_B00T_LATEST_VERSION" ]; then
    echo "😟🥾 _b00t_ version $_B00T_LOCAL_VERSION is not the latest $_B00T_LATEST_VERSION"
else
    echo "😁🥾 local _b00t_ version $_B00T_LOCAL_VERSION"
fi


## I don't like nix
# if [ -e /home/brianh/.nix-profile/etc/profile.d/nix.sh ]; then . /home/brianh/.nix-profile/etc/profile.d/nix.sh; fi # added by Nix installer

## setup a global uv
if [ -f ~/.venv/bin/activate ] ; then
    source .venv/bin/activate
    echo "🐍 ~/.venv/bin/activate"
fi

# check for uv
if command -v uv &> /dev/null; then
    eval "$(uv generate-shell-completion bash)"
fi


if command -v direnv &> /dev/null; then
    eval "$(direnv hook bash)"
else
    echo "🥲 direnv not installed, cannot hook shell."
fi


if command -v pixi &> /dev/null; then
    # replacement for conda
    eval "$(pixi completion --shell bash)"
fi

# TODO: check if go is installed
export PATH=$PATH:/usr/local/go/bin

if [ -d ~/.krew ] ; then
    export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"
fi

alias gemini='npx -y https://github.com/google-gemini/gemini-cli'

echo "🐚 .bash_profile loaded"
