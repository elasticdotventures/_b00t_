# --- Docker-backed Node toolchain aliases -------------------------------
# Image + arch (set linux/amd64 on ARM if you need x86 images)
export NODE_DOCKER_IMAGE="${NODE_DOCKER_IMAGE:-node:20-bookworm}"
export NODE_DOCKER_PLATFORM="${NODE_DOCKER_PLATFORM:-}"   # e.g. linux/arm64 or linux/amd64

# Shared cache dirs on host (opt-in; speeds up installs)
mkdir -p "$HOME/.npm" "$HOME/.cache/node" "$HOME/.pnpm-store" "$HOME/.yarn"

_node_docker() {
  # Usage: _node_docker <cmd> [args...]
  # Mount PWD, map UID/GID to avoid root-owned files, persist npm/pnpm/yarn caches.
  docker run --rm -it \
    ${NODE_DOCKER_PLATFORM:+--platform "$NODE_DOCKER_PLATFORM"} \
    -v "$PWD":"$PWD" -w "$PWD" \
    -v "$HOME/.npm":/home/node/.npm \
    -v "$HOME/.cache/node":/home/node/.cache \
    -v "$HOME/.pnpm-store":/home/node/.pnpm-store \
    -v "$HOME/.yarn":/home/node/.yarn \
    -e HOME=/home/node \
    -e npm_config_cache=/home/node/.npm \
    --user "$(id -u)":"$(id -g)" \
    "$NODE_DOCKER_IMAGE" "$@"
}

# Core aliases
alias node='_node_docker node'
alias npm='_node_docker npm'
alias npx='_node_docker npx'

# Corepack tools without polluting host:
# (Corepack is bundled in Node 16.10+; we call it directly)
alias yarn='_node_docker corepack yarn'
alias pnpm='_node_docker corepack pnpm'

# Quality of life
alias nodev='_node_docker node -v'
alias npmv='_node_docker npm -v'
alias pnv='_node_docker corepack pnpm -v'
alias yarnv='_node_docker corepack yarn -v'

