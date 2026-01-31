#!/bin/bash
# --- Docker-backed b00t CLI wrapper -------------------------------
# Multi-language support: Rust, Node, Python via Docker containers

export B00T_DOCKER_IMAGE="${B00T_DOCKER_IMAGE:-b00t:aarch64}"
export B00T_DOCKER_PLATFORM="${B00T_DOCKER_PLATFORM:-}"   # e.g. linux/arm64

# Gospel and workspace setup
mkdir -p "$HOME/.b00t" "$HOME/.b00t/_b00t_"

# Create symlink convention: ~/_b00t_ → ~/.b00t/_b00t_/
if [ ! -L "$HOME/_b00t_" ]; then
    ln -sf "$HOME/.b00t/_b00t_" "$HOME/_b00t_"
fi

_b00t_docker() {
    # Usage: _b00t_docker [args...]
    # Mount PWD as workspace, persist .b00t gospel (hidden) and _b00t_ workspace (visible)
    docker run --rm -it \
        ${B00T_DOCKER_PLATFORM:+--platform "$B00T_DOCKER_PLATFORM"} \
        -v "$PWD":"$PWD" -w "$PWD" \
        -v "$HOME/.b00t":/home/b00t/.b00t \
        -v "$HOME/_b00t_":/home/b00t/_b00t_ \
        -e B00T_AGENT_NAME="${B00T_AGENT_NAME:-$(whoami)}" \
        -e B00T_MQTT_URL="${B00T_MQTT_URL:-mqtt://localhost:1883}" \
        --user "$(id -u)":"$(id -g)" \
        --network host \
        "$B00T_DOCKER_IMAGE" "$@"
}

# Main aliases
alias b00t='_b00t_docker'
alias b00t-cli='_b00t_docker'
alias b00t-mcp='_b00t_docker b00t-mcp'

# Quality of life
alias b00tv='_b00t_docker --version'
