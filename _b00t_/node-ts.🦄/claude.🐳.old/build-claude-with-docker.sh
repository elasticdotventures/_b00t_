#!/bin/bash
# Build Claude CLI container with Docker CLI for building b00t containers

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $*"
}

# Auto-detect container runtime
detect_container_runtime() {
    if command -v docker >/dev/null 2>&1; then
        echo "docker"
    elif command -v podman >/dev/null 2>&1; then
        echo "podman"
    else
        echo "ERROR: Neither docker nor podman found" >&2
        exit 1
    fi
}

CONTAINER_CMD=$(detect_container_runtime)
log_info "Using container runtime: ${CONTAINER_CMD}"

log_step "Building claude-with-docker:latest"
${CONTAINER_CMD} build \
    -f "${SCRIPT_DIR}/Dockerfile.with-docker" \
    -t "claude-with-docker:latest" \
    "${SCRIPT_DIR}"

log_info "✓ Built: claude-with-docker:latest"
log_info ""
log_info "To use this image, set in your shell:"
log_info "  export CLAUDE_DOCKER_IMAGE=claude-with-docker:latest"
log_info ""
log_info "Then restart Claude:"
log_info "  source claude.🐳/env.sh"
log_info "  claude [command]"
