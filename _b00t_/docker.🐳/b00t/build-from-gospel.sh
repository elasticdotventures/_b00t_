#!/bin/bash
# Build b00t-cli and b00t-mcp using gospel Dockerfile (DRY - matches .github/workflows)
# This script mirrors the GitHub Actions workflow for local builds

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GOSPEL_DIR="${HOME}/.b00t"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

# Auto-detect container runtime (docker or podman)
detect_container_runtime() {
    if command -v docker >/dev/null 2>&1; then
        echo "docker"
    elif command -v podman >/dev/null 2>&1; then
        echo "podman"
    else
        log_error "Neither docker nor podman found"
        exit 1
    fi
}

CONTAINER_CMD=$(detect_container_runtime)
log_info "Using container runtime: ${CONTAINER_CMD}"

# Check gospel exists
if [ ! -d "${GOSPEL_DIR}" ]; then
    log_error "Gospel not found at ${GOSPEL_DIR}"
    log_info "The gospel is the source repository from elasticdotventures/dotfiles"
    exit 1
fi

# Check Dockerfile exists
if [ ! -f "${GOSPEL_DIR}/Dockerfile.b00t-cli" ]; then
    log_error "Dockerfile.b00t-cli not found in gospel"
    exit 1
fi

# Extract version from Cargo.toml (same as GitHub workflow)
log_step "Extracting version from workspace Cargo.toml"
VERSION=$(grep '^version =' "${GOSPEL_DIR}/b00t-c0re-lib/Cargo.toml" | head -1 | cut -d'"' -f2)
log_info "Version: ${VERSION}"

# Build arguments (same as GitHub workflow)
BUILD_COMMIT=$(cd "${GOSPEL_DIR}" && git rev-parse HEAD 2>/dev/null || echo "local")
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

log_step "Building b00t-cli container from gospel"
log_info "Dockerfile: ${GOSPEL_DIR}/Dockerfile.b00t-cli"
log_info "Context: ${GOSPEL_DIR}"
log_info "Version: ${VERSION}"
log_info "Commit: ${BUILD_COMMIT}"
log_info "Date: ${BUILD_DATE}"

# Build b00t-cli (matches GitHub workflow)
${CONTAINER_CMD} build \
    -f "${GOSPEL_DIR}/Dockerfile.b00t-cli" \
    --build-arg BUILD_VERSION="${VERSION}" \
    --build-arg BUILD_COMMIT="${BUILD_COMMIT}" \
    --build-arg BUILD_DATE="${BUILD_DATE}" \
    -t "b00t-cli:latest" \
    -t "b00t-cli:v${VERSION}" \
    -t "b00t-cli:aarch64" \
    "${GOSPEL_DIR}"

log_info "✓ Built: b00t-cli:latest"
log_info "✓ Built: b00t-cli:v${VERSION}"
log_info "✓ Built: b00t-cli:aarch64"

# Test container (same as GitHub workflow)
log_step "Testing b00t-cli container"
${CONTAINER_CMD} run --rm b00t-cli:latest b00t-cli --version
${CONTAINER_CMD} run --rm b00t-cli:latest b00t-cli status --help

log_info "✓ b00t-cli container tests passed"
log_info ""
log_info "Container ready:"
log_info "  ${CONTAINER_CMD} run --rm b00t-cli:latest b00t-cli --help"
log_info "  ${CONTAINER_CMD} run --rm -v \$PWD:\$PWD -w \$PWD b00t-cli:latest b00t-cli [command]"
