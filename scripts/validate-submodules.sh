#!/usr/bin/env bash
# validate-submodules.sh
#
# Validates that every vendor/* path referenced in workspace Cargo.toml
# has a matching entry in .gitmodules.
#
# Scans:
#   - Cargo.toml members / exclude arrays (vendor/* paths)
#   - workspace.dependencies with path = "vendor/..."
#   - [patch.crates-io] with path = "vendor/..."
#
# Ignores commented-out lines and path = "vendor/..." inside excluded members
# that are intentionally excluded.
#
# Usage: bash scripts/validate-submodules.sh
#   Exit 0 = consistent    (every vendor path has a .gitmodules entry)
#   Exit 1 = issues found  (missing entries, orphans, or both)
#
# No Python dependency — pure bash + sed + grep.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

CARGO_TOML="$REPO_ROOT/Cargo.toml"
GITMODULES="$REPO_ROOT/.gitmodules"

MISSING=()
ORPHANS=()
ERRORS=0

# ── Helper: extract top-level vendor/<name> from a path string ──────────────
#   vendor/foo          → vendor/foo
#   vendor/foo/bar/baz  → vendor/foo
vendor_top() {
    local p="$1"
    # Strip 'vendor/' prefix, take first component, re-add vendor/
    local rest="${p#vendor/}"
    echo "vendor/${rest%%/*}"
}

# ── Step 1: Extract all vendor/* paths from Cargo.toml ──────────────────────

if [ ! -f "$CARGO_TOML" ]; then
    echo "❌ Cargo.toml not found at $CARGO_TOML" >&2
    exit 1
fi

# Remove blank lines and comment lines, then find all "vendor/..." strings.
# We capture from members/exclude arrays, path dependencies, and patch sections.
declare -a RAW_PATHS

# Strategy: strip comments first, then grep for 'vendor/' in remaining text.
# Use sed to preserve quoted strings across lines within arrays.
RAW_PATHS=($(
    < "$CARGO_TOML" \
    sed 's/#.*//' |                                          # strip comments
    grep -oP '"vendor/[^"]+' |                                # extract "vendor/...
    sed 's/^"//' |                                            # strip leading quote
    while IFS= read -r p; do vendor_top "$p"; done |          # normalize to vendor/<name>
    sort -u
))

echo "🔍 vendor paths referenced in Cargo.toml (${#RAW_PATHS[@]}):"
for p in "${RAW_PATHS[@]}"; do
    echo "    $p"
done
echo ""

# ── Step 2: Extract all vendor/* paths from .gitmodules ────────────────────

declare -a GITMODULE_PATHS

if [ ! -f "$GITMODULES" ]; then
    echo "⚠️  .gitmodules not found — treating as empty"
else
    GITMODULE_PATHS=($(
        < "$GITMODULES" \
        grep -oP '\[submodule "vendor/[^"]+' |                # extract [submodule "vendor/...
        sed 's/\[submodule "//'                                # strip prefix
    ))
fi

echo "📦 vendor paths in .gitmodules (${#GITMODULE_PATHS[@]}):"
for p in "${GITMODULE_PATHS[@]}"; do
    echo "    $p"
done
echo ""

# ── Step 3: Build lookup maps ───────────────────────────────────────────────

declare -A GITMOD_MAP
for p in "${GITMODULE_PATHS[@]}"; do
    GITMOD_MAP["$p"]=1
done

declare -A CARGO_MAP
for p in "${RAW_PATHS[@]}"; do
    CARGO_MAP["$p"]=1
done

# ── Step 4: Check for gaps ──────────────────────────────────────────────────

# (a) Vendor paths in Cargo.toml but MISSING from .gitmodules
for p in "${RAW_PATHS[@]}"; do
    if [ -z "${GITMOD_MAP[$p]:-}" ]; then
        MISSING+=("$p")
    fi
done

# (b) .gitmodules entries NOT referenced in Cargo.toml (orphans)
for p in "${GITMODULE_PATHS[@]}"; do
    if [ -z "${CARGO_MAP[$p]:-}" ]; then
        ORPHANS+=("$p")
    fi
done

# ── Step 5: Report ──────────────────────────────────────────────────────────

if [ ${#MISSING[@]} -gt 0 ]; then
    echo "❌ MISSING .gitmodules entries (referenced in Cargo.toml but not in .gitmodules):"
    for p in "${MISSING[@]}"; do
        echo "    • $p"
        echo "      Add to .gitmodules:"
        echo "        [submodule \"$p\"]"
        echo "        \tpath = $p"
        echo "        \turl = <repo-url>"
    done
    echo ""
    ERRORS=$((ERRORS + 1))
fi

if [ ${#ORPHANS[@]} -gt 0 ]; then
    echo "⚠️  ORPHAN .gitmodules entries (in .gitmodules but not referenced in Cargo.toml):"
    for p in "${ORPHANS[@]}"; do
        echo "    • $p"
    done
    echo ""
    ERRORS=$((ERRORS + 1))
fi

if [ $ERRORS -eq 0 ]; then
    echo "✅ All vendor paths are consistent between Cargo.toml and .gitmodules"
    exit 0
else
    echo "🚩 Found $ERRORS issue(s) — fix before adding new vendor submodules"
    exit 1
fi
