#!/bin/bash
# 🤓 b00t-session-bootstrap — dynamic per-session wiring for rust-doc + b00t-server
# Source this in your session:  source scripts/bootstrap-session.sh
# Creates a fresh API key, starts the proxy, sets env vars.

set -euo pipefail
B00T_ROOT="${B00T_ROOT:-$HOME/.b00t}"
B00T_CLI="${B00T_ROOT}/target/debug/b00t-cli"
B00T_MCP="${B00T_ROOT}/target/debug/b00t-mcp"

echo "🥾 b00t session bootstrap"

# 1. Ensure proxy is running (idempotent)
if ! pgrep -f "b00t-mcp --http --llm" >/dev/null 2>&1; then
    echo "   Starting b00t-server proxy on :5273..."
    "$B00T_MCP" --http --llm --port 5273 &>/tmp/b00t-server.log &
    sleep 2
fi

# 2. Create fresh session key for rust-doc
echo "   Creating session key for rust-doc..."
export OPENAI_BASE_URL="http://localhost:5273/v1"
export OPENAI_API_KEY=$("$B00T_CLI" server key create --consumer rust-doc 2>/dev/null | tail -1)

echo "   OPENAI_BASE_URL=$OPENAI_BASE_URL"
echo "   OPENAI_API_KEY=${OPENAI_API_KEY:0:12}..."

# 3. Verify connectivity
if curl -s --max-time 2 http://localhost:5273/v1/models >/dev/null 2>&1; then
    echo "✅ b00t-server proxy: healthy"
else
    echo "⚠️  b00t-server proxy: not responding"
fi

echo "🍰 Session ready — rust-doc will route through b00t-server"
