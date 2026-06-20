#!/usr/bin/env bash
# 🎯 Simple echo test for zellij

echo "╔════════════════════════════════════════════════════════════════╗"
echo "║                    🥾 ZELLIJ TEST PANE                          ║"
echo "╚════════════════════════════════════════════════════════════════╝"
echo ""
echo "✅ This pane is working!"
echo "Session: ${ZELLIJ_SESSION_NAME:-unknown}"
echo "Pane: ${ZELLIJ_PANE_ID:-unknown}"
echo ""
echo "Waiting 5 seconds..."
sleep 5
echo ""
echo "✅ Test complete!"
echo ""
echo "Press Enter to exit..."
read -r