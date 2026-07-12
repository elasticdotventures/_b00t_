#!/bin/bash
set -euo pipefail

echo "📡 Updating opencode-ai..."

if command -v pnpm &>/dev/null; then
  PM="pnpm"
elif command -v npm &>/dev/null; then
  PM="npm"
else
  echo "❌ pnpm or npm required"
  exit 1
fi

$PM update -g opencode-ai

echo "✅ opencode updated"
echo ""
opencode --version
