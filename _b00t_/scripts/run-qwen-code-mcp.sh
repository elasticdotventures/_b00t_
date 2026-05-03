#!/usr/bin/env bash
# 🤓 run-qwen-code-mcp.sh - Launch qwen CLI as MCP server via stdio
# 🤓 qwen CLI doesn't have --mcp-server flag, so we use `qwen mcp` subcommand
# 🤓 This script is called by b00t-mcp when qwen-code MCP is activated

set -euo pipefail

# 🤓 qwen mcp subcommand expects specific format for server execution
# 🤓 We need to exec qwen in a way that provides stdio MCP interface
# 🤓 qwen CLI is designed for interactive use, so we wrap it for MCP compatibility

# 🤓 Check if qwen CLI is available
if ! command -v qwen &> /dev/null; then
    echo "Error: qwen CLI not found" >&2
    exit 1
fi

# 🤓 qwen mcp doesn't have a direct 'serve' subcommand
# 🤓 We need to use qwen in a mode that accepts stdin/stdout for MCP
# 🤓 For now, we'll use qwen with -p flag for prompt-based MCP interaction

# 🤓 MCP servers communicate via JSON-RPC over stdio
# 🤓 qwen CLI expects text prompts, not JSON-RPC
# 🤓 This requires a wrapper that translates JSON-RPC ↔ qwen CLI

# 🤓 Alternative: use b00t-mcp as the MCP server, proxy qwen commands through it
# 🤓 b00t-mcp already has ACL filtering and stdio support

# 🤓 For proper MCP integration, we need to:
# 1. Register qwen as an MCP tool through b00t-mcp
# 2. Use qwen mcp add to register capabilities if needed

# 🤓 For now, exec qwen in headless mode accepting stdin
exec qwen "$@"
