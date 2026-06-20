#!/usr/bin/env bash
# 🥾 Zellij Rust Wrapper — thin shim that redirects old bash scripts to `b00t zellij`
#
# Usage: zellij-rust-wrapper.sh <mode> [args...]
#
# Modes (mapped to b00t zellij subcommands):
#   input     → b00t zellij input   --prompt <arg1> [--default <arg2>]
#   menu      → b00t zellij menu    --title  <arg1> --items '<json-array from remaining args>'
#   confirm   → b00t zellij confirm --title  <arg1> --prompt <arg2>
#   subagent  → b00t zellij subagent --title <arg1> --content '<arg2+arg3...>'
#   wizard    → b00t zellij wizard  --title  <arg1> [--file <arg2>]
#   detect    → b00t zellij detect (prints JSON, exit 0=inside, 1=outside)
#
# Exit code: propagated from b00t zellij.

set -euo pipefail

MODE="${1:-}"
shift || true

# Auto-detect Zellij
if [ -z "${ZELLIJ_SESSION_NAME:-}" ]; then
    if [ "$MODE" = "detect" ] || [ "$MODE" = "init" ]; then
        exec b00t zellij detect
    fi
    echo "📱 Not in Zellij — b00t zellij will fall back to stdin/stdout" >&2
fi

build_json_array() {
    local first=true
    printf '['
    local idx=0
    for item in "$@"; do
        if $first; then first=false; else printf ','; fi
        # Escape for JSON string
        local escaped="${item//\\/\\\\}"
        escaped="${escaped//\"/\\\"}"
        printf '{"key":"item_%d","label":"%s"}' "$idx" "$escaped"
        idx=$((idx + 1))
    done
    printf ']'
}

case "$MODE" in
    detect|init)
        exec b00t zellij detect
        ;;
    input|text)
        PROMPT="${1:-}"
        DEFAULT="${2:-}"
        ARGS=()
        [ -n "$PROMPT" ] && ARGS+=(--prompt "$PROMPT")
        [ -n "$DEFAULT" ] && ARGS+=(--default "$DEFAULT")
        exec b00t zellij input "${ARGS[@]}"
        ;;
    menu|fzf-menu|fzf)
        TITLE="${1:-b00t Menu}"
        shift || true
        ITEMS_JSON=$(build_json_array "$@")
        exec b00t zellij menu --title "$TITLE" --items "$ITEMS_JSON"
        ;;
    confirm|yesno|yn)
        TITLE="${1:-b00t Confirm}"
        PROMPT="${2:-Proceed?}"
        exec b00t zellij confirm --title "$TITLE" --prompt "$PROMPT"
        ;;
    subagent|subagent-log|report)
        NAME="${1:-sub-agent}"
        STATUS="${2:-done}"
        SUMMARY="${3:-}"
        DETAILS="${4:-}"
        CONTENT="Status: ${STATUS}"
        [ -n "$SUMMARY" ] && CONTENT="${CONTENT}\nSummary: ${SUMMARY}"
        [ -n "$DETAILS" ] && CONTENT="${CONTENT}\nDetails: ${DETAILS}"
        exec b00t zellij subagent --title "${NAME}" --content "${CONTENT}"
        ;;
    wizard|steps)
        TITLE="${1:-b00t Wizard}"
        FILE="${2:-}"
        ARGS=(--title "$TITLE")
        [ -n "$FILE" ] && ARGS+=(--file "$FILE")
        exec b00t zellij wizard "${ARGS[@]}"
        ;;
    help|--help|-h|"")
        echo "🥾 zellij-rust-wrapper — redirects to b00t zellij"
        echo ""
        echo "Modes:"
        echo "  detect              → b00t zellij detect"
        echo "  input <prompt> [def] → b00t zellij input"
        echo "  menu <title> <items> → b00t zellij menu"
        echo "  confirm <title> <msg> → b00t zellij confirm"
        echo "  subagent <name> <status> <summary> [details] → b00t zellij subagent"
        echo "  wizard <title> [file] → b00t zellij wizard"
        echo ""
        echo "Exit code propagated from b00t zellij."
        ;;
    *)
        echo "❌ Unknown mode: $MODE" >&2
        echo "Use: detect, input, menu, confirm, subagent, wizard" >&2
        exit 2
        ;;
esac
