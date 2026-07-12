#!/usr/bin/env bash
# b00t grok-fallback — keyword RAG over learn/ + datums/ when raglite+irontology are down.
# No external dependencies. Searches filenames, content, and tail-map cmds.
# Usage: grok-fallback.sh [--topic <topic>] <query>
set -euo pipefail

B00T_ROOT="${B00T_ROOT:-$HOME/.b00t}"
LEARN_DIR="$B00T_ROOT/_b00t_/learn"
DATUMS_DIR="$B00T_ROOT/_b00t_/datums"
MAX_RESULTS="${MAX_RESULTS:-10}"

topic=""
query=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --topic|-t) topic="$2"; shift 2 ;;
        *) query="$1"; shift ;;
    esac
done

if [[ -z "$query" ]]; then
    echo "usage: grok-fallback.sh [--topic <topic>] <query>"
    exit 1
fi

# Build regex from query words (case-insensitive, OR)
words=$(echo "$query" | tr ' ' '\n' | grep -v '^$' | sort -u)
pattern=$(echo "$words" | paste -sd '|' -)
if [[ -z "$pattern" ]]; then
    echo "no searchable terms"
    exit 0
fi

results=()

# 1. Search learn/ by filename first (best signal)
while IFS= read -r file; do
    fname=$(basename "$file" .md)
    score=0
    for w in $words; do
        if echo "$fname" | grep -qi "$w"; then
            score=$((score + 10))
        fi
    done
    if [[ $score -gt 0 ]]; then
        results+=("$score|learn:$fname|$(head -3 "$file" | grep -v '^---' | grep -v '^$' | head -1)")
    fi
done < <(find "$LEARN_DIR" -name "*.md" 2>/dev/null || true)

# 2. Search datums/ by name + hint + cmds tail-map
while IFS= read -r file; do
    fname=$(basename "$file" .tomllmd)
    fname=${fname%.toml}
    score=0
    content=""
    # Check filename
    for w in $words; do
        if echo "$fname" | grep -qi "$w"; then
            score=$((score + 10))
        fi
    done
    # Check hint/summary lines
    hint=$(grep -im1 "^#\s*summary:" "$file" 2>/dev/null | sed 's/^#\s*summary:\s*//' || true)
    if [[ -n "$hint" ]]; then
        for w in $words; do
            if echo "$hint" | grep -qi "$w"; then
                score=$((score + 5))
            fi
        done
    fi
    # Check cmds tail-map
    cmds=$(grep -im1 "^#\s*cmds:" "$file" 2>/dev/null | sed 's/^#\s*cmds:\s*//' || true)
    if [[ -n "$cmds" ]]; then
        for w in $words; do
            if echo "$cmds" | grep -qi "$w"; then
                score=$((score + 8))
            fi
        done
    fi
    if [[ $score -gt 0 ]]; then
        preview="${hint:-$cmds}"
        [[ -z "$preview" ]] && preview="(datum: $fname)"
        results+=("$score|datum:$fname|${preview:0:120}")
    fi
done < <(find "$DATUMS_DIR" -name "*.tomllm*" -o -name "*.toml" 2>/dev/null | grep -v '/\.' || true)

# 3. Search learn/ content (heavier, only if topic matches or fewer results than max)
if [[ ${#results[@]} -lt $MAX_RESULTS ]]; then
    for w in $words; do
        while IFS= read -r match; do
            file=$(echo "$match" | cut -d: -f1)
            fname=$(basename "$file" .md)
            snippet=$(echo "$match" | cut -d: -f3- | head -c 120)
            results+=("2|learn:$fname|$snippet")
        done < <(grep -ril "$w" "$LEARN_DIR" 2>/dev/null | head -5 | while read f; do
            echo "$f:$(grep -im1 "$w" "$f" 2>/dev/null | head -1)"
        done || true)
    done
fi

# Sort by score descending, deduplicate, output
printf '%s\n' "${results[@]}" | sort -t'|' -k1 -nr | head -"$MAX_RESULTS" | while IFS='|' read -r score source preview; do
    printf "%-6s %-30s %s\n" "[$score]" "$source" "${preview:0:100}"
done

echo ""
echo "$(printf '%s\n' "${results[@]}" | sort -t'|' -k1 -nr | head -$MAX_RESULTS | wc -l) result(s) [fallback: keyword RAG]"
