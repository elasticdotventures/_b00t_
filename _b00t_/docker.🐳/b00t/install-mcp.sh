#!/bin/bash
# Self-installing b00t MCP server
# Registers b00t-mcp with Claude CLI
#
# Usage:
#   ./docker.🐳/b00t/install-mcp.sh

set -euo pipefail

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}b00t MCP Self-Installer${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Check if b00t container exists
if ! docker image inspect b00t:latest >/dev/null 2>&1; then
    echo -e "${YELLOW}WARNING: b00t:latest image not found${NC}"
    echo "Please build the unified container first:"
    echo "  ./docker.🐳/b00t/build-unified.sh"
    echo ""
    read -p "Continue anyway? (y/N) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check if Claude CLI is available
if ! command -v claude >/dev/null 2>&1; then
    echo -e "${YELLOW}WARNING: claude command not found${NC}"
    echo "Claude CLI not available in current environment"
    echo ""
    echo "To install manually, add to ~/.claude/config.json:"
    echo ""
    cat <<'EOF'
{
  "mcpServers": {
    "b00t": {
      "command": "docker",
      "args": [
        "run", "--rm", "-i",
        "--network", "host",
        "-v", "$HOME/.b00t:/home/node/.b00t:ro",
        "-e", "B00T_MQTT_URL=mqtt://localhost:1883",
        "b00t:latest",
        "b00t-mcp"
      ]
    }
  }
}
EOF
    echo ""
    exit 1
fi

echo -e "${GREEN}✓${NC} Claude CLI found: $(which claude)"
echo -e "${GREEN}✓${NC} b00t container: b00t:latest"
echo ""

# MCP configuration
MCP_CONFIG_DIR="${HOME}/.claude"
MCP_CONFIG_FILE="${MCP_CONFIG_DIR}/config.json"

# Create config directory if needed
mkdir -p "${MCP_CONFIG_DIR}"

# Check if config exists
if [ -f "${MCP_CONFIG_FILE}" ]; then
    echo -e "${BLUE}Existing Claude config found${NC}"

    # Check if b00t already configured
    if grep -q '"b00t"' "${MCP_CONFIG_FILE}" 2>/dev/null; then
        echo -e "${YELLOW}b00t MCP server already configured${NC}"
        echo ""
        echo "Current configuration:"
        jq '.mcpServers.b00t' "${MCP_CONFIG_FILE}" 2>/dev/null || echo "Unable to parse config"
        echo ""
        read -p "Overwrite? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo "Installation cancelled"
            exit 0
        fi
    fi

    # Backup existing config
    BACKUP="${MCP_CONFIG_FILE}.backup.$(date +%Y%m%d-%H%M%S)"
    cp "${MCP_CONFIG_FILE}" "${BACKUP}"
    echo -e "${GREEN}✓${NC} Backed up config to: ${BACKUP}"
fi

# Create or update config using jq
echo -e "${BLUE}Installing b00t MCP server...${NC}"

# Create the b00t MCP configuration
cat > /tmp/b00t-mcp-config.json <<'EOF'
{
  "command": "docker",
  "args": [
    "run",
    "--rm",
    "-i",
    "--network", "host",
    "-v", "$HOME/.b00t:/home/node/.b00t:ro",
    "-e", "B00T_MQTT_URL=mqtt://localhost:1883",
    "b00t:latest",
    "b00t-mcp"
  ],
  "env": {},
  "disabled": false
}
EOF

# Merge with existing config or create new
if [ -f "${MCP_CONFIG_FILE}" ]; then
    # Update existing config
    jq --argjson b00t "$(cat /tmp/b00t-mcp-config.json)" \
       '.mcpServers.b00t = $b00t' \
       "${MCP_CONFIG_FILE}" > /tmp/claude-config-updated.json
    mv /tmp/claude-config-updated.json "${MCP_CONFIG_FILE}"
else
    # Create new config
    jq -n --argjson b00t "$(cat /tmp/b00t-mcp-config.json)" \
       '{mcpServers: {b00t: $b00t}}' > "${MCP_CONFIG_FILE}"
fi

rm /tmp/b00t-mcp-config.json

echo -e "${GREEN}✓${NC} b00t MCP server installed"
echo ""

# Verify installation
echo -e "${BLUE}Verifying installation...${NC}"
if jq -e '.mcpServers.b00t' "${MCP_CONFIG_FILE}" >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Configuration valid"
    echo ""
    echo "b00t MCP server configuration:"
    jq '.mcpServers.b00t' "${MCP_CONFIG_FILE}"
else
    echo -e "${YELLOW}WARNING: Unable to verify configuration${NC}"
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}Installation Complete!${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Next steps:"
echo ""
echo "1. Restart Claude:"
echo "   source claude.🐳/env.sh"
echo "   claude"
echo ""
echo "2. Check MCP servers:"
echo "   /mcp"
echo ""
echo "3. Test b00t tools:"
echo "   Use b00t-mcp to check status"
echo ""
echo "Configuration file: ${MCP_CONFIG_FILE}"
echo ""
