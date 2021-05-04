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

# 
alias c='code $(fzf --height 40% --reverse)'

alias fd="/usr/bin/fdfind"

# handy for generating dumps, etc..
# $ script.sh >> foobar.`ymd`
alias ymd="date +'%Y%m%d'"
alias ymd_hm="date +'%Y%m%d.%H%M'"
alias ymd_hms="date +'%Y%m%d.%H%M%S'"
