# 🥾 Zellij Interactive Modal just recipes
# Provides interactive modal dialogs via Zellij floating panes.
#
# Usage:
#   JUST_UNSTABLE=1 just zellij-modal           # Default "confirm" modal
#   JUST_UNSTABLE=1 just zellij-modal "Title" "Message"
#   JUST_UNSTABLE=1 just zellij-deploy-confirm  # Deploy confirmation
#   JUST_UNSTABLE=1 just zellij-status          # System status modal

set shell := ["bash", "-cu"]

B00T_ROOT := "/home/brianh/.dotfiles"

# Display interactive modal in Zellij floating pane
# User MUST acknowledge (Y/N) for success
# Exit codes: 0=Yes, 1=No/Cancel, 2=Error
zellij-modal title="🥾 B00T Interactive Modal" message="Acknowledge this dialog?":
    #!/bin/bash
    set -euo pipefail
    SCRIPT="{{ B00T_ROOT }}/_b00t_/scripts/zellij-modal-runner.sh"
    if [ -n "${ZELLIJ_SESSION_NAME:-}" ]; then
        bash "$SCRIPT" "{{title}}" "{{message}}"
        echo ""
        echo "🕐 Waiting for user response in floating pane..."
    else
        echo "❌ Not in Zellij session. Start zellij first."
        echo "   zellij attach main || zellij"
        exit 2
    fi

# Quick deploy confirmation modal
zellij-deploy-confirm env="production":
    just zellij-modal "🚀 Deploy to {{env}}?" "Confirm deployment to {{env}}\n\nBranch: $(git rev-parse --abbrev-ref HEAD 2>/dev/null)\nCommit: $(git rev-parse --short HEAD 2>/dev/null)"

# Quick system status modal
zellij-status:
    #!/bin/bash
    set -euo pipefail
    MSG="Zellij session: ${ZELLIJ_SESSION_NAME:-unknown}\n"
    MSG+="Window: ${B00T_WINDOW_NAME:-hermes-agent}\n"
    MSG+="fzf: $(fzf --version 2>/dev/null || echo 'not found')\n"
    MSG+="Whiptail: $(which whiptail 2>/dev/null || echo 'not found')\n"
    MSG+="\nFloating panes visible: $(zellij action are-floating-panes-visible 2>/dev/null || echo 'unknown')"
    just zellij-modal "📊 System Status" "$MSG"

# List all zellij recipes
zellij-help:
    @echo "🥾 Zellij Interactive Modal Commands"
    @echo ""
    @echo "  just zellij-modal                - Default confirm dialog"
    @echo "  just zellij-modal 'Title' 'Msg'  - Custom dialog"
    @echo "  just zellij-deploy-confirm       - Deploy confirmation"
    @echo "  just zellij-status               - System status modal"
    @echo ""
    @echo "🎯 Scripts:"
    @echo "  _b00t_/scripts/zellij-modal.sh         - Core modal script"
    @echo "  _b00t_/scripts/zellij-modal-runner.sh   - Zellij launcher"
    @echo "  _b00t_/scripts/zellij-fzf-menu.sh      - fzf interactive menu"
    @echo ""
    @echo "⚠️  Requires: JUST_UNSTABLE=1 (or env var)"
    @echo ""

zellij-test:
    #!/bin/bash
    set -euo pipefail
    echo "🥾 Zellij Modal Test Suite"
    echo ""
    echo "1️⃣  Test: Floating pane visibility"
    if zellij action are-floating-panes-visible 2>/dev/null; then
        echo "   Status: visible ✅"
    else
        echo "   Status: hidden ❌"
    fi
    echo ""
    echo "2️⃣  Test: Session detection"
    if [ -n "${ZELLIJ_SESSION_NAME:-}" ]; then
        echo "   Session: $ZELLIJ_SESSION_NAME ✅"
    else
        echo "   Session: NOT DETECTED ❌"
    fi
    echo ""
    echo "3️⃣  Test: zellij run capability"
    PANE_ID=$(zellij run --close-on-exit -- echo "modal test" 2>/dev/null)
    if [ -n "$PANE_ID" ]; then
        echo "   Pane created: $PANE_ID ✅"
    else
        echo "   zellij run: FAILED ❌"
    fi
    echo ""
    echo "4️⃣  Test: fzf availability"
    FZF_VER=$(fzf --version 2>/dev/null || echo "")
    if [ -n "$FZF_VER" ]; then
        echo "   fzf: $FZF_VER ✅"
    else
        echo "   fzf: NOT FOUND ❌"
    fi
    echo ""
    echo "5️⃣  Test: whiptail availability"
    WHIP_VER=$(whiptail --version 2>&1 | head -1 2>/dev/null || echo "")
    if [ -n "$WHIP_VER" ]; then
        echo "   whiptail: $WHIP_VER ✅"
    else
        echo "   whiptail: NOT FOUND ❌"
    fi
    echo ""
    echo "✅ Zellij modal system ready"
