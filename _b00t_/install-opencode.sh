#!/bin/bash
# install-opencode.sh — detect + fix: ensures opencode + goal plugin are present
set -euo pipefail

# 🤓 Prefer bun (opencode's native PM), fallback pnpm → npm
if command -v bun &>/dev/null; then PM="bun add -g"
elif command -v pnpm &>/dev/null; then PM="pnpm add -g"
else PM="npm install -g"; fi

# ── Install opencode if missing ──
if ! command -v opencode &>/dev/null; then
    echo "📦 Installing opencode-ai..."
    $PM opencode-ai
fi

# ── Install goal plugin if missing ──
if ! node -e "require('opencode-goal-plugin')" 2>/dev/null; then
    echo "📦 Installing opencode-goal-plugin..."
    $PM opencode-goal-plugin 2>/dev/null || {
        cd ~/.dotfiles/vendor/opencode-goal-plugin && bun link --global 2>/dev/null || true
    }
fi

command -v opencode &>/dev/null || { echo "❌ opencode not found"; exit 1; }
echo "✅ opencode ready"
