#!/usr/bin/env bash
# 🎯 Minimal fzf test

echo "Testing fzf in Zellij pane..."
echo "Session: ${ZELLIJ_SESSION_NAME:-unknown}"
echo "Pane: ${ZELLIJ_PANE_ID:-unknown}"
echo ""

# Simple fzf test
echo "Choose an option:" | fzf --header="Simple Test" --prompt="> "

echo ""
echo "You selected: $?"
echo "fzf test complete"