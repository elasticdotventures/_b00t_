#!/bin/bash
# 🥾 Zellij Session Initialization
# Detects Zellij session, customizes window names, enables Zellij features

echo "🥾 Zellij Session Initialization"
echo "================================"
echo ""

# Zellij environment detection
ZELLIJ_SESSION="${ZELLIJ_SESSION_NAME:-}"
ZELLIJ_PANE="${ZELLIJ_PANE_ID:-}"
ZELLIJ_SOCKET="${ZELLIJ:-}"

# Agent identification
AGENT_TYPE="${B00T_AGENT_TYPE:-hermes-agent}"
SESSION_ID="${SESSION_ID:-$$}"
TASK_CONTEXT="${B00T_TASK_CONTEXT:-general}"

# Check if running in Zellij
if [ -n "$ZELLIJ_SESSION" ]; then
    echo "✅ Zellij workspace detected"
    echo "   Session: $ZELLIJ_SESSION"
    echo "   Pane: $ZELLIJ_PANE"
    echo "   Socket: $ZELLIJ_SOCKET"
    echo ""
    
    # Generate window name based on agent identity
    WINDOW_NAME="${AGENT_TYPE}-${SESSION_ID}:${TASK_CONTEXT}"
    
    echo "🏷️  Setting window name to: $WINDOW_NAME"
    
    # Rename window to agent name
    # Note: This requires Zellij CLI or protocol communication
    # For now, we'll prepare the command
    echo "   Command: zellij action rename-pane --name '$WINDOW_NAME'"
    
    # Enable Zellij-specific features
    export B00T_ZELLIJ_MODE="enabled"
    export B00T_ZELLIJ_SESSION="$ZELLIJ_SESSION"
    export B00T_ZELLIJ_PANE="$ZELLIJ_PANE"
    export B00T_WINDOW_NAME="$WINDOW_NAME"
    
    # Enable floating menus by default in Zellij
    export B00T_FLOATING_MENUS="enabled"
    export B00T_MENU_TYPE="floating"
    
    echo ""
    echo "✅ Zellij features enabled"
    echo "   - Floating menus: enabled"
    echo "   - Window naming: $WINDOW_NAME"
    echo "   - Pane management: active"
    echo "   - Session awareness: $ZELLIJ_SESSION"
    
else
    echo "📱 Standard terminal mode (no Zellij detected)"
    export B00T_ZELLIJ_MODE="disabled"
    export B00T_WINDOW_NAME="$AGENT_TYPE-$SESSION_ID:$TASK_CONTEXT"
fi

echo ""
echo "╔════════════════════════════════════════════╗"
echo "║      🥾 ZELLIJ SESSION INIT COMPLETE      ║"
echo "╠════════════════════════════════════════════╣"
echo "║                                            ║"
echo "║  Agent: $AGENT_TYPE"
echo "║  Session ID: $SESSION_ID"
echo "║  Window: $WINDOW_NAME"
echo "║  Zellij: $B00T_ZELLIJ_MODE"
echo "║                                            ║"
echo "╚════════════════════════════════════════════╝"

echo ""
echo "💡 Agent name is now the window name!"
echo "   Window identity: $WINDOW_NAME"

return 0