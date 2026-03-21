#!/usr/bin/env bash
# b00t hook: session.start
#
# Fires on SessionStart. Injects b00t session context into the tool's env.
# 🤓 CLAUDE_ENV_FILE is the only way to persist env across the session from a hook.
#    Write `export KEY=VALUE` lines to it — claude-code sources it before continuing.
#
# Non-blocking: exit 2 has no effect here (session already started).

set -euo pipefail

B00T_DIR="${B00T_DIR:-$HOME/.b00t/_b00t_}"

# Determine role from b00t session (if session daemon running)
ROLE=""
if command -v b00t-cli >/dev/null 2>&1; then
    ROLE="$(b00t-cli session status --field=role 2>/dev/null || echo "")"
fi

# Inject into tool env file (claude-code specific — B00T_ENV_FILE set by adapter)
if [ -n "${B00T_ENV_FILE:-}" ]; then
    [ -n "$ROLE" ] && echo "export B00T_ROLE=$ROLE" >> "$B00T_ENV_FILE"
    echo "export B00T_DIR=$B00T_DIR"               >> "$B00T_ENV_FILE"
    echo "export B00T_TOOL=${B00T_TOOL:-unknown}"  >> "$B00T_ENV_FILE"
fi

exit 0
