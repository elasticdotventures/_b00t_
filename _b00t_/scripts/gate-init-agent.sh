#!/usr/bin/env bash
# 🥾 Zellij Gate-Integrated Agent Init
# Wraps init-zellij-agent.sh + mandatory gate activation.
# When an agent starts in Zellij, the gate is automatically activated.
#
# Usage: bash gate-init-agent.sh [--bypass-gate]
#
# The gate becomes mandatory after init. Agent cannot proceed without
# user interaction through the fzf menu.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INIT_SCRIPT="$SCRIPT_DIR/init-zellij-agent.sh"
GATE_SCRIPT="$(dirname "$(dirname "$SCRIPT_DIR")")/zellij-gate/gates/zellij-mandatory-gate.sh"

# ── Run standard init ───────────────────────────────────────────
if [ -f "$INIT_SCRIPT" ]; then
    bash "$INIT_SCRIPT"
else
    echo "⚠️  init-zellij-agent.sh not found: $INIT_SCRIPT"
fi

# ── Activate mandatory gate ─────────────────────────────────────
if [ -n "${ZELLIJ_SESSION_NAME:-}" ]; then
    echo ""
    echo "🛡️ Activating Zellij Interaction Gate..."
    
    # Set gate environment
    export B00T_ZELLIJ_GATE="enabled"
    export B00T_GATE_TIMEOUT="120000"
    export B00T_GATE_AUDIT="${HOME}/.b00t/audit/zellij-gate.jsonl"
    
    # Write gate activation to KVCache
    KV_SCRIPT="$SCRIPT_DIR/../zellij-gate/scripts/zellij-kv-cache.sh"
    if [ -f "$KV_SCRIPT" ]; then
        bash "$KV_SCRIPT" set "zellij.gate.active" "true" 2>/dev/null || true
        bash "$KV_SCRIPT" set "zellij.gate.mode" "mandatory" 2>/dev/null || true
        bash "$KV_SCRIPT" set "zellij.gate.activated-at" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" 2>/dev/null || true
    fi
    
    # Check if bypass is requested
    if [ "${1:-}" = "--bypass-gate" ]; then
        echo "⚠️  Gate BYPASSED (--bypass-gate flag)"
        echo "Agent will proceed without mandatory interaction."
        export B00T_ZELLIJ_GATE="bypassed"
        if [ -f "$KV_SCRIPT" ]; then
            bash "$KV_SCRIPT" set "zellij.gate.bypassed" "true" 2>/dev/null || true
        fi
        exit 0
    fi
    
    echo ""
    echo "🛡️ Gate is MANDATORY — agent requires user input before proceeding."
    echo ""
    echo "   • just gate-build-test        — Build & test (gate required)"
    echo "   • just gate-deploy-staging    — Deploy to staging (gate required)"
    echo "   • just gate-deploy-production — Deploy to production (double gate)"
    echo "   • just gate-code-review       — Code review (gate required)"
    echo "   • just gate-diagnostics       — System diagnostics (gate required)"
    echo "   • just gate-task-list         — Task management (gate required)"
    echo ""
    echo "To bypass: bash gate-init-agent.sh --bypass-gate"
    
    # Optionally trigger the gate immediately on init
    if [ "${B00T_GATE_AUTO_FIRE:-false}" = "true" ]; then
        echo ""
        echo "🔥 Auto-firing gate on init..."
        bash "$GATE_SCRIPT" --important --action="agent-init" 2>&1 || true
    fi
else
    echo ""
    echo "📱 No Zellij detected — gate bypassed (stdin/stdout mode)"
fi
