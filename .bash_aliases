
# https://dotfiles.io/aliases/gcloud/#authorization

# python
alias ve='python3 -m venv ./venv'
alias va='source ./venv/bin/activate'

# k8s
alias k="k3s kubectl "

# tofu (terraform)
alias tf=tofu

alias docker=podman

# azure
alias az="docker run -it -v ${HOME}/.ssh:/root/.ssh mcr.microsoft.com/azure-cli"


# requires ripgrep, ignores .git
alias itree='rg --files | tree --fromfile'


# starship
eval "$(starship init bash)"

# vscode!
# git config --global core.editor "'{path to editor}' -n -w"
export GIT_EDITOR="code -w -r"
git config --global core.editor "code --wait"
# vscode
[[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"
export XAUTHORITY=$HOME/.Xauthority


