# --- Docker-backed Claude CLI wrapper -------------------------------
# Shares node image + npm cache with npm.🐳, persists .claude credentials

export CLAUDE_DOCKER_IMAGE="${CLAUDE_DOCKER_IMAGE:-node:20-bookworm}"
export CLAUDE_DOCKER_PLATFORM="${CLAUDE_DOCKER_PLATFORM:-}"   # e.g. linux/arm64 or linux/amd64

# Shared cache dirs on host (reuse npm.🐳 setup)
mkdir -p "$HOME/.npm" "$HOME/.cache/node" "$HOME/.claude" "$HOME/.claude-tmp" "$HOME/.b00t"

# Subagents directory - determine path relative to this script
CLAUDE_DOCKER_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SUBAGENTS_DIR="$CLAUDE_DOCKER_DIR/awesome-claude-code-subagents/categories"

# Auto-detect container runtime (docker or podman) and socket location
# Returns: "docker:/path/to/socket" or "podman:/path/to/socket" or empty
_detect_container_runtime() {
  # Check for Docker first
  if command -v docker >/dev/null 2>&1; then
    if [ -S /var/run/docker.sock ]; then
      echo "docker:/var/run/docker.sock"
      return 0
    fi
  fi

  # Check for Podman
  if command -v podman >/dev/null 2>&1; then
    # Podman socket locations (in priority order)
    if [ -S /run/podman/podman.sock ]; then
      echo "podman:/run/podman/podman.sock"
      return 0
    elif [ -n "$XDG_RUNTIME_DIR" ] && [ -S "$XDG_RUNTIME_DIR/podman/podman.sock" ]; then
      echo "podman:$XDG_RUNTIME_DIR/podman/podman.sock"
      return 0
    fi
  fi

  # No runtime detected
  return 1
}

_claude_docker() {
  # Usage: _claude_docker [args...]
  # Mount PWD as chroot, persist .claude for credentials, share npm cache.
  # Includes persistent .tmp workspace, awesome-claude-code-subagents, and ~/.b00t.
  # Auto-detects and mounts container socket (docker or podman) for building b00t containers.

  local RUNTIME_INFO=$(_detect_container_runtime)
  local RUNTIME_CMD="docker"  # default to docker command
  local SOCKET_MOUNT=""

  if [ -n "$RUNTIME_INFO" ]; then
    local RUNTIME_TYPE="${RUNTIME_INFO%%:*}"
    local SOCKET_PATH="${RUNTIME_INFO#*:}"

    # Mount socket to standard /var/run/docker.sock location inside container
    SOCKET_MOUNT="-v $SOCKET_PATH:/var/run/docker.sock"

    # Use detected runtime for launching Claude container
    RUNTIME_CMD="$RUNTIME_TYPE"
  fi

  $RUNTIME_CMD run --rm -it \
    ${CLAUDE_DOCKER_PLATFORM:+--platform "$CLAUDE_DOCKER_PLATFORM"} \
    -v "$PWD":"$PWD" -w "$PWD" \
    -v "$HOME/.claude":/home/node/.claude \
    -v "$HOME/.claude-tmp":/home/node/.tmp \
    -v "$HOME/.b00t":/home/node/.b00t \
    -v "$HOME/.npm":/home/node/.npm \
    -v "$HOME/.cache/node":/home/node/.cache \
    -v "$SUBAGENTS_DIR":/home/node/.claude/agents:ro \
    $SOCKET_MOUNT \
    -e HOME=/home/node \
    -e npm_config_cache=/home/node/.npm \
    -e CONTAINER_RUNTIME="$RUNTIME_TYPE" \
    --user "$(id -u)":"$(id -g)" \
    "$CLAUDE_DOCKER_IMAGE" \
    npx --yes @anthropic-ai/claude "$@"
}

# Main alias
alias claude='_claude_docker'

# Quality of life
alias claudev='_claude_docker --version'

