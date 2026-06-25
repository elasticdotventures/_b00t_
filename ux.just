# ux.just — b00t capability graph visualization module
# Invoke: just ux::viz | just ux::viz-full | just ux::watch | just ux::mermaid
#
# Requires: vendor/l3dg3rr xtask (built automatically via cargo)
# Output:   dist/*.html  (standalone, open in any browser — no server needed)

set shell := ["bash", "-cu"]

_xtask := "cargo run -q -p xtask-mcpb --manifest-path vendor/l3dg3rr/Cargo.toml --"
_dist  := justfile_directory() / "dist"
_open  := if os() == "macos" { "open" } else { "xdg-open" }

# List available ux recipes
[private]
default:
    @just --list --justfile {{justfile()}}

# ── Core visualization ──────────────────────────────────────────────────────────

# Generate + open the b00t capability graph (evidence log → interactive HTML)
[group('viz')]
viz output=(_dist / "b00t-cap.html"):
    @mkdir -p {{_dist}}
    {{_xtask}} viz-b00t-capabilities --output {{output}}
    @echo "→ opening {{output}}"
    @{{_open}} {{output}} 2>/dev/null || echo "open manually: {{output}}"

# Generate + open the full merged graph (b00t capabilities + ledgerr domain types)
[group('viz')]
viz-full output=(_dist / "b00t-full.html"):
    @mkdir -p {{_dist}}
    {{_xtask}} viz-b00t-capabilities --output {{output}} --merge-seed
    @echo "→ opening {{output}}"
    @{{_open}} {{output}} 2>/dev/null || echo "open manually: {{output}}"

# Generate graph with live DatumType metadata from b00t-mcp server
[group('viz')]
viz-live url="http://localhost:3000" output=(_dist / "b00t-live.html"):
    @mkdir -p {{_dist}}
    {{_xtask}} viz-b00t-capabilities \
        --output {{output}} \
        --type-graph-url {{url}}/v1/b00t/type-graph \
        --merge-seed
    @{{_open}} {{output}} 2>/dev/null || echo "open manually: {{output}}"

# ── Alternate formats ───────────────────────────────────────────────────────────

# Export Mermaid graph LR source (paste at https://mermaid.live)
[group('viz')]
mermaid output=(_dist / "b00t-cap.md"):
    @mkdir -p {{_dist}}
    {{_xtask}} viz-b00t-capabilities --format mermaid --output {{output}}
    @echo "paste at: https://mermaid.live"
    @echo "file: {{output}}"

# Export Cytoscape JSON (load in Gephi, Cytoscape desktop, or d3-force)
[group('viz')]
json output=(_dist / "b00t-cap.json"):
    @mkdir -p {{_dist}}
    {{_xtask}} viz-b00t-capabilities --format json --output {{output}}
    @echo "wrote: {{output}}"

# ── Watch mode ──────────────────────────────────────────────────────────────────

# Watch evidence log and regenerate HTML on every write (requires inotifywait)
[group('viz')]
watch output=(_dist / "b00t-cap.html"):
    #!/usr/bin/env bash
    set -euo pipefail
    evidence="${HOME}/.b00t/evidence"
    mkdir -p "{{_dist}}" "$evidence"
    if ! command -v inotifywait &>/dev/null; then
        echo "⚠️  inotifywait not found — install inotify-tools: sudo apt install inotify-tools"
        echo "   falling back to poll mode (5s)"
        while true; do
            cargo run -q -p xtask-mcpb \
                --manifest-path vendor/l3dg3rr/Cargo.toml \
                -- viz-b00t-capabilities --output "{{output}}" 2>/dev/null
            sleep 5
        done
    fi
    echo "watching $evidence → {{output}}"
    echo "open {{output}} in your browser — it will auto-refresh via meta refresh"
    # Inject meta refresh into the first generated file
    cargo run -q -p xtask-mcpb \
        --manifest-path vendor/l3dg3rr/Cargo.toml \
        -- viz-b00t-capabilities --output "{{output}}"
    # Patch in a 3-second meta refresh so the browser auto-reloads
    sed -i 's|</head>|<meta http-equiv="refresh" content="3"></head>|' "{{output}}"
    xdg-open "{{output}}" 2>/dev/null || true
    while inotifywait -qq -e close_write "$evidence" 2>/dev/null; do
        cargo run -q -p xtask-mcpb \
            --manifest-path vendor/l3dg3rr/Cargo.toml \
            -- viz-b00t-capabilities --output "{{output}}" 2>/dev/null
        sed -i 's|</head>|<meta http-equiv="refresh" content="3"></head>|' "{{output}}"
        echo "  ↺ regenerated $(date +%H:%M:%S)"
    done

# ── Serve mode ──────────────────────────────────────────────────────────────────

# Serve the dist/ directory with a tiny HTTP server (requires python3 or miniserve)
[group('viz')]
serve port="8787":
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{_dist}}"
    # Ensure something exists to serve
    if [[ ! -f "{{_dist}}/b00t-cap.html" ]]; then
        just ux::viz
    fi
    echo "serving http://localhost:{{port}} → {{_dist}}"
    echo "press Ctrl+C to stop"
    if command -v miniserve &>/dev/null; then
        miniserve --port {{port}} "{{_dist}}"
    elif command -v python3 &>/dev/null; then
        python3 -m http.server {{port}} --directory "{{_dist}}"
    else
        echo "⚠️  need miniserve or python3 on PATH"
        echo "   cargo install miniserve"
        exit 1
    fi

# ── Quick helpers ───────────────────────────────────────────────────────────────

# Regenerate all formats at once
[group('viz')]
all: viz viz-full mermaid json
    @echo "✓ all formats written to {{_dist}}/"

# Print what's in the evidence log
[group('debug')]
evidence-stats:
    #!/usr/bin/env bash
    evidence="${HOME}/.b00t/evidence"
    echo "=== evidence log stats ==="
    for f in edges.jsonl satisfies.jsonl; do
        path="$evidence/$f"
        if [[ -f "$path" ]]; then
            count=$(wc -l < "$path")
            echo "  $f: $count records"
        else
            echo "  $f: (empty)"
        fi
    done

# Tail the live edge stream
[group('debug')]
edge-tail:
    @tail -f "${HOME}/.b00t/evidence/edges.jsonl" 2>/dev/null || \
        echo "no edges.jsonl yet — run b00t blessing --manifest to populate"

# Dump all unique predicates in the evidence log
[group('debug')]
predicates:
    #!/usr/bin/env bash
    evidence="${HOME}/.b00t/evidence"
    echo "=== predicates in edges.jsonl ==="
    if [[ -f "$evidence/edges.jsonl" ]]; then
        jq -r '.predicate' "$evidence/edges.jsonl" 2>/dev/null | sort -u
    else
        echo "  (none)"
    fi
    echo "=== predicates in satisfies.jsonl ==="
    if [[ -f "$evidence/satisfies.jsonl" ]]; then
        jq -r '.predicate' "$evidence/satisfies.jsonl" 2>/dev/null | sort -u
    else
        echo "  (none)"
    fi
