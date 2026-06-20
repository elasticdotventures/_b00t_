#!/usr/bin/env bash
# 🥾 Zellij fzf Selection Menu (for use with zellij run)
# For complex selections with search/filter

set -euo pipefail

echo ""
echo "  🥾 B00T Selection Menu"
echo "  ─────────────────────"
echo ""

# fzf interactive selection
selection=$(cat << 'EOF' | fzf \
    --header="🎯 Select an option (type to filter, ↑↓ to navigate)" \
    --prompt="🥾 b00t> " \
    --border=rounded \
    --border-label=" zellij-fzf " \
    --color=header:blue,marker:green,pointer:cyan,border:gray,prompt:yellow \
    --height=~60% \
    --min-height=8 \
    --reverse \
    --cycle
1 • Build project • Compile and run tests
2 • Run integration tests • Full test suite
3 • Deploy to staging • Staging environment
4 • Deploy to production • Production environment
5 • Code review • Review pending PRs
6 • Run diagnostics • System health check
7 • Cancel • Return to workspace
EOF
)

echo ""
case "$selection" in
    "1 •"*) echo "✅ Selected: Build project" ;;
    "2 •"*) echo "✅ Selected: Run tests" ;;
    "3 •"*) echo "✅ Selected: Deploy to staging" ;;
    "4 •"*) echo "✅ Selected: Deploy to production" ;;
    "5 •"*) echo "✅ Selected: Code review" ;;
    "6 •"*) echo "✅ Selected: Run diagnostics" ;;
    "7 •"*) echo "👋 Cancelled" ;;
    "")      echo "👋 No selection" ;;
esac
echo ""
echo "Press any key to close..."
read -r -n 1