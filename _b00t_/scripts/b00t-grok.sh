#!/usr/bin/env bash
# b00t grok wrapper — auto-fallback to keyword RAG when backends are down.
# Transparently wraps `b00t-cli grok ask` with fallback to grok-fallback.sh.
# Usage: b00t-grok ask "query" [-t topic] [--rag backend]
set -euo pipefail

B00T_ROOT="${B00T_ROOT:-$HOME/.b00t}"
FALLBACK="$B00T_ROOT/_b00t_/scripts/grok-fallback.sh"

# Pass through to real b00t-cli grok
output=$(b00t-cli grok "$@" 2>&1) || true
rc=$?

# If grok succeeded with results, just output and exit
if echo "$output" | grep -q "📊 Found [1-9]"; then
    echo "$output"
    exit 0
fi

# If grok failed or returned 0 results, try fallback
echo "$output" >&2
echo "" >&2
echo "🔄 grok backends unavailable — trying keyword fallback..." >&2

# Extract query from args
query=""
topic=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        -t|--topic) topic="$2"; shift 2 ;;
        --rag) shift 2 ;;  # skip rag flag
        ask|digest|learn) shift ;;  # skip subcommand
        *) query="$1"; shift ;;
    esac
done

if [[ -z "$query" ]]; then
    echo "❌ fallback: could not extract query from args" >&2
    exit 1
fi

if [[ -n "$topic" ]]; then
    exec bash "$FALLBACK" --topic "$topic" "$query"
else
    exec bash "$FALLBACK" "$query"
fi
