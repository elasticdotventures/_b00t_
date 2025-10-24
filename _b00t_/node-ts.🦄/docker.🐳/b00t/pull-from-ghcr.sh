#!/bin/bash
# Pull pre-built b00t-cli from GitHub Container Registry
# Much faster than building locally (seconds vs 10-15 minutes)
#
# Usage:
#   ./docker.🐳/b00t/pull-from-ghcr.sh

set -euo pipefail

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Pull b00t-cli from GitHub Container Registry${NC}"
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

# GHCR image
GHCR_IMAGE="ghcr.io/elasticdotventures/b00t-cli"

echo -e "${BLUE}Pulling from GHCR...${NC}"
echo "  Image: ${GHCR_IMAGE}:latest"
echo ""

# Pull latest
$RUNTIME pull ${GHCR_IMAGE}:latest

# Tag as local b00t-cli
$RUNTIME tag ${GHCR_IMAGE}:latest b00t-cli:latest
$RUNTIME tag ${GHCR_IMAGE}:latest b00t:latest

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Pull Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Images tagged:"
echo "  - b00t-cli:latest (from GHCR)"
echo "  - b00t:latest (alias)"
echo ""

# Test
echo -e "${BLUE}Testing...${NC}"
$RUNTIME run --rm b00t-cli:latest b00t-cli --version

echo ""
echo -e "${GREEN}✓ Ready to use!${NC}"
echo ""
echo "Usage:"
echo "  $RUNTIME run --rm b00t-cli:latest b00t-cli --help"
echo "  $RUNTIME run --rm -v \$PWD:\$PWD -w \$PWD b00t-cli:latest b00t-cli status"
echo ""
echo "Note: This image contains b00t-cli only (from CI/CD)"
echo "For unified container (CLI + MCP), use: ./docker.🐳/b00t/build-unified.sh"
echo ""
