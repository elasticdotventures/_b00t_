#!/usr/bin/env bash
# 🥾 Zellij Mandatory Interaction Gate Hook
# Fires BEFORE any agent action when Zellij is detected.
# Presents fzf menu as mandatory step — agent CANNOT proceed without user input.
#
# Eisenhower routing:
#   URGENT+IMPORTANT   → quick fzf confirm (Y/N)
#   NOT-URG+IMPORTANT  → fzf action menu (select route)
#   URGENT+NOT-IMPORT  → sub-agent report (delegate)
#   NOT-URG+NOT-IMPORT → block (deny)
#
# Usage:
#   bash zellij-mandatory-gate.sh [--urgent] [--important] [--action=<name>]
#
# Exit codes:
#   0 = Allow (user approved)
#   1 = Deny  (user blocked or timeout)
#   2 = Hook  (register event, agent waits)
#   3 = Error (gate system failure)
#
# Audit: ~/.b00t/audit/zellij-gate.jsonl

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
INTERACTIVE_RUNNER="$(dirname "$SCRIPT_DIR")/scripts/zellij-run-interactive.sh"
KV_CACHE="$(dirname "$SCRIPT_DIR")/scripts/zellij-kv-cache.sh"
AUDIT_FILE="${HOME}/.b00t/audit/zellij-gate.jsonl"
TIMESTAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
SESSION="${ZELLIJ_SESSION_NAME:-unknown}"
AGENT="${B00T_AGENT_TYPE:-hermes-agent}"

# ── Parse Arguments ─────────────────────────────────────────────
URGENT=false
IMPORTANT=false
ACTION="${*:-unknown}"

for arg in "$@"; do
    case "$arg" in
        --urgent)    URGENT=true ;;
        --important) IMPORTANT=true ;;
        --action=*)  ACTION="${arg#--action=}" ;;
    esac
done

# ── Gate Detection ───────────────────────────────────────────────
if [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
    echo "📱 No Zellij session — gate bypassed (stdin/stdout mode)"
    echo "ALLOW:no-zellij"
    exit 0  # Allow — no Zellij means gate doesn't apply
fi

if [ ! -f "$INTERACTIVE_RUNNER" ]; then
    echo "❌ Gate runner not found: $INTERACTIVE_RUNNER"
    echo "ERROR:missing-runner"
    exit 3
fi

# ── Audit Helper ─────────────────────────────────────────────────
audit_log() {
    local result="$1" selection="$2" exit_code="$3"
    mkdir -p "$(dirname "$AUDIT_FILE")"
    echo "{\"timestamp\":\"$TIMESTAMP\",\"session\":\"$SESSION\",\"agent\":\"$AGENT\",\"action\":\"$ACTION\",\"result\":\"$result\",\"selection\":\"$selection\",\"exit_code\":$exit_code}" >> "$AUDIT_FILE"
}

# ── Write Gate State to KVCache ──────────────────────────────────
write_gate_state() {
    local result="$1" selection="$2" exit_code="$3"
    if [ -f "$KV_CACHE" ]; then
        bash "$KV_CACHE" set "zellij.gate.active" "true" 2>/dev/null || true
        bash "$KV_CACHE" set "zellij.gate.last-result" "$result" 2>/dev/null || true
        bash "$KV_CACHE" set "zellij.gate.last-selection" "$selection" 2>/dev/null || true
        bash "$KV_CACHE" set "zellij.gate.last-exit-code" "$exit_code" 2>/dev/null || true
        bash "$KV_CACHE" set "zellij.gate.last-timestamp" "$TIMESTAMP" 2>/dev/null || true
    fi
}

# ── Eisenhower Routing ───────────────────────────────────────────
# Determine quadrant and route accordingly

if $URGENT && $IMPORTANT; then
    # ═══ URGENT + IMPORTANT: Quick confirm (Allow) ═══════════════
    echo "🥾 Gate: URGENT+IMPORTANT → confirm dialog"
    
    RESULT=$(bash "$INTERACTIVE_RUNNER" confirm \
        "🥾 $ACTION" \
        "URGENT — Press Y to proceed, N to block" 2>&1) || GATE_EXIT=$?
    GATE_EXIT=${GATE_EXIT:-$?}
    
    if [ "$GATE_EXIT" = "0" ]; then
        audit_log "ALLOW" "confirmed" 0
        write_gate_state "ALLOW" "confirmed" 0
        echo "ALLOW:user-confirmed"
        exit 0
    else
        audit_log "DENY" "user-blocked" "$GATE_EXIT"
        write_gate_state "DENY" "user-blocked" "$GATE_EXIT"
        echo "DENY:user-blocked"
        exit 1
    fi

elif $IMPORTANT && ! $URGENT; then
    # ═══ IMPORTANT + NOT URGENT: fzf menu (Hook) ════════════════
    echo "🥾 Gate: IMPORTANT+NOT-URGENT → fzf action menu"
    
    # Build menu from gate TOML items
    MENU_ITEMS=(
        "🔨 Build & Test"
        "🚀 Deploy to Staging"
        "🔥 Deploy to Production"
        "👁 Code Review"
        "📊 System Diagnostics"
        "📋 Task Management"
        "🤖 Dispatch Sub-agent"
        "❌ Cancel"
    )
    
    SELECTION=$(bash "$INTERACTIVE_RUNNER" fzf-menu \
        "🥾 Action Required — $ACTION" \
        "${MENU_ITEMS[@]}" 2>&1) || GATE_EXIT=$?
    GATE_EXIT=${GATE_EXIT:-$?}
    
    if [ "$GATE_EXIT" != "0" ] || [ -z "$SELECTION" ]; then
        audit_log "DENY" "cancelled-or-empty" "$GATE_EXIT"
        write_gate_state "DENY" "cancelled" "$GATE_EXIT"
        echo "DENY:user-cancelled"
        exit 1
    fi
    
    # Route selection
    case "$SELECTION" in
        *"Build"*)
            audit_log "ALLOW" "build-test" 0
            write_gate_state "ALLOW" "build-test" 0
            echo "ALLOW:build-test"
            exit 0
            ;;
        *"Staging"*)
            audit_log "ALLOW" "deploy-staging" 0
            write_gate_state "ALLOW" "deploy-staging" 0
            echo "ALLOW:deploy-staging"
            exit 0
            ;;
        *"Production"*)
            # Production requires double confirm
            CONFIRM=$(bash "$INTERACTIVE_RUNNER" confirm \
                "🔥 DEPLOY PRODUCTION" \
                "THIS IS PRODUCTION. Are you absolutely sure?" 2>&1) || GATE_EXIT=$?
            GATE_EXIT=${GATE_EXIT:-$?}
            if [ "$GATE_EXIT" = "0" ]; then
                audit_log "ALLOW" "deploy-prod-confirmed" 0
                write_gate_state "ALLOW" "deploy-prod-confirmed" 0
                echo "ALLOW:deploy-prod"
                exit 0
            else
                audit_log "DENY" "deploy-prod-cancelled" "$GATE_EXIT"
                write_gate_state "DENY" "deploy-prod-cancelled" "$GATE_EXIT"
                echo "DENY:deploy-prod-cancelled"
                exit 1
            fi
            ;;
        *"Review"*)
            audit_log "ALLOW" "code-review" 0
            write_gate_state "ALLOW" "code-review" 0
            echo "ALLOW:code-review"
            exit 0
            ;;
        *"Diagnostics"*)
            audit_log "ALLOW" "diagnostics" 0
            write_gate_state "ALLOW" "diagnostics" 0
            echo "ALLOW:diagnostics"
            exit 0
            ;;
        *"Task"*)
            audit_log "ALLOW" "task-management" 0
            write_gate_state "ALLOW" "task-management" 0
            echo "ALLOW:task-management"
            exit 0
            ;;
        *"Sub-agent"*)
            audit_log "HOOK" "subagent-dispatch" 0
            write_gate_state "HOOK" "subagent-dispatch" 0
            echo "HOOK:subagent-dispatch"
            exit 2
            ;;
        *"Cancel"*)
            audit_log "DENY" "user-cancelled" 1
            write_gate_state "DENY" "user-cancelled" 1
            echo "DENY:user-cancelled"
            exit 1
            ;;
        *)
            audit_log "DENY" "unknown-selection:$SELECTION" 1
            write_gate_state "DENY" "unknown-selection" 1
            echo "DENY:unknown-selection"
            exit 1
            ;;
    esac

elif $URGENT && ! $IMPORTANT; then
    # ═══ URGENT + NOT IMPORTANT: Sub-agent report (Hook) ═════════
    echo "🥾 Gate: URGENT+NOT-IMPORTANT → sub-agent report"
    
    bash "$INTERACTIVE_RUNNER" subagent \
        "gate-delegate" "info" \
        "Delegated: $ACTION" \
        "This task has been delegated to a sub-agent. You will be notified when complete." 2>&1 || true
    
    audit_log "HOOK" "delegated-to-subagent" 0
    write_gate_state "HOOK" "delegated-to-subagent" 0
    echo "HOOK:delegated-to-subagent"
    exit 2

else
    # ═══ NOT URGENT + NOT IMPORTANT: Deny (block) ══════════════
    echo "🥾 Gate: NOT-URGENT+NOT-IMPORTANT → blocked"
    
    bash "$INTERACTIVE_RUNNER" subagent \
        "gate-blocked" "deny" \
        "Blocked: $ACTION" \
        "This action was blocked by the Zellij interaction gate (low priority)." 2>&1 || true
    
    audit_log "DENY" "low-priority-block" 0
    write_gate_state "DENY" "low-priority-block" 0
    echo "DENY:low-priority"
    exit 1
fi
