#!/bin/bash
# Build unified b00t container (both CLI and MCP in one image)
# Run this from the HOST (not from within Claude container)
#
# Usage:
#   cd ~/homeassistant/_b00t_/node-ts.🦄
#   ./docker.🐳/b00t/build-unified.sh

set -euo pipefail

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}b00t Unified Container Build${NC}"
echo -e "${BLUE}(b00t-cli + b00t-mcp in one image)${NC}"
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

# Paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GOSPEL="${HOME}/.b00t"
DOCKERFILE="${SCRIPT_DIR}/Dockerfile.b00t"

# Verify gospel exists
if [ ! -d "$GOSPEL" ]; then
    echo -e "${YELLOW}ERROR: Gospel not found at ${GOSPEL}${NC}"
    echo "Clone from: https://github.com/elasticdotventures/dotfiles"
    exit 1
fi

echo -e "${GREEN}✓${NC} Gospel found: ${GOSPEL}"

# Verify unified Dockerfile exists
if [ ! -f "$DOCKERFILE" ]; then
    echo -e "${YELLOW}ERROR: Unified Dockerfile not found: ${DOCKERFILE}${NC}"
    exit 1
fi

echo -e "${GREEN}✓${NC} Unified Dockerfile: ${DOCKERFILE}"

# Get version from gospel
VERSION=$(grep '^version =' "${GOSPEL}/b00t-c0re-lib/Cargo.toml" | head -1 | cut -d'"' -f2)
echo -e "${GREEN}✓${NC} Version: ${VERSION}"

# Detect architecture
ARCH=$(uname -m)
echo -e "${GREEN}✓${NC} Architecture: ${ARCH}"

# Build info
BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
BUILD_COMMIT=$(cd "$GOSPEL" && git rev-parse HEAD 2>/dev/null || echo "unknown")

echo ""
echo -e "${BLUE}Building unified b00t container...${NC}"
echo "  Context: ${GOSPEL}"
echo "  Dockerfile: ${DOCKERFILE}"
echo "  Version: ${VERSION}"
echo "  Commit: ${BUILD_COMMIT}"
echo "  Date: ${BUILD_DATE}"
echo ""
echo "This builds BOTH:"
echo "  - b00t-cli (CLI tool)"
echo "  - b00t-mcp (MCP server)"
echo ""

# Build
$RUNTIME build \
    -f "${DOCKERFILE}" \
    --build-arg BUILD_VERSION="${VERSION}" \
    --build-arg BUILD_COMMIT="${BUILD_COMMIT}" \
    --build-arg BUILD_DATE="${BUILD_DATE}" \
    -t "b00t:latest" \
    -t "b00t:v${VERSION}" \
    -t "b00t:${ARCH}" \
    -t "b00t-cli:latest" \
    -t "b00t-mcp:latest" \
    "${GOSPEL}"

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Build Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Images tagged:"
echo "  - b00t:latest (unified)"
echo "  - b00t:v${VERSION}"
echo "  - b00t:${ARCH}"
echo "  - b00t-cli:latest (alias)"
echo "  - b00t-mcp:latest (alias)"
echo ""

# Test
echo -e "${BLUE}Running tests...${NC}"
echo ""
echo "Testing b00t-cli:"
$RUNTIME run --rm b00t:latest b00t-cli --version
$RUNTIME run --rm b00t:latest b00t-cli status --help

echo ""
echo "Testing b00t-mcp:"
$RUNTIME run --rm b00t:latest b00t-mcp --version

echo ""
echo "Testing b00t alias (→ b00t-cli):"
$RUNTIME run --rm b00t:latest b00t --version

echo ""
echo -e "${GREEN}✓ All tests passed!${NC}"
echo ""

# Verify architecture
echo -e "${BLUE}Binary architecture:${NC}"
$RUNTIME run --rm b00t:latest uname -m

echo ""
echo -e "${GREEN}Usage:${NC}"
echo ""
echo "Run CLI:"
echo "  $RUNTIME run --rm b00t:latest b00t-cli --help"
echo "  $RUNTIME run --rm b00t:latest b00t --help  # alias to b00t-cli"
echo ""
echo "Run MCP server:"
echo "  $RUNTIME run --rm -it b00t:latest b00t-mcp"
echo ""
echo "With workspace:"
echo "  $RUNTIME run --rm -v \$PWD:\$PWD -w \$PWD b00t:latest b00t-cli [command]"
echo ""
echo "Interactive:"
echo "  $RUNTIME run --rm -it b00t:latest bash"
echo "  # Inside container: both b00t-cli and b00t-mcp available"
echo ""
