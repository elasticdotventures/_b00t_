#!/usr/bin/env bash
# 🤓: b00t MCP sync - bidirectional sync between agent MCP configs
set -euo pipefail

OPERATION="${1:-}"  # push|pull
TARGET="${2:-}"     # kiro|claude|q
AGENT="${3:-}"      # optional: specific agent name

usage() {
  cat << EOF
Usage: $0 <push|pull> <target> [agent]

Sync MCP server configs between b00t and agent platforms:
  push    - Sync b00t MCP servers -> target agent configs
  pull    - Sync target agent MCP configs -> b00t datums
  
Targets:
  kiro    - Kiro CLI (JSON format)
  claude  - Claude Code (markdown frontmatter)
  q       - AWS Q Developer

Examples:
  $0 push kiro              # Sync all b00t MCP to kiro global config
  $0 push kiro cli-master   # Sync to specific kiro agent
  $0 pull claude            # Import Claude agent datums to b00t
EOF
  exit 1
}

[[ -z "$OPERATION" || -z "$TARGET" ]] && usage

case "$TARGET" in
  kiro)
    DATUM_FILE="$HOME/.b00t/_b00t_/kiro.mcp.toml"
    ;;
  claude)
    DATUM_FILE="$HOME/.b00t/_b00t_/claude.mcp.toml"
    ;;
  q)
    DATUM_FILE="$HOME/.b00t/_b00t_/q.mcp.toml"
    ;;
  *)
    echo "❌ Unknown target: $TARGET" >&2
    usage
    ;;
esac

# Delegate to b00t-cli for actual sync logic
exec b00t-cli mcp sync "$OPERATION" "$TARGET" ${AGENT:+"$AGENT"}
