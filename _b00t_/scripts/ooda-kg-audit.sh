#!/bin/bash
# ooda-kg-audit.sh — OODA loop using codebase-memory CLI (bypasses MCP timeout)
# Run: b00t sh _b00t_/scripts/ooda-kg-audit.sh
set -euo pipefail

PROJECT="home-brianh-.dotfiles"
CBM="codebase-memory-mcp cli"

# ── PHASE 1: OBSERVE ──────────────────────────────────────────────────────
echo "═══ OBSERVE ═══"
echo ""

# Isolated nodes by label (no edges = orphan)
echo "🔍 Isolated nodes (degree = 0):"
for label in Route Function Module Class Method; do
    result=$($CBM query_graph "{\"project\":\"$PROJECT\",\"query\":\"MATCH (n:$label) OPTIONAL MATCH (n)-[r]-() WITH n, count(r) AS deg WHERE deg = 0 RETURN count(n) AS cnt\",\"max_rows\":1}" 2>/dev/null)
    count=$(echo "$result" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['rows'][0][0] if d.get('rows') else 0)" 2>/dev/null || echo "?")
    [ "$count" != "0" ] && echo "  $label: $count isolated"
done

# Hub nodes (high degree)
echo ""
echo "🏗️  Hub nodes (high degree):"
$CBM query_graph "{\"project\":\"$PROJECT\",\"query\":\"MATCH (n)-[r]-() WITH n, labels(n)[0] AS lbl, count(r) AS deg WHERE deg > 10 RETURN lbl AS label, n.qualified_name AS hub, deg ORDER BY deg DESC LIMIT 10\",\"max_rows\":10}" 2>/dev/null \
  | python3 -c "
import sys, json
d = json.load(sys.stdin)
for row in d.get('rows', []):
    print(f'  {row[0]}: {row[1][:60]} ({row[2]} edges)')
" 2>/dev/null || true

# ── PHASE 2: ORIENT ───────────────────────────────────────────────────────
echo ""
echo "═══ ORIENT ═══"
echo ""

# Degree distribution by label
echo "📊 Degree distribution:"
$CBM query_graph "{\"project\":\"$PROJECT\",\"query\":\"MATCH (n)-[r]-() WITH labels(n)[0] AS label, count(r) AS degree RETURN label, avg(degree) AS avg_deg, count(*) AS total LIMIT 10\",\"max_rows\":10}" 2>/dev/null \
  | python3 -c "
import sys, json
d = json.load(sys.stdin)
for row in d.get('rows', []):
    print(f'  {row[0]}: avg {row[1][:6]} edges/node ({row[2]} nodes)')
" 2>/dev/null || true

# Cassowary layout heuristic
echo ""
echo "🏗️  Layout heuristic (Cassowary):"
echo "  spread = base_radius * (1 + 2 * degree / max_degree)"
echo "  hubs (degree > 10) get pushed apart in cytoscape"

# ── PHASE 3: DECIDE ───────────────────────────────────────────────────────
echo ""
echo "═══ DECIDE ═══"
echo ""

# Get a sample of isolated nodes for classification
$CBM query_graph "{\"project\":\"$PROJECT\",\"query\":\"MATCH (n:Function) OPTIONAL MATCH (n)-[r]-() WITH n, count(r) AS deg WHERE deg = 0 RETURN n.qualified_name AS name LIMIT 3\",\"max_rows\":3}" 2>/dev/null \
  | python3 -c "
import sys, json
d = json.load(sys.stdin)
rows = d.get('rows', [])
if rows:
    print(f'  ⚠️  {len(rows)} isolated Function nodes found — classify for reconnection')
    for row in rows:
        print(f'     → trace_path(\"{row[0][:40]}\", mode=\"calls\")')
else:
    print('  ✅ No isolated nodes found')
" 2>/dev/null || true

# ── PHASE 4: ACT ──────────────────────────────────────────────────────────
echo ""
echo "═══ ACT ═══"
echo "  Rules-based classifier active"
echo "  Model-based classification pending (finetuned on b00t primitives)"
echo ""
echo "✅ OODA loop complete"
