#!/bin/bash
# ooda-kg-stats.sh — snapshot knowledge graph statistics for OODA loop
# Run: b00t sh _b00t_/scripts/ooda-kg-stats.sh
# Output: _b00t_/ooda/kg-stats.json

set -euo pipefail
ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$HOME/.dotfiles")"
PROJECT="$(echo "$ROOT" | tr '/' '-')"
OUT="${ROOT}/_b00t_/ooda/kg-stats.json"
mkdir -p "$(dirname "$OUT")"

codebase-memory-mcp query_graph --project "$PROJECT" --max-rows 50 --json \
  "MATCH (n) OPTIONAL MATCH (n)-[r]-() WITH n, count(r) AS degree WHERE degree = 0 RETURN labels(n) AS labels, n.qualified_name AS node LIMIT 50" \
  > /tmp/isolated.json 2>/dev/null

codebase-memory-mcp query_graph --project "$PROJECT" --max-rows 20 --json \
  "MATCH (n)-[r]-() WITH n, count(r) AS degree WHERE degree > 10 RETURN n.qualified_name AS hub, labels(n) AS labels, degree ORDER BY degree DESC LIMIT 20" \
  > /tmp/hubs.json 2>/dev/null

python3 -c "
import json, os
result = {'project': '$PROJECT', 'timestamp': '$(date -Iseconds)'}

for fn, key in [('/tmp/isolated.json', 'isolated'), ('/tmp/hubs.json', 'hubs')]:
    try:
        with open(fn) as f:
            result[key] = json.load(f)
    except: result[key] = []

with open('$OUT', 'w') as f:
    json.dump(result, f, indent=2)
print(f'Saved to $OUT')
"
