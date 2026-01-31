#!/bin/bash
# git layer environment

export GIT_LAYER_VERSION="2.43"

# Docker wrapper for git
_git_docker() {
    docker run --rm -it \
        -v "$PWD":"$PWD" -w "$PWD" \
        -v "$HOME/.gitconfig":/home/user/.gitconfig:ro \
        --user "$(id -u)":"$(id -g)" \
        b00t-layer/git:2.43 \
        git "$@"
}

alias git='_git_docker'
