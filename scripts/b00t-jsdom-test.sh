#!/bin/bash
# b00t-jsdom-test.sh — Execute served inline JS in a DOM context, check for runtime errors
set -euo pipefail
URL="${1:-http://localhost:31337/}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
B00T_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$B00T_ROOT"
if ! node -e "require('jsdom')" 2>/dev/null; then
    npm install jsdom --no-save 2>/dev/null || true
fi

curl -s "$URL" | NODE_PATH="$B00T_ROOT/node_modules" node "$SCRIPT_DIR/b00t-jsdom-test.mjs" 2>&1
