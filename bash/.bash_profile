
# when .bash_profile exists then it runs before .bashrc and must call .bashrc
if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi


# check for .code-connect directory in home
if [ ! -d ~/.dotfiles/vscode.🆚/code-connect ]; then
    # replace /home/brian/ with
    alias code='~/.dotfiles/vscode.🆚/.code-connect/bash/code.sh'
    alias code-connect='~/.dotfiles/vscode.🆚/.code-connect/bash/code-connect.sh'
    [[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"

    # git config --global core.editor "'{path to editor}' -n -w"
    export GIT_EDITOR="code -w -r"
    export EDITOR='code -w -r'
    git config --global core.editor "code --wait"
    # vscode
    [[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"

    echo "✅🆚"
else
    echo "🙈🆚 no vscode"
fi

# vscode!
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
    export PODMAN_SOCKET=$(podman machine inspect ${PODMAN_MACHINE_NAME} | jq -r '.[].ConnectionInfo.PodmanSocket.Path')
    export PODMAN_HOST="unix://${PODMAN_SOCKET}"
    export DOCKER_HOST=$PODMAN_HOST
    export DOCKER_HOST=unix://$(podman info --format '{{.Host.RemoteSocket.Path}}');
    # export DOCKER_HOST='unix:///home/brianh/.local/share/containers/podman/machine/qemu/podman.sock'
fi

# detect bun
if command -v bun &> /dev/null; then
    # bun
    export BUN_INSTALL="$HOME/.bun"
    export PATH=$BUN_INSTALL/bin:$PATH
fi


   source "$HOME/.rye/env"
