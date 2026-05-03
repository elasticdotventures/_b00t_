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

# lazy
alias dotenv='dotenvy'

# time & date
alias path='echo -e ${PATH//:/\\n}'
alias now='date +"%T"'
alias nowtime=now
alias nowdate='date +"%d-%m-%Y"'


# 🐍 Python
# alias ve='python3 -m venv ./venv'
# alias va='source ./venv/bin/activate'
alias ve='uv venv'
alias va='source .venv/bin/activate'

#
alias c='code $(fzf --height 40% --reverse)'

alias fd="/usr/bin/fdfind"

# handy for generating dumps, etc..
# $ script.sh >> foobar.`ymd`
alias ymd="date +'%Y%m%d'"
alias ymd_hm="date +'%Y%m%d.%H%M'"
alias ymd_hms="date +'%Y%m%d.%H%M%S'"

# generate a random password
alias randpw="tr -dc 'a-zA-Z0-9' < /dev/urandom | head -c 16"

# itree, like tree but ignores .git
alias itree='rg --files | tree --fromfile'

alias tf=tofu


#alias aws='docker run --rm -it -v ~/.aws:/root/.aws docker.io/amazon/aws-cli'
# source "$HOME/.rye/env"

## note: all k8s k= kubectl minikube should be in .bash_profile
alias nsc='podman run --rm docker.io/natsio/nats-box:latest nsc'

alias nsc='podman run --rm docker.io/natsio/nats-box:latest nsc'
