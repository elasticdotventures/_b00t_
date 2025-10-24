#!/bin/bash
# Quick build script for b00t-cli on aarch64
# Run this from the HOST (not from within Claude container)
#
# Usage:
#   cd ~/homeassistant/_b00t_/node-ts.🦄
#   ./docker.🐳/b00t/quick-build-aarch64.sh

set -euo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}b00t-cli Quick Build for aarch64${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Detect container runtime
if command -v docker >/dev/null 2>&1; then
    RUNTIME="docker"
elif command -v podman >/dev/null 2>&1; then
    RUNTIME="podman"
else
    echo -e "${YELLOW}ERROR: Neither docker nor podman found${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Using runtime: ${RUNTIME}"

# Verify gospel exists
GOSPEL="${HOME}/.b00t"
if [ ! -d "$GOSPEL" ]; then
    echo -e "${YELLOW}ERROR: Gospel not found at ${GOSPEL}${NC}"
    echo "Clone from: https://github.com/elasticdotventures/dotfiles"
    exit 1
fi

echo -e "${GREEN}✓${NC} Gospel found: ${GOSPEL}"

# Verify Dockerfile exists
DOCKERFILE="${GOSPEL}/Dockerfile.b00t-cli"
if [ ! -f "$DOCKERFILE" ]; then
    echo -e "${YELLOW}ERROR: Dockerfile not found: ${DOCKERFILE}${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Dockerfile: ${DOCKERFILE}"

# Get version
VERSION=$(grep '^version =' "${GOSPEL}/b00t-c0re-lib/Cargo.toml" | head -1 | cut -d'"' -f2)
echo -e "${GREEN}✓${NC} Version: ${VERSION}"

# Detect architecture
ARCH=$(uname -m)
echo -e "${GREEN}✓${NC} Architecture: ${ARCH}"

if [ "$ARCH" != "aarch64" ] && [ "$ARCH" != "arm64" ]; then
    echo -e "${YELLOW}WARNING: Running on ${ARCH}, not aarch64${NC}"
    echo "Build will produce ${ARCH} binary, not ARM64"
    echo ""
fi

# Build info
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
BUILD_COMMIT=$(cd "$GOSPEL" && git rev-parse HEAD 2>/dev/null || echo "unknown")

echo ""
echo -e "${BLUE}Building b00t-cli...${NC}"
echo "  Context: ${GOSPEL}"
echo "  Version: ${VERSION}"
echo "  Commit: ${BUILD_COMMIT}"
echo "  Date: ${BUILD_DATE}"
echo ""

# Build
$RUNTIME build \
    -f "${DOCKERFILE}" \
    --build-arg BUILD_VERSION="${VERSION}" \
    --build-arg BUILD_COMMIT="${BUILD_COMMIT}" \
    --build-arg BUILD_DATE="${BUILD_DATE}" \
    -t "b00t-cli:latest" \
    -t "b00t-cli:v${VERSION}" \
    -t "b00t-cli:${ARCH}" \
    "${GOSPEL}"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Build Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Images tagged:"
echo "  - b00t-cli:latest"
echo "  - b00t-cli:v${VERSION}"
echo "  - b00t-cli:${ARCH}"
echo ""

# Test
echo -e "${BLUE}Running tests...${NC}"
$RUNTIME run --rm b00t-cli:latest b00t-cli --version
$RUNTIME run --rm b00t-cli:latest b00t-cli status --help

echo ""
echo -e "${GREEN}✓ All tests passed!${NC}"
echo ""

# Verify architecture
echo -e "${BLUE}Binary architecture:${NC}"
$RUNTIME run --rm b00t-cli:latest uname -m

echo ""
echo -e "${GREEN}Usage:${NC}"
echo "  $RUNTIME run --rm b00t-cli:latest b00t-cli --help"
echo "  $RUNTIME run --rm -v \$PWD:\$PWD -w \$PWD b00t-cli:latest b00t-cli [command]"
echo ""
