#!/usr/bin/env bash
# b00t opencode wrapper — launches opencode with b00t agent-hook intercepts.
# Registers guard intercepts: grep→rg, pip→uv, docker→podman, huggingface-cli→hf
#
# Usage:
#   b00t opencode              # launch TUI with b00t plugins
#   b00t opencode run "msg"    # run headless with b00t plugins
#   b00t opencode serve        # headless server with b00t plugins

set -euo pipefail

B00T_DIR="${B00T_DIR:-$HOME/.b00t}"
HOOKS_DIR="$B00T_DIR/_b00t_/agent-hooks"

if [[ ! -x "$HOOKS_DIR/dispatch.sh" ]]; then
    echo "❌ b00t: agent-hooks not found at $HOOKS_DIR" >&2
    exit 1
fi

# Register b00t hooks with opencode
export OPENCODE_HOOK_PATH="$HOOKS_DIR/dispatch.sh"
export OPENCODE_PROJECT_DIR="${OPENCODE_PROJECT_DIR:-$PWD}"
export B00T_ROLE="${B00T_ROLE:-worker}"

# Launch opencode with b00t intercepts
exec opencode "$@"
