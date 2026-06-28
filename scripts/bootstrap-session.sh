#!/bin/bash
# 🤓 b00t-session-bootstrap — zero-trust session wiring
# Source: source scripts/bootstrap-session.sh
# Keys NEVER touch shell env. OS keyring stores upstream secrets.
# Consumers get opaque b00t-sk-* tokens. b00t-server injects real keys at proxy time.

set -euo pipefail
B00T_ROOT="${B00T_ROOT:-$HOME/.b00t}"
B00T_CLI="${B00T_ROOT}/target/debug/b00t-cli"
B00T_MCP="${B00T_ROOT}/target/debug/b00t-mcp"

echo "🥾 b00t session bootstrap (zero-trust)"

# ── 1. Ensure b00t-server proxy running ──
if ! pgrep -f "b00t-mcp --http --llm" >/dev/null 2>&1; then
    echo "   Starting b00t-server proxy on :5273..."
    "$B00T_MCP" --http --llm --port 5273 &>/tmp/b00t-server.log &
    sleep 2
fi

# ── 2. Verify upstream key is set in OS keyring ──
#    One-time setup (operator only):
#      b00t server key set --provider openai < /path/to/key.txt
#    Or via interactive prompt:
#      b00t server key set --provider openai --interactive
if ! "$B00T_CLI" server key check --provider openai 2>/dev/null; then
    echo "   ⚠️  No upstream key in keyring."
    echo "   Run: b00t server key set --provider openai --interactive"
    echo "   Then re-source this script."
else
    echo "   ✅ Upstream key: keyring (not in env)"
fi

# ── 3. Create scoped token for rust-doc ──
#    Opaque token — reveals nothing. Scoped to embeddings + models only.
echo "   Creating scoped token for rust-doc..."
export OPENAI_BASE_URL="http://localhost:5273/v1"
export OPENAI_API_KEY=$("$B00T_CLI" server key create \
    --consumer rust-doc \
    --scopes embeddings,models 2>/dev/null | tail -1)

echo "   OPENAI_BASE_URL=$OPENAI_BASE_URL"
echo "   OPENAI_API_KEY=${OPENAI_API_KEY:0:12}... (scoped: embeddings,models)"
echo "   (Raw upstream key NEVER exposed — injected by proxy from keyring)"

# ── 4. Verify connectivity ──
if curl -s --max-time 2 http://localhost:5273/v1/models >/dev/null 2>&1; then
    echo "   ✅ b00t-server proxy: healthy"
else
    echo "   ⚠️  b00t-server proxy: not responding (check upstream key in keyring)"
fi

echo "🍰 Zero-trust session ready — no raw keys in env"
