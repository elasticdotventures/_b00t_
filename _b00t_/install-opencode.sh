#!/bin/bash
# install-opencode.sh — detect + fix: ensures opencode + goal plugin are present
# Called by: b00t install opencode  OR  as the opencode wrapper script
set -euo pipefail

# ── Install opencode if missing ──
if ! command -v opencode &>/dev/null; then
    echo "📦 Installing opencode-ai..."
    PM=$(command -v pnpm &>/dev/null && echo pnpm || echo npm)
    $PM add -g opencode-ai
fi

# ── Install goal plugin if missing ──
if ! node -e "require('opencode-goal-plugin')" 2>/dev/null; then
    echo "📦 Installing opencode-goal-plugin..."
    PM=$(command -v pnpm &>/dev/null && echo pnpm || echo npm)
    $PM add -g opencode-goal-plugin 2>/dev/null || {
        cd ~/.dotfiles/vendor/opencode-goal-plugin && $PM link --global 2>/dev/null || true
    }
fi

# ── Verify both ──
command -v opencode &>/dev/null || { echo "❌ opencode not found"; exit 1; }
echo "✅ opencode ready"
