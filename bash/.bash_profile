
# when .bash_profile exists then it runs before .bashrc and must call .bashrc
if [ -f ~/.bashrc ]; then
    . ~/.bashrc
fi

# vscode!
# git config --global core.editor "'{path to editor}' -n -w"
export GIT_EDITOR="code -w -r"
export EDITOR='code -w -r'
git config --global core.editor "code --wait"
# vscode
[[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"
export XAUTHORITY=$HOME/.Xauthority

# kubectl shell completion
if [ -f ~/.kube/completion.bash.inc ]; then
    source ~/.kube/completion.bash.inc
fi


# starship: prompt customizer
eval "$(starship init bash)"

# kubectl krew
export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"
