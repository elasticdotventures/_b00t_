#!/bin/bash
# opencode-preflight.sh — ensure required plugins are installed before launch
# Called by opencode.runtime.tomllmd hook_pre

set -euo pipefail
CONFIG="${HOME}/.config/opencode/opencode.json"
NEED_INSTALL=false

# Check if @prevalentware/opencode-goal-plugin is in the config
if [ -f "$CONFIG" ]; then
    if ! grep -q "prevalentware/opencode-goal-plugin" "$CONFIG" 2>/dev/null; then
        NEED_INSTALL=true
    fi
else
    NEED_INSTALL=true
fi

if $NEED_INSTALL; then
    echo "[b00t] goal plugin not found — installing..."
    cd "${HOME}/.config/opencode"
    npm install @prevalentware/opencode-goal-plugin 2>/dev/null
    opencode plugin -g ./node_modules/@prevalentware/opencode-goal-plugin 2>/dev/null
    echo "[b00t] goal plugin installed — /goal available this session"
fi

# Also check b00t plugin
if [ -f "$CONFIG" ]; then
    if ! grep -q "b00t-opencode-plugin" "$CONFIG" 2>/dev/null; then
        echo "[b00t] b00t plugin not found — installing..."
        mkdir -p "${HOME}/.config/opencode/node_modules/b00t-opencode-plugin"
        cp "${HOME}/.dotfiles/b00t-opencode-plugin/plugin.js" "${HOME}/.config/opencode/node_modules/b00t-opencode-plugin/"
        cp "${HOME}/.dotfiles/b00t-opencode-plugin/package.json" "${HOME}/.config/opencode/node_modules/b00t-opencode-plugin/"
        opencode plugin -g ./node_modules/b00t-opencode-plugin 2>/dev/null
        echo "[b00t] b00t plugin installed — /b00t available this session"
    fi
fi

exit 0
