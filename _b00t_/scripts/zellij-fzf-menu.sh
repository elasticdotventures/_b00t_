#!/usr/bin/env bash
# 🎯 Launch fzf menu in Zellij floating pane

set -euo pipefail

# 🎨 Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${BLUE}🥾 B00t Interactive Menu${NC}"
echo -e "${CYAN}Session: ${ZELLIJ_SESSION_NAME:-unknown}${NC}"
echo ""

# fzf interactive menu
selection=$(cat << 'EOF' | fzf --header="🎯 Select an option:" --prompt="🥾 b00t> " --border=rounded --height=~40% --reverse --cycle
1 • Tell me a joke • Programming humor
2 • Show system status • Check b00t health
3 • View recent tasks • Task history
4 • Run tests • Execute test suite
5 • Deploy to production • Deploy code
6 • Exit menu • Return to workspace
EOF
)

echo ""

# Process selection
case "$selection" in
    "1 •"*)
        echo -e "${GREEN}🐛 Joke:${NC}"
        echo "Why do programmers prefer dark mode?"
        echo "Because light attracts bugs! 🐛"
        ;;
    "2 •"*)
        echo -e "${BLUE}📊 System Status:${NC}"
        echo "✅ Zellij session: ${ZELLIJ_SESSION_NAME:-unknown}"
        echo "✅ Current pane: ${ZELLIJ_PANE_ID:-unknown}"
        echo "✅ fzf version: $(fzf --version)"
        echo "✅ b00t agent: $(whoami)"
        ;;
    "3 •"*)
        echo -e "${CYAN}📋 Recent Tasks:${NC}"
        echo "→ Session awareness implemented"
        echo "→ Window naming configured"
        echo "→ Interactive menus with fzf"
        echo "→ Floating pane integration"
        ;;
    "4 •"*)
        echo -e "${GREEN}🧪 Running tests...${NC}"
        echo "Test suite execution started"
        ;;
    "5 •"*)
        echo -e "${YELLOW}🚀 Deploying to production...${NC}"
        echo "Deployment pipeline initiated"
        ;;
    "6 •"*)
        echo -e "${BLUE}👋 Goodbye!${NC}"
        exit 0
        ;;
esac

echo ""
echo -e "${CYAN}Press any key to close...${NC}"
read -r -n 1