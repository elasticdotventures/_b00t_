#!/bin/sh
# obsidian-mcp-bridge entrypoint
# Bridges MCP stdio ↔ HTTP to a running Obsidian instance with obsidian-mcp-connector
set -euo pipefail

OBSIDIAN_HOST="${OBSIDIAN_HOST:-host.docker.internal}"
OBSIDIAN_PORT="${OBSIDIAN_PORT:-27200}"
OBSIDIAN_MCP_PATH="${OBSIDIAN_MCP_PATH:-/mcp}"
OBSIDIAN_TOKEN="${OBSIDIAN_TOKEN:-}"
BIND_PORT="${BIND_PORT:-27201}"

OBSIDIAN_URL="http://${OBSIDIAN_HOST}:${OBSIDIAN_PORT}${OBSIDIAN_MCP_PATH}"

# Mode 1: HTTP relay mode — expose port for HTTP MCP clients (Claude Code, etc.)
if [ "${1:-}" = "http" ]; then
    echo "Starting HTTP relay on 0.0.0.0:${BIND_PORT} → ${OBSIDIAN_URL}" >&2
    exec socat TCP-LISTEN:${BIND_PORT},fork,reuseaddr \
        "SYSTEM:npx -y mcp-remote '${OBSIDIAN_URL}' --header 'Authorization: Bearer ${OBSIDIAN_TOKEN}' ${EXTRA_ARGS:-}",nofork
fi

# Mode 2: Stdio bridge — for clients that speak stdio MCP (b00t, Claude Desktop)
# Run via: podman run -i --rm --name obsidian-mcp obsidian-mcp-bridge
echo "Starting stdio bridge → ${OBSIDIAN_URL}" >&2
exec npx -y mcp-remote "${OBSIDIAN_URL}" --header "Authorization: Bearer ${OBSIDIAN_TOKEN}" ${EXTRA_ARGS:-}
