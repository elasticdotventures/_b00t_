#!/usr/bin/env bash
# 🎯 Launch fzf menu using zellij run command

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "🎯 Launching fzf menu in Zellij..."

# Use zellij run to execute the script in a new pane
zellij run --floating --width 60% --height 50% --cwd "$SCRIPT_DIR" bash "$SCRIPT_DIR/zellij-fzf-menu.sh"

echo "✅ fzf menu launched!"
echo "Use arrow keys to navigate, type to filter, Enter to select"