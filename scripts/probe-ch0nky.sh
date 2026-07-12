#!/usr/bin/env bash
# Probe ch0nky (or any OpenAI-compatible endpoint) with b00t-awareness questions
# Usage: scripts/probe-ch0nky.sh [endpoint] [api-key]
set -euo pipefail

SVC="${1:-$(kubectl get svc ch0nky -n b00t-inference -o jsonpath='{.status.loadBalancer.ingress[0].ip}' 2>/dev/null)}"
# Fall back to NodePort on the node IP (31001 is the fixed nodePort)
[[ -z "$SVC" ]] && SVC="192.168.1.137:31001"
KEY="${2:-local-b00t}"

if [[ -z "$SVC" ]]; then
    echo "Usage: $0 <host:port> [api-key]" >&2
    echo "       or: kubectl get svc ch0nky -n b00t-inference" >&2
    exit 1
fi

URL="http://${SVC}/v1/chat/completions"
echo "🔍 probing ${URL}"
echo

QUESTIONS=(
    "What subcommand does b00t-cli use to read and write the soul KV store? Show exact syntax."
    "How do I add a new task in the b00t task system? Show the exact CLI command."
    "What modes does just install support and how does it remember past choices?"
    "What is the correct syntax for b00t grok ask?"
    "What command do I run to orient my role and load blessings in b00t?"
)
LABELS=("soul kv" "b00t task" "just install" "b00t grok" "b00t whoami")

PASSED=0
for i in "${!QUESTIONS[@]}"; do
    Q="${QUESTIONS[$i]}"
    LABEL="${LABELS[$i]}"
    printf "[%s] %s\n" "$LABEL" "$Q"

    RESPONSE=$(curl -s "$URL" \
      -H "Authorization: Bearer ${KEY}" \
      -H "Content-Type: application/json" \
      -d "$(python3 -c "import json; print(json.dumps({
          'model': 'ch0nky',
          'messages': [
              {'role': 'system', 'content': 'Answer in 1-2 sentences. Be direct and concise.'},
              {'role': 'user',   'content': '${Q}'}
          ],
          'max_tokens': 400,
          'temperature': 0.1
      }))")" 2>/dev/null)

    ANSWER=$(echo "$RESPONSE" | python3 -c "
import json, sys, re
r = json.load(sys.stdin)
c = r['choices'][0]['message']
text = (c.get('content') or c.get('reasoning_content') or '').strip()
text = re.sub(r'<think>.*?</think>', '', text, flags=re.DOTALL).strip()
finish = r['choices'][0]['finish_reason']
tokens = r['usage']['completion_tokens']
print(text[:300] if text else '(no answer — still in CoT)')
print(f'  finish={finish} tokens={tokens}')
" 2>/dev/null || echo "  (parse error)")

    echo "→ $ANSWER"

    # Signal check: answer mentions b00t vocabulary
    if echo "$ANSWER" | grep -qiE "b00t|soul|just|mcp|grok|whoami|task add"; then
        echo "  PASS"
        PASSED=$((PASSED + 1))
    else
        echo "  FAIL"
    fi
    echo
done

echo "Result: ${PASSED}/${#QUESTIONS[@]} probes returned b00t-aware responses"
