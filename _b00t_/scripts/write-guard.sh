#!/usr/bin/env bash
# write-guard.sh — Write guard hook for _b00t_/ sandbox gate
# Intercepts writes to _b00t_/*.toml. Redirects unauthorized writes to staging.
#
# Usage: bash _b00t_/scripts/write-guard.sh <target_file>
# Exit: 0 = allowed (blessed), 1 = redirected to staging (not blessed)
#
# Blessing check: reads KVCache key "zellij.gate.blessings" for "datum-authoring"
set -euo pipefail

# ── Parse args ──────────────────────────────────────────────────────────────
TARGET_FILE="${1:-}"
if [[ -z "$TARGET_FILE" ]]; then
    echo "write-guard: ERROR - no target file specified" >&2
    echo "Usage: bash _b00t_/scripts/write-guard.sh <target_file>" >&2
    exit 2
fi

# ── Resolve paths ───────────────────────────────────────────────────────────
REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || echo "$(cd "$(dirname "$0")/../.." && pwd)")"
cd "$REPO_ROOT"

TARGET_BASENAME="$(basename "$TARGET_FILE")"
STAGING_DIR=".tmp/proposed-datums"
KV_FILE="${B00T_KV_FILE:-$HOME/.b00t/kv-store.json}"

# ── Check blessings via KVCache ─────────────────────────────────────────────
BLESSED=false
if command -v python3 >/dev/null 2>&1 && [[ -f "$KV_FILE" ]]; then
    BLESSINGS_JSON="$(python3 -c "
import json, sys
try:
    with open('$KV_FILE') as f:
        data = json.load(f)
    blessings = data.get('zellij.gate.blessings', '')
    print(blessings)
except:
    sys.exit(1)
" 2>/dev/null || echo "")"

    if [[ "$BLESSINGS_JSON" == *"datum-authoring"* ]]; then
        BLESSED=true
    fi
fi

# ── Allow if blessed ────────────────────────────────────────────────────────
if [[ "$BLESSED" == "true" ]]; then
    echo "write-guard: ✅ datum-authoring blessing confirmed — write allowed to $TARGET_FILE"
    exit 0
fi

# ── Redirect to staging ─────────────────────────────────────────────────────
mkdir -p "$STAGING_DIR"

STAGING_PATH="$STAGING_DIR/$TARGET_BASENAME"
cp "$TARGET_FILE" "$STAGING_PATH" 2>/dev/null || {
    echo "write-guard: WARNING - could not copy '$TARGET_FILE' to staging (file may not exist yet)" >&2
    # Create a placeholder to indicate the attempted write
    echo "# Proposed datum: $TARGET_BASENAME (staged by write-guard)" > "$STAGING_PATH"
    echo "# Original target: $TARGET_FILE" >> "$STAGING_PATH"
    echo "# Status: pending review" >> "$STAGING_PATH"
}

# ── Create review task ──────────────────────────────────────────────────────
if command -v b00t >/dev/null 2>&1; then
    b00t task add "review: proposed datum $TARGET_BASENAME" 2>/dev/null || true
fi

# ── Report to caller ────────────────────────────────────────────────────────
echo ""
echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  🛡️  WRITE GUARD: datum-authoring blessing required         ║"
echo "╠══════════════════════════════════════════════════════════════╣"
echo "║  Write to _b00t_/ blocked: $TARGET_BASENAME"
echo "║  Redirected to staging: $STAGING_PATH"
echo "║                                                              ║"
echo "║  ▸ Request blessing from operator                            ║"
echo "║  ▸ Or use: just propose-datum $TARGET_FILE                 ║"
echo "║  ▸ Review pending: just review-proposed                      ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

exit 1
