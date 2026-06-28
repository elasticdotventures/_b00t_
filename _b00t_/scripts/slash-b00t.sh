#!/usr/bin/env bash
# slash-b00t.sh — Gather b00t status for Claude Code /b00t slash command
# Invoked via: just slash-b00t
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
B00T_CLI="cargo run --manifest-path "$REPO_ROOT/Cargo.toml" --bin b00t-cli --"

echo
echo "🥾  b00t Status"
echo "═══════════════"
echo

# ── Git ──
echo "📂  Git"
echo "   branch : $(git -C "$REPO_ROOT" branch --show-current)"
echo "   commit : $(git -C "$REPO_ROOT" log -1 --oneline)"
echo "   remote : $(git -C "$REPO_ROOT" remote get-url origin 2>/dev/null || echo 'none')"
echo

# ── Zellij ──
if [ -n "${ZELLIJ:-}" ]; then
    echo "🪟  Zellij: ACTIVE"
    zellij ls 2>/dev/null | sed 's/^/   /' || echo "   (could not list sessions)"
else
    echo "🪟  Zellij: not active"
fi
echo

# ── Governance gates ──
echo "🛡️  Governance Gates"
$B00T_CLI governance status 2>/dev/null || echo "   ⚠️  governance status unavailable (b00t-cli not built with governance feature)"
echo

# ── Cake balance ──
echo "🍰  Cake Balance"
$B00T_CLI cake balance 2>/dev/null || echo "   ⚠️  cake balance unavailable"
echo

# ── Open GitHub issues ──
echo "📋  Open Issues"
if command -v gh &>/dev/null; then
    OPEN_COUNT=$(gh issue list --state open --limit 100 --json number 2>/dev/null | jq 'length' 2>/dev/null || echo "?")
    echo "   open : $OPEN_COUNT"
    gh issue list --state open --limit 5 --json number,title 2>/dev/null | \
        jq -r '.[] | "   #\(.number)  \(.title)"' 2>/dev/null || true
else
    echo "   ⚠️  gh CLI not available"
fi
echo

# ── Exercise reminder ──
echo "🏃  Exercise Reminder"
echo "   15-min interval: take a stretch break, hydrate, step away from the keyboard."
echo
