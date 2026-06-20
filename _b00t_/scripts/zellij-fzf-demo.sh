#!/usr/bin/env bash
# 🎯 Demonstrate fzf menu in Zellij floating pane

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "🎯 Testing fzf menu integration with Zellij..."
echo ""

# Check if in Zellij
if [ -n "${ZELLIJ:-}" ]; then
    echo "✅ Running in Zellij session: ${ZELLIJ_SESSION_NAME:-unknown}"
    echo "✅ Current pane: ${ZELLIJ_PANE_ID:-unknown}"
else
    echo "⚠️  Not in Zellij session"
fi

echo ""
echo "📋 fzf Menu Features:"
echo ""
echo "1. 🎭 Interactive fzf menus with keyboard navigation"
echo "2. 🎨 Color-coded options with descriptions"
echo "3. 🎯 Floating pane integration in Zellij"
echo "4. 🚀 Real-time selection and execution"
echo "5. 💡 Multiple menu types (jokes, actions, status)"
echo ""

echo "🚀 To launch the fzf menu in a floating pane:"
echo ""
echo "  bash _b00t_/scripts/zellij-launch-fzf-menu.sh"
echo ""

echo "📝 Or run directly in current pane:"
echo ""
echo "  bash _b00t_/scripts/zellij-fzf-menu.sh"
echo ""

echo "🎯 Menu Options Include:"
echo ""
echo "  1. Tell me a joke (Programming humor)"
echo "  2. Show system status (Check b00t health)"
echo "  3. View recent tasks (Task history)"
echo "  4. Run tests (Execute test suite)"
echo "  5. Deploy to production (Deploy code)"
echo "  6. Exit menu (Return to workspace)"
echo ""

echo "✨ Key fzf Features Used:"
echo ""
echo "  • Interactive keyboard navigation (↑↓)"
echo "  • Search/filtering (type to filter)"
echo "  • Color-coded selection indicators"
echo "  • Rounded border with label"
echo "  • Automatic height adjustment"
echo "  • Cycle through options"
echo ""

echo "💡 Pro Tips:"
echo ""
echo "  • Use arrow keys or j/k to navigate"
echo "  • Type to filter options instantly"
echo "  • Press Enter to select"
echo "  • Press ESC to cancel"
echo "  • Tab for next match, Shift+Tab for previous"
echo ""

echo "🔧 Customization:"
echo ""
echo "  • Edit _b00t_/scripts/zellij-fzf-menu.sh to add options"
echo "  • Modify colors and prompts"
echo "  • Add new menu types"
echo "  • Integrate with b00t commands"
echo ""

echo "✅ fzf integration ready!"
echo "🥾 Your Zellij floating panes now support interactive fzf menus!"