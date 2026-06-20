#!/usr/bin/env bash
# 🎯 Visual demo of fzf menu (non-interactive)

set -euo pipefail

# 🎨 Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
MAGENTA='\033[0;35m'
NC='\033[0m'

echo -e "${BLUE}╭─────────────────────────────────────────────────────────────╮${NC}"
echo -e "${BLUE}│${NC}  🥾 B00t Interactive Menu ${BLUE}                                      │${NC}"
echo -e "${BLUE}╰─────────────────────────────────────────────────────────────╯${NC}"
echo ""
echo -e "${CYAN}Session: kind-duck${NC}"
echo -e "${CYAN}Pane: hermes-agent-117836:general${NC}"
echo ""

echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${NC}  🎯 Select an option:                                            ${GREEN}║${NC}"
echo -e "${GREEN}╠════════════════════════════════════════════════════════════════╣${NC}"
echo -e "${GREEN}║${NC}  > 1 • Tell me a joke • Programming humor                        ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}    2 • Show system status • Check b00t health                  ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}    3 • View recent tasks • Task history                        ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}    4 • Run tests • Execute test suite                          ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}    5 • Deploy to production • Deploy code                      ${GREEN}║${NC}"
echo -e "${GREEN}║${NC}    6 • Exit menu • Return to workspace                        ${GREEN}║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${CYAN}🥾 b00t> ${NC} (Type to filter, ↑↓ to navigate, Enter to select)"
echo ""

echo -e "${YELLOW}╭─────────────────────────────────────────────────────────────╮${NC}"
echo -e "${YELLOW}│${NC}  🎭 DEMO: Selecting option 2 (Show system status)        ${YELLOW}│${NC}"
echo -e "${YELLOW}╰─────────────────────────────────────────────────────────────╯${NC}"
echo ""

echo -e "${BLUE}📊 System Status:${NC}"
echo -e "${GREEN}✅${NC} Zellij session: kind-duck"
echo -e "${GREEN}✅${NC} Current pane: 0"
echo -e "${GREEN}✅${NC} Window name: hermes-agent-117836:general"
echo -e "${GREEN}✅${NC} fzf version: 0.44.1 (debian)"
echo -e "${GREEN}✅${NC} b00t agent: hermes-agent"
echo ""

echo -e "${MAGENTA}╭─────────────────────────────────────────────────────────────╮${NC}"
echo -e "${MAGENTA}│${NC}  🎯 fzf Features Demo                                    ${MAGENTA}│${NC}"
echo -e "${MAGENTA}╰─────────────────────────────────────────────────────────────╯${NC}"
echo ""

echo -e "${CYAN}🔍 Search Demo (typing "status"):${NC}"
echo ""
echo -e "${GREEN}╔════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${GREEN}║${NC}  🎯 Select an option:                                            ${GREEN}║${NC}"
echo -e "${GREEN}╠════════════════════════════════════════════════════════════════╣${NC}"
echo -e "${GREEN}║${NC}  > 2 • Show system status • Check b00t health                  ${GREEN}║${NC}"
echo -e "${GREEN}╚════════════════════════════════════════════════════════════════╝${NC}"
echo ""
echo -e "${CYAN}🥾 b00t> status${NC}"
echo ""

echo -e "${MAGENTA}🎭 Joke Demo (Selecting option 1):${NC}"
echo ""
echo -e "${GREEN}🐛 Why do programmers prefer dark mode?${NC}"
echo -e "${GREEN}Because light attracts bugs! 🐛${NC}"
echo ""

echo -e "${YELLOW}💡 Pro Tips:${NC}"
echo "  • Use arrow keys or j/k to navigate"
echo "  • Type to filter options instantly"
echo "  • Press Enter to select"
echo "  • Press ESC to cancel"
echo "  • Tab for next match, Shift+Tab for previous"
echo ""

echo -e "${RED}╭─────────────────────────────────────────────────────────────╮${NC}"
echo -e "${RED}│${NC}  ✅ fzf Integration Demo Complete                           ${RED}│${NC}"
echo -e "${RED}╰─────────────────────────────────────────────────────────────╯${NC}"
echo ""
echo -e "${BLUE}To try the real interactive menu:${NC}"
echo "  bash _b00t_/scripts/zellij-fzf-menu.sh"
echo ""
echo -e "${BLUE}Or launch in floating pane:${NC}"
echo "  bash _b00t_/scripts/zellij-launch-fzf-menu.sh"
echo ""