#!/usr/bin/env bash
# b00t agent-hooks installer for claude-code
#
# Non-destructively merges b00t agent-hooks into claude-code settings.
# "Non-destructive" = existing hooks are PRESERVED; b00t hooks are APPENDED.
# Idempotent: running twice produces the same result (no duplicates).
#
# Target priority (first writable wins):
#   1. .claude/settings.local.json  (project-local, gitignored)
#   2. ~/.claude/settings.json      (user-global)
#
# Usage:
#   ~/.b00t/_b00t_/agent-hooks/install-claude-hooks.sh [--global] [--dry-run]
#
# 🤓 agent-hooks are AI agent lifecycle hooks (SessionStart, PreToolUse, etc.)
#    NOT git hooks (pre-commit) or CI hooks. Named explicitly to avoid confusion.
#
# 🤓 jq merge strategy: for each event key, APPEND b00t matchers to existing array.
#    Hook deduplication in claude-code means identical command strings run once.

set -euo pipefail

AGENT_HOOKS_DIR="${B00T_DIR:-$HOME/.b00t/_b00t_}/agent-hooks"
DISPATCH="$AGENT_HOOKS_DIR/dispatch.sh"
DRY_RUN=false
FORCE_GLOBAL=false

for arg in "$@"; do
    case "$arg" in
        --dry-run) DRY_RUN=true ;;
        --global)  FORCE_GLOBAL=true ;;
    esac
done

# --- determine target settings file ---
LOCAL_SETTINGS=".claude/settings.local.json"
GLOBAL_SETTINGS="$HOME/.claude/settings.json"

if $FORCE_GLOBAL; then
    TARGET="$GLOBAL_SETTINGS"
elif [ -d ".claude" ] && ! $FORCE_GLOBAL; then
    TARGET="$LOCAL_SETTINGS"
    echo "ℹ️  Project .claude/ found — installing to $TARGET (use --global for ~/.claude/settings.json)"
else
    TARGET="$GLOBAL_SETTINGS"
fi

mkdir -p "$(dirname "$TARGET")"

# --- b00t hooks payload (canonical, versioned) ---
# 🤓 dispatch.sh handles ALL events via B00T_EVENT normalization.
#    Matchers kept broad (.*) — dispatch.sh does fine-grained routing internally.
#    Timeout values: PreToolUse 30s (blocking), SubagentStop 60s (agent gate).
read -r -d '' B00T_HOOKS_JSON <<'HOOKS_JSON' || true
{
  "SessionStart": [
    {
      "matcher": "startup|resume",
      "hooks": [
        {
          "type": "command",
          "command": "~/.b00t/_b00t_/agent-hooks/dispatch.sh",
          "timeout": 10
        }
      ]
    }
  ],
  "PreToolUse": [
    {
      "matcher": "Bash|Edit|Write",
      "hooks": [
        {
          "type": "command",
          "command": "~/.b00t/_b00t_/agent-hooks/dispatch.sh",
          "timeout": 30,
          "statusMessage": "b00t gate check..."
        }
      ]
    }
  ],
  "PostToolUse": [
    {
      "matcher": "Bash|Edit|Write",
      "hooks": [
        {
          "type": "command",
          "command": "~/.b00t/_b00t_/agent-hooks/dispatch.sh",
          "timeout": 60,
          "statusMessage": "b00t post-tool validation..."
        }
      ]
    }
  ],
  "UserPromptSubmit": [
    {
      "hooks": [
        {
          "type": "command",
          "command": "~/.b00t/_b00t_/agent-hooks/dispatch.sh",
          "timeout": 10
        }
      ]
    }
  ],
  "SubagentStart": [
    {
      "matcher": ".*",
      "hooks": [
        {
          "type": "command",
          "command": "~/.b00t/_b00t_/agent-hooks/dispatch.sh",
          "timeout": 10
        }
      ]
    }
  ],
  "SubagentStop": [
    {
      "matcher": ".*",
      "hooks": [
        {
          "type": "command",
          "command": "~/.b00t/_b00t_/agent-hooks/dispatch.sh",
          "timeout": 60,
          "statusMessage": "b00t agent validation..."
        }
      ]
    }
  ]
}
HOOKS_JSON

# --- load existing settings or start empty ---
if [ -f "$TARGET" ]; then
    EXISTING="$(cat "$TARGET")"
else
    EXISTING='{}'
fi

# Validate existing JSON
if ! echo "$EXISTING" | jq empty 2>/dev/null; then
    echo "⚠️  $TARGET contains invalid JSON — creating backup at ${TARGET}.bak" >&2
    cp "$TARGET" "${TARGET}.bak"
    EXISTING='{}'
fi

# --- merge: append b00t hooks to each event's array, dedup by command string ---
# jq strategy:
#   For each event in b00t payload:
#     existing_event_array + b00t_event_matchers
#     then deduplicate by the nested hooks[].command value
MERGED="$(echo "$EXISTING" | jq \
    --argjson b00t "$B00T_HOOKS_JSON" \
    '
    . as $existing |
    ($existing.hooks // {}) as $eh |
    reduce ($b00t | keys[]) as $event (
        $eh;
        .[$event] = (
            ((.[$event] // []) + $b00t[$event])
            | group_by(.hooks[0].command // "")
            | map(.[0])
        )
    ) |
    . as $merged_hooks |
    $existing | .hooks = $merged_hooks
    '
)"

if [ -z "$MERGED" ] || ! echo "$MERGED" | jq empty 2>/dev/null; then
    echo "❌ jq merge failed — aborting without changes" >&2
    exit 1
fi

if $DRY_RUN; then
    echo "=== DRY RUN: would write to $TARGET ==="
    echo "$MERGED" | jq .
    echo "=== END DRY RUN ==="
    exit 0
fi

echo "$MERGED" | jq . > "$TARGET"
echo "✅ b00t agent-hooks installed → $TARGET"
echo "   Events registered: $(echo "$MERGED" | jq -r '.hooks | keys | join(", ")')"
echo ""
echo "   Verify with: claude /hooks"
