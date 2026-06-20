#!/usr/bin/env bash
# 🎯 Test fzf menu directly (no Zellij actions)

echo "🥾 B00t fzf Menu Test"
echo "Session: ${ZELLIJ_SESSION_NAME:-unknown}"
echo "Pane: ${ZELLIJ_PANE_ID:-unknown}"
echo ""

# Simple fzf test
selection=$(cat << 'EOF' | fzf --header="🎯 Select an option:" --prompt="🥾 b00t> " --border=rounded --height=~40%
1 • Tell me a joke
2 • Show system status
3 • Exit
EOF
)

echo ""
echo "Selected: $selection"

case "$selection" in
    "1 •"*)
        echo "🐛 Why do programmers prefer dark mode?"
        echo "Because light attracts bugs!"
        ;;
    "2 •"*)
        echo "📊 Session: ${ZELLIJ_SESSION_NAME}"
        echo "Pane: ${ZELLIJ_PANE_ID}"
        ;;
    "3 •"*)
        echo "👋 Goodbye!"
        ;;
esac