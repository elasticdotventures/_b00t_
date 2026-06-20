#!/usr/bin/env bash
# DEPRECATED: see b00t zellij {menu,confirm,input} (Rust binary in b00t-cli/src/commands/zellij.rs)
# 🥾 Zellij Interactive Runner
# Launches any interactive dialog in a Zellij floating pane with proper TTY.
# Uses zellij run (not write-chars) — critical for interactive programs.
#
# Usage: zellij-run-interactive.sh <mode> [args...]
#   zellij-run-interactive.sh confirm "Deploy?" "Press Y to continue"
#   zellij-run-interactive.sh fzf-menu "Select action" "Build" "Test" "Deploy"
#   zellij-run-interactive.sh input "Enter branch name:" "main"
#   zellij-run-interactive.sh subagent "worker-1" "pass" "PR #441 merged"
#   zellij-run-interactive.sh wizard "Configure deployment"

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INTERACTION_SCRIPT="$SCRIPT_DIR/zellij-user-interaction.sh"
WIDTH="${ZELLIJ_MODAL_WIDTH:-60%}"
HEIGHT="${ZELLIJ_MODAL_HEIGHT:-40%}"
PINNED="${ZELLIJ_MODAL_PINNED:-true}"

if [ ! -f "$INTERACTION_SCRIPT" ]; then
    echo "❌ Interaction script not found: $INTERACTION_SCRIPT"
    exit 2
fi

if [ -z "${ZELLIJ:-}" ]; then
    echo "❌ Not running in Zellij session"
    echo "Start zellij first."
    exit 2
fi

MODE="${1:-help}"
shift || true

echo "🥾 Launching Zellij interactive: $MODE"
echo "Pane: floating (${WIDTH}x${HEIGHT})"

# 🎯 CRITICAL: Use zellij run for interactive programs
# zellij action new-pane does NOT provide TTY — fzf, read, whiptail all fail silently
# zellij run creates a pane WITH proper TTY — interactive programs work
zellij run \
    --floating \
    --width "$WIDTH" \
    --height "$HEIGHT" \
    --pinned "$PINNED" \
    --close-on-exit \
    -- bash "$INTERACTION_SCRIPT" "$MODE" "$@"

echo "✅ Pane created (will close on user response)"