#!/usr/bin/env bash
# 🎯 Launch fzf menu in Zellij floating pane

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
MENU_SCRIPT="$SCRIPT_DIR/zellij-fzf-menu.sh"

# Check if Zellij is running
if [ -z "${ZELLIJ:-}" ]; then
    echo "❌ Not running in Zellij"
    echo "This script must be run from within a Zellij session"
    exit 1
fi

# Create floating pane
echo "🎯 Creating floating pane with fzf menu..."
FLOATING_PANE_ID=$(zellij action new-pane --floating --width 60% --height 50% --)

# Wait for pane to be ready
sleep 0.5

# Write command to the floating pane
printf "cd %s && %s\n" "$SCRIPT_DIR" "$MENU_SCRIPT" | zellij action write-chars "$FLOATING_PANE_ID"

echo "✅ Floating pane created with ID: $FLOATING_PANE_ID"
echo "🥾 Interactive fzf menu is now available"

# List panes to show current state
echo ""
echo "Current panes:"
zellij action list-panes