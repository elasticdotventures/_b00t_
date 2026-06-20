# 🥾 Zellij Governance Gates — just recipes
# Mandatory interaction gate system for b00t agents in Zellij.
#
# The zellij-interaction gate fires BEFORE any agent action.
# Agent CANNOT proceed without user input through fzf menu.
#
# Eisenhower routing:
#   URGENT+IMPORTANT     → quick confirm (Y/N)
#   IMPORTANT+NOT-URGENT → fzf action menu
#   URGENT+NOT-IMPORTANT → sub-agent report
#   NOT-URGENT+NOT-IMPORT → block
#
# Usage:
#   JUST_UNSTABLE=1 just zellij-gate::gate-check              # Standalone gate check
#   JUST_UNSTABLE=1 just zellij-gate::gate-check-urgent       # Urgent mode
#   JUST_UNSTABLE=1 just zellij-gate::gate-audit              # View audit log
#   JUST_UNSTABLE=1 just zellij-gate::gate-kvcache            # View gate KVCache state
#   JUST_UNSTABLE=1 just zellij-gate::gate-preflight <action> # Pre-flight check before action

set shell := ["bash", "-cu"]

B00T_ROOT := "/home/brianh/.dotfiles"
GATE_SCRIPT := "{{ B00T_ROOT }}/_b00t_/gates/zellij-mandatory-gate.sh"
AUDIT_FILE := "{{ env_var('HOME') }}/.b00t/audit/zellij-gate.jsonl"

# ── Default: Standalone gate check (interactive fzf menu) ──────
gate-check action="agent-action":
    #!/bin/bash
    set -euo pipefail
    if [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
        echo "📱 Not in Zellij — gate bypassed"
        exit 0
    fi
    bash "{{ GATE_SCRIPT }}" --important --action="{{ action }}"
    EXIT_CODE=$?
    echo ""
    echo "Gate result: exit_code=$EXIT_CODE"
    exit $EXIT_CODE

# ── Urgent gate (quick confirm only, no menu) ──────────────────
gate-check-urgent action="urgent-action":
    #!/bin/bash
    set -euo pipefail
    if [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
        echo "📱 Not in Zellij — gate bypassed"
        exit 0
    fi
    bash "{{ GATE_SCRIPT }}" --urgent --important --action="{{ action }}"
    EXIT_CODE=$?
    echo ""
    echo "Gate result (urgent): exit_code=$EXIT_CODE"
    exit $EXIT_CODE

# ── Pre-flight check: run before any just action ────────────────
gate-preflight action="command":
    #!/bin/bash
    set -euo pipefail
    echo "🥾 Zellij Interaction Gate — Pre-flight Check"
    echo "Action: {{ action }}"
    echo ""
    
    if [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
        echo "📱 Not in Zellij — gate bypassed (stdin/stdout mode)"
        echo "Proceeding without interactive gate..."
        exit 0
    fi
    
    # Check if gate was already passed for this session
    if [ -f "{{ env_var('HOME') }}/.b00t/kv-store.json" ]; then
        LAST_RESULT=$(grep -o '"zellij.gate.last-result": "[^"]*"' ~/.b00t/kv-store.json 2>/dev/null | cut -d'"' -f4 || echo "")
        LAST_TIME=$(grep -o '"zellij.gate.last-timestamp": "[^"]*"' ~/.b00t/kv-store.json 2>/dev/null | cut -d'"' -f4 || echo "")
        if [ "$LAST_RESULT" = "ALLOW" ]; then
            echo "✅ Gate already passed ($LAST_TIME)"
            echo "Proceeding with: {{ action }}"
            exit 0
        fi
    fi
    
    # Gate not passed — fire interactive menu
    bash "{{ GATE_SCRIPT }}" --important --action="{{ action }}"
    EXIT_CODE=$?
    
    case $EXIT_CODE in
        0) echo "✅ Gate: ALLOW — proceeding with {{ action }}" ;;
        1) echo "❌ Gate: DENY — blocked by user" ; exit 1 ;;
        2) echo "🔄 Gate: HOOK — event registered, agent notified" ; exit 2 ;;
        3) echo "⚠️  Gate: ERROR — system failure" ; exit 3 ;;
    esac

# ── View audit trail ────────────────────────────────────────────
gate-audit lines="20":
    #!/bin/bash
    set -euo pipefail
    AUDIT="{{ AUDIT_FILE }}"
    if [ -f "$AUDIT" ]; then
        echo "🥾 Zellij Gate Audit Trail"
        echo "File: $AUDIT"
        echo "──────────────────────────────────────────────"
        tail -{{ lines }} "$AUDIT" | while IFS= read -r line; do
            echo "$line" | python3 -m json.tool 2>/dev/null || echo "$line"
        done
    else
        echo "No audit file yet: $AUDIT"
    fi

# ── View KVCache gate state ─────────────────────────────────────
gate-kvcache:
    #!/bin/bash
    set -euo pipefail
    KV="${HOME}/.b00t/kv-store.json"
    if [ -f "$KV" ]; then
        echo "🥾 Gate KVCache State"
        echo "──────────────────────────────────────────────"
        # Use jq for safe JSON parsing (avoids just syntax conflicts)
        if command -v jq &>/dev/null; then
            jq -r 'to_entries[] | select(.key | test("gate")) | "  \(.key): \(.value)"' "$KV" 2>/dev/null
        else
            grep -o '"zellij\.gate\.[^"]*": "[^"]*"' "$KV" 2>/dev/null || echo "  (no gate keys)"
        fi
    else
        echo "No KVCache file: $KV"
    fi

# ── Gate status summary ─────────────────────────────────────────
gate-status:
    #!/bin/bash
    set -euo pipefail
    B00T_ROOT="/home/brianh/.dotfiles"
    AUDIT="${HOME}/.b00t/audit/zellij-gate.jsonl"
    
    echo "🥾 Zellij Interaction Gate — Status"
    echo "──────────────────────────────────────────────"
    
    if [ -n "${ZELLIJ_SESSION_NAME:-}" ]; then
        echo "Session:    $ZELLIJ_SESSION_NAME (active)"
        echo "Pane:       ${ZELLIJ_PANE_ID:-?}"
        echo "Window:     ${B00T_WINDOW_NAME:-?}"
        echo ""
        echo "Gate mode:  mandatory"
        echo "Trigger:    on-action"
        echo "Condition:  zellij-active ✅"
    else
        echo "Session:    none (Zellij not detected)"
        echo "Gate mode:  bypassed"
    fi
    
    echo ""
    echo "Gate script: $B00T_ROOT/_b00t_/gates/zellij-mandatory-gate.sh"
    echo "Audit file:  $AUDIT"
    
    if [ -f "$AUDIT" ]; then
        COUNT=$(wc -l < "$AUDIT")
        echo "Audit entries: $COUNT"
    fi

# ── All gate recipes ────────────────────────────────────────────
gate-help:
    @echo "🥾 Zellij Governance Gate Commands"
    @echo ""
    @echo "  just zellij-gate::gate-check               - Interactive gate check (fzf menu)"
    @echo "  just zellij-gate::gate-check-urgent        - Urgent confirm gate (Y/N only)"
    @echo "  just zellij-gate::gate-preflight <action>  - Mandatory pre-flight before any action"
    @echo "  just zellij-gate::gate-audit               - View audit trail"
    @echo "  just zellij-gate::gate-kvcache             - View gate state in KVCache"
    @echo "  just zellij-gate::gate-status              - Gate system status"
    @echo ""
    @echo "⚠️  Requires: JUST_UNSTABLE=1"
    @echo ""
    @echo "Eisenhower routing:"
    @echo "  --urgent --important     → quick confirm (Y/N)"
    @echo "  --important (default)    → fzf action menu"
    @echo "  --urgent                 → sub-agent report"
    @echo "  (no flags)               → blocked"
