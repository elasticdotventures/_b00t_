#!/usr/bin/env bash
# 🥾 Zellij Interactive Modal Runner
# Launches a floating pane with an interactive modal dialog.
# Uses zellij run for proper TTY support.
#
# The modal appears as a floating pane in the Zellij session.
# User acknowledges by pressing Y/N/C.
#
# Exit codes from the modal script are returned.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TITLE="${1:-🥾 B00T Interactive Modal}"
MESSAGE="${2:-Please acknowledge this dialog to continue.}"
SCRIPT="$SCRIPT_DIR/zellij-modal.sh"

# Ensure we're in a Zellij session
if [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
    echo "❌ Not in a Zellij session"
    echo "Start zellij first, then run this command."
    exit 2
fi

echo "🥾 Launching Zellij modal..."
echo "Title: $TITLE"
echo ""

# Launch modal in floating pane with proper TTY
zellij run \
    --floating \
    --width 60% \
    --height 35% \
    --pinned true \
    --close-on-exit \
    -- bash "$SCRIPT" "$TITLE" "$MESSAGE"

echo "✅ Modal displayed in floating pane"
echo "   (Pane will close when you respond)"