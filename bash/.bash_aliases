alias ls='ls -F  --color=auto'
alias ll='ls -lh'
alias lt='ls --human-readable --size -1 -S --classify'

# bat - a pretty replacement for cat.
alias bat="batcat"

alias count='find . -type f | wc -l'
# copy verbose
alias cpv='rsync -ah --info=progress2'

# cd to _b00t_ (or current repo)
alias cg='cd `git rev-parse --show-toplevel`'
alias ..='cd ..'
alias ...='cd ../../'
alias mkdir='mkdir -pv'

# time & date
alias path='echo -e ${PATH//:/\\n}'
alias now='date +"%T"'
alias nowtime=now
alias nowdate='date +"%d-%m-%Y"'


# 🐍 Python
alias ve='python3 -m venv ./venv'
alias va='source ./venv/bin/activate'

# k8s
alias k="kubectl "

#
alias c='code $(fzf --height 40% --reverse)'

alias fd="/usr/bin/fdfind"

# handy for generating dumps, etc..
# $ script.sh >> foobar.`ymd`
alias ymd="date +'%Y%m%d'"
alias ymd_hm="date +'%Y%m%d.%H%M'"
alias ymd_hms="date +'%Y%m%d.%H%M%S'"

# vscode!
# git config --global core.editor "'{path to editor}' -n -w"
export GIT_EDITOR="code -w -r"
export EDITOR='code -w -r'
git config --global core.editor "code --wait"
# vscode
[[ "$TERM_PROGRAM" == "vscode" ]] && . "$(code --locate-shell-integration-path bash)"
export XAUTHORITY=$HOME/.Xauthority

# itree, like tree but ignores .git
alias itree='rg --files | tree --fromfile'

alias tf=tofu

# kubectl shell completion
source '/home/brianh/.kube/completion.bash.inc'

# kubectl krew
export PATH="${KREW_ROOT:-$HOME/.krew}/bin:$PATH"


