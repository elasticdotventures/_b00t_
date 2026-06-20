#!/usr/bin/env bash
# 🎯 Simple fzf menu test for Zellij floating pane

set -euo pipefail

# 🎨 Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}🥾 B00t Interactive Menu${NC}"
echo -e "${CYAN}Session: ${ZELLIJ_SESSION_NAME:-unknown}${NC}"
echo ""

# Simple fzf menu
selection=$(cat << 'EOF' | fzf --header="🎯 Select an option:" --prompt="🥾 b00t> " --border=rounded --height=~40%
1 • Tell me a joke • Programming humor
2 • Show system status • Check b00t health
3 • View recent tasks • Task history
4 • Exit menu • Return to workspace
EOF
)

# Process selection
case "$selection" in
    "1 •"*)
        echo -e "${GREEN}🐛 Why do programmers prefer dark mode?${NC}"
        echo -e "${GREEN}Because light attracts bugs!${NC}"
        ;;
    "2 •"*)
        echo -e "${BLUE}📊 System Status:${NC}"
        echo "✅ Zellij session: ${ZELLIJ_SESSION_NAME:-unknown}"
        echo "✅ Current pane: ${ZELLIJ_PANE_ID:-unknown}"
        echo "✅ fzf version: $(fzf --version)"
        ;;
    "3 •"*)
        echo -e "${CYAN}📋 Recent tasks:${NC}"
        echo "→ Session awareness implemented"
        echo "→ Window naming configured"
        echo "→ Interactive menus tested"
        ;;
    "4 •"*)
        echo -e "${BLUE}👋 Goodbye!${NC}"
        ;;
esac

echo ""
echo -e "${CYAN}Press any key to close...${NC}"
read -r -n 1