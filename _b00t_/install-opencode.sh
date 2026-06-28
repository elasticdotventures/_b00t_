#!/bin/bash
set -euo pipefail

BIN_DIR="${HOME}/.local/bin"
mkdir -p "$BIN_DIR"

echo "📦 Installing opencode-ai from npm registry..."

if command -v pnpm &>/dev/null; then
  PM="pnpm"
elif command -v npm &>/dev/null; then
  PM="npm"
else
  echo "❌ pnpm or npm required"
  exit 1
fi

$PM add -g opencode-ai

echo "🔧 Running postinstall to download native binary (fixes issue #27906)..."

# Find opencode-ai installation across possible package manager paths
for PKG_PATH in \
  "$HOME/.bun/install/global/node_modules/opencode-ai" \
  "$HOME/.npm-global/lib/node_modules/opencode-ai" \
  "$HOME/.local/share/pnpm/global/5/node_modules/opencode-ai" \
  "$(pnpm root -g 2>/dev/null)/opencode-ai" \
  "$(npm root -g 2>/dev/null)/opencode-ai"
do
  if [ -d "$PKG_PATH" ] && [ -f "$PKG_PATH/postinstall.mjs" ]; then
    echo "  Found at: $PKG_PATH"
    cd "$PKG_PATH" && node postinstall.mjs && break
  fi
done

echo "🔗 Symlinking to $BIN_DIR/opencode (works in/out containers)..."
ln -sf "$(command -v opencode)" "$BIN_DIR/opencode"
chmod +x "$BIN_DIR/opencode"

echo "✅ opencode installed"
echo ""
echo "Verify installation:"
opencode --version
