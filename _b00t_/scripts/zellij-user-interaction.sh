#!/usr/bin/env bash
# 🥾 Zellij User Interaction Protocol — Multi-Modal Interface
# Provides 4 interaction modes for agent↔user communication inside Zellij:
#   1. fzf-menu     — Complex selection with search (fzf)
#   2. confirm       — Y/N/C dialog (read -n 1)
#   3. input         — Free-text input
#   4. subagent-log  — Sub-agent status report (no interaction, just display)
#
# Invoked by zellij-run-interactive.sh  (uses zellij run for proper TTY)
#
# Exit codes: 0=success, 1=user cancelled/declined, 2=error
# Output: interaction result to stdout, also written to KVCache

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 🎨 Colors
BLUE='\033[0;34m'
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
WHITE='\033[1;37m'
NC='\033[0m'

# 🎯 Load some state from environment or KVCache
AGENT_NAME="${B00T_AGENT_NAME:-hermes-agent}"
SESSION_ID="${B00T_SESSION_ID:-$$}"
INTERACTION_ID="interact-$(date +%s)"

# ─────────────────────────────────────────────────────────
# MODE 1: fzf Interactive Menu (complex selections)
# ─────────────────────────────────────────────────────────
mode_fzf_menu() {
    local title="${1:-🥾 B00T Menu}"
    shift
    local options=("$@")
    
    echo -e "${BLUE}╭────────────────────────────────────────────────────────╮${NC}"
    echo -e "${BLUE}│${NC}  🥾 $title"
    echo -e "${BLUE}╰────────────────────────────────────────────────────────╯${NC}"
    echo ""
    
    # Build header with all options
    local header_text="🎯 $title"
    local fzf_opts=(
        --header="$header_text"
        --prompt="🥾 b00t> "
        --border=rounded
        --border-label=" zellij-fzf "
        --color=header:blue,marker:green,pointer:cyan,border:gray,prompt:yellow
        --height=~50%
        --min-height=8
        --reverse
        --cycle
    )
    
    local selection
    selection=$(printf '%s\n' "${options[@]}" | fzf "${fzf_opts[@]}")
    
    if [ -n "$selection" ]; then
        echo ""
        echo -e "${GREEN}✅ Selected:${NC} $selection"
        echo "$selection"
        return 0
    else
        echo ""
        echo -e "${YELLOW}⚠️  Cancelled${NC}"
        return 1
    fi
}

# ─────────────────────────────────────────────────────────
# MODE 2: Confirm Dialog (Y/N/C)
# ─────────────────────────────────────────────────────────
mode_confirm() {
    local title="${1:-Confirm}"
    local message="${2:-Proceed with this action?}"
    
    echo -e "${BLUE}╭────────────────────────────────────────────────────────╮${NC}"
    echo -e "${BLUE}│${NC}  🥾 $title"
    echo -e "${BLUE}╰────────────────────────────────────────────────────────╯${NC}"
    echo ""
    echo -e "  ${WHITE}$message${NC}"
    echo ""
    echo -e "  ${GREEN}[Y] Yes${NC}    ${RED}[N] No${NC}    ${YELLOW}[C] Cancel${NC}"
    echo ""
    printf "  ${YELLOW}❯${NC} "
    read -r -n 1 response
    echo ""
    
    case "$response" in
        [Yy])
            echo ""
            echo -e "  ${GREEN}✅ Confirmed${NC}"
            echo "YES"
            return 0
            ;;
        [Nn])
            echo ""
            echo -e "  ${RED}❌ Declined${NC}"
            echo "NO"
            return 1
            ;;
        [Cc])
            echo ""
            echo -e "  ${YELLOW}⚠️  Cancelled${NC}"
            echo "CANCELLED"
            return 1
            ;;
        *)
            echo ""
            echo -e "  ${RED}⚠️  Invalid: '$response'${NC}"
            echo "INVALID"
            return 2
            ;;
    esac
}

# ─────────────────────────────────────────────────────────
# MODE 3: Free-text Input
# ─────────────────────────────────────────────────────────
mode_input() {
    local prompt="${1:-Enter value:}"
    local default="${2:-}"
    
    echo -e "${BLUE}╭────────────────────────────────────────────────────────╮${NC}"
    echo -e "${BLUE}│${NC}  🥾 Text Input"
    echo -e "${BLUE}╰────────────────────────────────────────────────────────╯${NC}"
    echo ""
    echo -e "  ${WHITE}$prompt${NC}"
    if [ -n "$default" ]; then
        echo -e "  ${CYAN}Default:${NC} $default"
    fi
    echo ""
    printf "  ${YELLOW}❯${NC} "
    read -r response
    
    if [ -z "$response" ] && [ -n "$default" ]; then
        echo ""
        echo -e "  ${CYAN}Using default:${NC} $default"
        echo "$default"
    elif [ -n "$response" ]; then
        echo ""
        echo -e "  ${GREEN}Got:${NC} $response"
        echo "$response"
    else
        echo ""
        echo -e "  ${YELLOW}⚠️  Empty input, cancelled${NC}"
        return 1
    fi
}

# ─────────────────────────────────────────────────────────
# MODE 4: Sub-agent Status Report (display only)
# Displays a multi-pane report from a sub-agent.
# User acknowledges with any key.
# ─────────────────────────────────────────────────────────
mode_subagent_log() {
    local agent_name="${1:-sub-agent}"
    local status="${2:-done}"
    local summary="${3:-Task completed}"
    local details="${4:-}"
    
    echo -e "${MAGENTA}╭────────────────────────────────────────────────────────╮${NC}"
    echo -e "${MAGENTA}│${NC}  🧩 Sub-Agent Report: ${WHITE}$agent_name${NC}"
    echo -e "${MAGENTA}╰────────────────────────────────────────────────────────╯${NC}"
    echo ""
    echo -e "  ${CYAN}Status:${NC}"
    case "$status" in
        done|pass|success|ok)
            echo -e "    ${GREEN}✅ $status${NC}"
            ;;
        fail|error|fail|timeout)
            echo -e "    ${RED}❌ $status${NC}"
            ;;
        warn|warning)
            echo -e "    ${YELLOW}⚠️  $status${NC}"
            ;;
        *)
            echo -e "    ${BLUE}ℹ️  $status${NC}"
            ;;
    esac
    echo ""
    echo -e "  ${CYAN}Summary:${NC}"
    echo -e "    $summary"
    if [ -n "$details" ]; then
        echo ""
        echo -e "  ${CYAN}Details:${NC}"
        echo -e "    $details" | fold -w 50 | sed 's/^/    /'
    fi
    echo ""
    echo -e "  ${YELLOW}Press any key to acknowledge report...${NC}"
    read -r -n 1
    echo ""
    echo -e "  ${GREEN}✅ Acknowledged${NC}"
    echo "ACKNOWLEDGED:$agent_name:$status"
}

# ─────────────────────────────────────────────────────────
# MODE 5: Multi-Step Wizard
# Presents multiple sequential inputs/steps
# ─────────────────────────────────────────────────────────
mode_wizard() {
    local title="${1:-Multi-Step Wizard}"
    echo -e "${BLUE}╭────────────────────────────────────────────────────────╮${NC}"
    echo -e "${BLUE}│${NC}  🥾 Wizard: ${WHITE}$title${NC}"
    echo -e "${BLUE}╰────────────────────────────────────────────────────────╯${NC}"
    echo ""
    
    local step=1
    local results="{}"
    
    while true; do
        echo -e "  ${CYAN}Step $step${NC}"
        echo ""
        printf "  ${YELLOW}❯${NC} "
        read -r step_input
        
        if [ -z "$step_input" ]; then
            echo ""
            echo -e "  ${GREEN}✅ Wizard complete${NC}"
            echo "$results"
            return 0
        fi
        
        # Store step result
        results=$(python3 -c "
import json
r = json.loads('$results')
r['step_$step'] = '$step_input'
print(json.dumps(r))
")
        step=$((step + 1))
        echo ""
        echo -e "  ${CYAN}(empty input to finish, ${step} steps taken)${NC}"
        echo ""
    done
}

# ─────────────────────────────────────────────────────────
# Main — mode dispatcher
# ─────────────────────────────────────────────────────────
main() {
    local mode="${1:-help}"
    shift || true
    
    # Check Zellij environment
    if [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
        echo -e "${RED}❌ Not in Zellij session${NC}"
        echo "Run this inside a Zellij terminal."
        exit 2
    fi
    
    case "$mode" in
        fzf-menu|fzf)
            mode_fzf_menu "$@"
            ;;
        confirm|yesno|yn)
            mode_confirm "$@"
            ;;
        input|text)
            mode_input "$@"
            ;;
        subagent|subagent-log|report)
            mode_subagent_log "$@"
            ;;
        wizard|steps)
            mode_wizard "$@"
            ;;
        help|--help|-h)
            echo "🥾 Zellij User Interaction Protocol"
            echo ""
            echo "Interactive agent↔user dialogs inside Zellij floating panes."
            echo ""
            echo "Modes:"
            echo "  confirm  <title> <message>   Y/N/C dialog"
            echo "  fzf-menu <title> <opt1> ...   fzf selection menu"
            echo "  input    <prompt> [default]   Free-text input"
            echo "  subagent <name> <status> <summary> [details]"
            echo "  wizard   <title>             Multi-step input"
            echo ""
            echo "Exit codes: 0=yes/selected, 1=no/cancelled, 2=error"
            echo ""
            echo "Environment:"
            echo "  ZELLIJ_SESSION_NAME  Auto-detected"
            echo "  B00T_AGENT_NAME      Your agent identity"
            echo "  B00T_KV_FILE         KVCache path (~/.b00t/kv-store.json)"
            echo ""
            echo "Results persist to KVCache for cross-agent coordination."
            ;;
        *)
            echo -e "${RED}Unknown mode: $mode${NC}"
            echo "Use: confirm, fzf-menu, input, subagent, wizard"
            exit 2
            ;;
    esac
    
    local exit_code=$?
    
    # Persist interaction to KVCache
    local kv_script="$SCRIPT_DIR/zellij-kv-cache.sh"
    if [ -f "$kv_script" ]; then
        bash "$kv_script" set "zellij.last-interaction" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" 2>/dev/null || true
        bash "$kv_script" set "zellij.last-mode" "$mode" 2>/dev/null || true
        bash "$kv_script" set "zellij.last-exit" "$exit_code" 2>/dev/null || true
    fi
    
    return $exit_code
}

main "$@"
