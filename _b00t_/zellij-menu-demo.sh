#!/bin/bash
# Zellij Menu Demo - Proof of Concept for Interactive Yes/No Menu

# Check if running in Zellij
if [ -z "$ZELLIJ_SESSION_NAME" ]; then
    echo "❌ Error: This script must be run inside a Zellij session"
    echo "   Start Zellij: zellij attach main"
    exit 1
fi

echo "🥾 Zellij Menu Demo - Proof of Concept"
echo "====================================="
echo ""

# Demo 1: Basic Yes/No Menu
echo "Demo 1: Basic Yes/No Menu"
echo "--------------------------"

# Create floating pane with menu
zellij action new-pane --direction right --floating --width 50% --height 25%

# Write menu interface to new pane
zellij action write-chars -- 0 "
╔════════════════════════════════════════════╗
║         🥾 ZELLIJ MENU SYSTEM             ║
╠════════════════════════════════════════════╣
║                                            ║
║  QUESTION: Continue with operation?       ║
║                                            ║
║  [Y] Yes - Proceed                         ║
║  [N] No  - Cancel                          ║
║                                            ║
║  Type Y or N and press Enter               ║
║                                            ║
╚════════════════════════════════════════════╝
> "

# Wait for user input (in a real implementation, this would capture from the pane)
echo ""
echo "⏳ Waiting for user input in the menu pane..."
echo ""
echo "💡 In a real implementation, the script would:"
echo "   1. Create the floating pane with the menu"
echo "   2. Capture keystrokes from the menu pane"
echo "   3. Process Y/N input"
echo "   4. Close the pane and return result"
echo ""
echo "✅ Menu displayed successfully in Zellij!"
echo ""
echo "Demo complete - check your Zellij session for the floating menu pane"

# Optional: Auto-close after demonstration
# Uncomment to close the menu pane automatically
# sleep 5
# zellij action close-pane

exit 0
