#!/usr/bin/env bash
# b00t-whatis <topic> — the idiomatic "what is this thing in the b00t ecosystem?"
#
# There is no `b00t whatis` subcommand; this is the alias the operator asked for.
# It infers what <topic> is (datum / agent / mcp / cli / skill / hive profile /
# capability) and prints it plus its ecosystem cross-references — a local, offline
# "deepwiki page" for a b00t concept. For external GitHub repos use the deepwiki
# MCP (_b00t_/deepwiki.mcp.toml) instead.
#
# Usage: b00t-whatis <topic> [--json]
set -u
T="${1:-}"
[ -z "$T" ] && { echo "usage: b00t-whatis <topic> [--json]" >&2; exit 2; }
BC="${B00T_CLI:-b00t-cli}"

section() { printf '\n\033[1m── %s ──\033[0m\n' "$1"; }

section "datum"
"$BC" datum show "$T" 2>/dev/null || echo "  (no datum named '$T')"

section "type + roles + validate (ontology triples)"
"$BC" ontology sparql --subject "$T" --predicate all 2>/dev/null || echo "  (no ontology triples)"

section "graph neighbours (ecosystem references)"
"$BC" datum neighbors "$T" 2>/dev/null || "$BC" datum search "$T" 2>/dev/null | head -20 || echo "  (none)"

section "capability matches"
"$BC" capabilities 2>/dev/null | grep -i -- "$T" | head -15 || echo "  (none)"

section "learn (DWIW fanout: DatumSearchSource + GraphAdjacencySource)"
"$BC" learn "$T" 2>/dev/null | sed -n '1,40p' || echo "  (nothing to learn)"

section "next"
echo "  external repo?  ->  deepwiki MCP: ask_question(repoName='owner/$T', question='what is $T?')"
echo "  reviewer gate   ->  keep grok ask / sm0l RELEVANT|SKIP before acting (see _b00t_/learn/deepwiki.md)"
