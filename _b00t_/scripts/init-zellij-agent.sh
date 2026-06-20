#!/usr/bin/env bash
# 🥾 Agent Startup: Zellij Detection
# Run during agent init to detect Zellij, persist to KVCache,
# and set up the interaction protocol environment.
#
# Detects:
#   - Zellij session presence (via env vars)
#   - Window name capability
#   - Floating pane support
#   - fzf / whiptail availability
#
# Stores to KVCache (~/.b00t/kv-store.json):
#   zellij.active               true|false
#   zellij.session              Session name
#   zellij.pane                 Current pane ID
#   zellij.window               Agent window name
#   zellij.modal.width          60%
#   zellij.modal.height         40%
#   zellij.fzf                  Version or "none"
#   zellij.whiptail              Version or "none"
#   zellij.last-seen            ISO timestamp
#
# Environment variables set:
#   B00T_ZELLIJ_MODE            enabled|disabled
#   B00T_WINDOW_NAME            agent-name:task-context

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KV_SCRIPT="$SCRIPT_DIR/../zellij-gate/scripts/zellij-kv-cache.sh"
AGENT_TYPE="${B00T_AGENT_TYPE:-hermes-agent}"
SESSION_ID="${B00T_SESSION_ID:-$$}"
TASK_CONTEXT="${B00T_TASK_CONTEXT:-general}"

# ══════════════════════════════════════════════════════════
# Detects Zellij session, writes env + KVCache, sets window
# ══════════════════════════════════════════════════════════
init_zellij() {
    if [ -n "${ZELLIJ_SESSION_NAME:-}" ]; then
        # ── Zellij detected ──────────────────────────────────────
        echo "🥾 Zellij workspace detected: $ZELLIJ_SESSION_NAME"
        export B00T_ZELLIJ_MODE="enabled"
        
        # Generate window name
        local window_name="${AGENT_TYPE}-${SESSION_ID}:${TASK_CONTEXT}"
        export B00T_WINDOW_NAME="$window_name"
        
        # Rename Zellij window
        zellij action rename-pane "$window_name" 2>/dev/null || true
        
        # ── Persist to KVCache ────────────────────────────────────
        if [ -f "$KV_SCRIPT" ]; then
            bash "$KV_SCRIPT" set "zellij.active" "true" 2>/dev/null || true
            bash "$KV_SCRIPT" set "zellij.session" "$ZELLIJ_SESSION_NAME" 2>/dev/null || true
            bash "$KV_SCRIPT" set "zellij.pane" "${ZELLIJ_PANE_ID:-0}" 2>/dev/null || true
            bash "$KV_SCRIPT" set "zellij.window" "$window_name" 2>/dev/null || true
            bash "$KV_SCRIPT" set "zellij.modal.width" "60%" 2>/dev/null || true
            bash "$KV_SCRIPT" set "zellij.modal.height" "40%" 2>/dev/null || true
            bash "$KV_SCRIPT" set "zellij.last-seen" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" 2>/dev/null || true
        fi
        
        # ── Detect tool availability ──────────────────────────────
        local fzf_ver whiptail_ver
        fzf_ver=$(fzf --version 2>/dev/null || echo "none")
        whiptail_ver=$(whiptail --version 2>&1 | head -1 2>/dev/null || echo "none")
        
        if [ -f "$KV_SCRIPT" ]; then
            bash "$KV_SCRIPT" set "zellij.fzf" "$fzf_ver" 2>/dev/null || true
            bash "$KV_SCRIPT" set "zellij.whiptail" "$whiptail_ver" 2>/dev/null || true
        fi
        
        echo "  Window: $window_name"
        echo "  fzf: $fzf_ver"
        echo "  whiptail: $whiptail_ver"
        echo ""
        echo "✅ Zellij interaction protocol active — use _b00t_/zellij-gate/scripts/zellij-run-interactive.sh"
        echo ""
    else
        # ── No Zellij ─────────────────────────────────────────────
        echo "📱 Standard terminal mode (no Zellij)"
        export B00T_ZELLIJ_MODE="disabled"
        
        if [ -f "$KV_SCRIPT" ]; then
            bash "$KV_SCRIPT" set "zellij.active" "false" 2>/dev/null || true
        fi
    fi
}

init_zellij

# ══════════════════════════════════════════════════════════
# Non-interactive fallback: if not in Zellij, show what
# the agent would show if Zellij were available.
# ══════════════════════════════════════════════════════════
if [ "${B00T_ZELLIJ_MODE:-disabled}" = "disabled" ]; then
    echo "  (Zellij modals unavailable — agent will use stdin/stdout instead)"
fi

# Print KVCache state summary
if [ -f "$KV_SCRIPT" ]; then
    echo ""
    echo "📊 KVCache state:"
    bash "$KV_SCRIPT" list 2>/dev/null | grep zellij || echo "  (no zellij keys yet)"
fi