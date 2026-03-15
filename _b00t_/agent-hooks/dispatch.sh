#!/usr/bin/env bash
# b00t agent-hook dispatcher — tool-agnostic, context-aware hook router
#
# "agent-hooks" = AI agent lifecycle hooks (distinct from git hooks / CI hooks).
# Registered as a single handler in .claude/settings.json (or equivalent).
# Routes to: _b00t_/agent-hooks/events/<event>.sh
#            _b00t_/agent-hooks/roles/<role>/<event>.sh  (if role active)
# Tool adapter normalizes tool-specific event names → b00t canonical before routing.
#
# Exit protocol:
#   exit 0 + stdout JSON  → allow / provide decisions
#   exit 2                → block (stderr fed to model)
#   exit 1                → non-blocking warn (verbose only)
#
# 🤓 Hook deduplication: identical command strings run once per event scope.
#    Register this script once per event — no need to worry about duplicates.

set -euo pipefail

B00T_DIR="${B00T_DIR:-$HOME/.b00t/_b00t_}"
HOOKS_DIR="$B00T_DIR/agent-hooks"

# --- read stdin (may be empty for some tools) ---
INPUT="$(cat)"
export B00T_HOOK_INPUT="$INPUT"   # 🤓 export BEFORE adapter sources so adapter can read it

# --- detect invoking tool + normalize event name ---
# 🤓 Tools differ on event naming. Detection priority:
#   1. CLAUDE_PROJECT_DIR → claude-code    (events: PascalCase)
#   2. OPENCODE_PROJECT_DIR → opencode     (events: snake_case — adapter needed)
#   3. fallback → unknown, pass through as-is
if   [ -n "${CLAUDE_PROJECT_DIR:-}" ]; then
    B00T_TOOL="claude-code"
    source "$HOOKS_DIR/adapters/claude-code.sh" 2>/dev/null || true
elif [ -n "${OPENCODE_PROJECT_DIR:-}" ]; then
    B00T_TOOL="opencode"
    source "$HOOKS_DIR/adapters/opencode.sh" 2>/dev/null || true
else
    B00T_TOOL="unknown"
fi
export B00T_TOOL

# B00T_EVENT should be set by adapter or fallback to env/stdin field
if [ -z "${B00T_EVENT:-}" ]; then
    # Try: env var injected by tool, then parse stdin JSON
    B00T_EVENT="${HOOK_EVENT_NAME:-$(echo "$INPUT" | jq -r '.hook_event_name // empty' 2>/dev/null || echo "")}"
fi
export B00T_EVENT

# --- determine active role and spawned agent type ---
# 🤓 B00T_ROLE = session role (executive, orchestrator, etc.) — set by SessionStart hook
#    AGENT_TYPE = the type of subagent being spawned/stopped (Explore, security, etc.)
#    These are DISTINCT: B00T_ROLE routes to roles/<B00T_ROLE>/<event>.sh
#    agent_type is available as context WITHIN that handler via B00T_HOOK_INPUT.
export B00T_ROLE="${B00T_ROLE:-}"
export B00T_AGENT_TYPE="$(echo "$INPUT" | jq -r '.agent_type // empty' 2>/dev/null || echo "")"

# --- dispatch: role-specific first, then generic ---
# 🤓 use explicit if+exit pattern — "cmd && exit $?" drops non-zero codes

# Role-specific hook (e.g., agent-hooks/roles/executive/tool.pre.sh)
if [ -n "$B00T_ROLE" ] && [ -n "$B00T_EVENT" ]; then
    ROLE_HOOK="$HOOKS_DIR/roles/$B00T_ROLE/$B00T_EVENT.sh"
    if [ -x "$ROLE_HOOK" ]; then
        echo "$INPUT" | "$ROLE_HOOK"
        exit $?
    fi
fi

# Generic event hook (e.g., agent-hooks/events/tool.pre.sh)
if [ -n "$B00T_EVENT" ]; then
    EVENT_HOOK="$HOOKS_DIR/events/$B00T_EVENT.sh"
    if [ -x "$EVENT_HOOK" ]; then
        echo "$INPUT" | "$EVENT_HOOK"
        exit $?
    fi
fi

# Default: allow
exit 0
