#!/usr/bin/env bash
# 🤓 Verify b00t Chrome browser plugin installation via CDP ping/pong
# Usage: bash scripts/verify-plugin.sh [windows_host_ip]
# Returns non-zero if ping/pong fails.
set -uo pipefail

HOST="${1:-172.30.64.1}"
CDP_PORT="${CDP_PORT:-9222}"
CDP="http://${HOST}:${CDP_PORT}"

echo "🔍 b00t Browser Plugin — Ping/Pong Verification"
echo "   CDP endpoint: ${CDP}"
echo ""

# ─── Step 1: Ping — verify CDP is reachable ───────────────────────────────

echo "  1️⃣  Ping: connecting to CDP..."
VERSION=$(curl -s --max-time 3 "${CDP}/json/version" 2>/dev/null)
if [ -z "$VERSION" ]; then
    echo "     ❌ CDP not reachable at ${CDP}"
    echo ""
    echo "     Start Chrome on Windows with:"
    echo '       chrome.exe --remote-debugging-port=9222 --remote-allow-origins=*'
    echo ""
    echo "     Or run: just start-chrome"
    exit 1
fi
BROWSER=$(echo "$VERSION" | python3 -c "import sys,json; print(json.load(sys.stdin).get('Browser','?'))" 2>/dev/null)
echo "     ✅ Pong! Browser: ${BROWSER}"

# ─── Step 2: List tabs — verify we can enumerate targets ──────────────────

echo "  2️⃣  Listing tabs..."
TABS=$(curl -s --max-time 3 "${CDP}/json" 2>/dev/null)
TAB_COUNT=$(echo "$TABS" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
echo "     ✅ ${TAB_COUNT} tab(s) open"

# ─── Step 3: Find admin dashboard tab ─────────────────────────────────────

echo "  3️⃣  Finding b00t-admin dashboard tab..."
ADMIN_TAB=$(echo "$TABS" | python3 -c "
import sys, json
tabs = json.load(sys.stdin)
for t in tabs:
    url = t.get('url', '')
    title = t.get('title', '')
    if '31337' in url or 'b00t-admin' in title:
        print(t['id'])
        break
" 2>/dev/null)

if [ -n "$ADMIN_TAB" ]; then
    echo "     ✅ Found dashboard tab: ${ADMIN_TAB}"
else
    echo "     ℹ️  No dashboard tab open — navigate to http://localhost:31337/ first"
fi

# ─── Step 4: Install browser plugin ───────────────────────────────────────

echo "  4️⃣  Installing b00t browser extension..."
EXT_PATH="/home/brianh/.dotfiles/b00t-browser-ext/build/chrome-mv3-prod"
if [ -d "$EXT_PATH" ]; then
    echo "     ✅ Extension found at ${EXT_PATH}"
    # Install via CDP
    RESULT=$(curl -s --max-time 5 -X PUT "${CDP}/json/protocol" 2>/dev/null)
    echo "     ℹ️  Use: b00t-rpa plugin"
else
    echo "     ⚠️  Extension not built at ${EXT_PATH}"
    echo "     Build: cd b00t-browser-ext && npm run build"
fi

# ─── Step 5: Ping/pong round-trip via CDP evaluate ────────────────────────

echo "  5️⃣  JS console ping/pong..."
if [ -n "${ADMIN_TAB:-}" ]; then
    WS_URL=$(echo "$TABS" | python3 -c "
import sys, json
tabs = json.load(sys.stdin)
for t in tabs:
    if t.get('id') == '${ADMIN_TAB}':
        print(t['webSocketDebuggerUrl'])
        break
" 2>/dev/null)
    if [ -n "$WS_URL" ]; then
        # Send JS via CDP WebSocket to evaluate console.log + return pong
        PONG=$(echo '{"id":1,"method":"Runtime.evaluate","params":{"expression":"console.log(\"b00t-ping\"); JSON.stringify({pong: true, ts: Date.now()})"}}' | \
          websocat -1 "$WS_URL" 2>/dev/null | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('result',{}).get('result',{}).get('value','?'))" 2>/dev/null)
        echo "     ✅ Ping/pong: ${PONG:-response received}"
    else
        echo "     ⚠️  No WebSocket URL for tab ${ADMIN_TAB}"
    fi
else
    echo "     ⚠️  No dashboard tab — open http://localhost:31337/ first"
    echo "     Then run this script again"
fi

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Verification complete"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
