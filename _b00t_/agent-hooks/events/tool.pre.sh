#!/usr/bin/env bash
# b00t guard — tool.pre hook: intercept bash commands before execution.
# Blocks or warns on disallowed patterns per AGENTS.md command guards.
# Exit 0 = allow, 1 = warn (verbose), 2 = block
set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat 2>/dev/null || echo "")}"
TOOL_NAME="$(echo "$INPUT" | jq -r '.tool_name // .tool // empty' 2>/dev/null || echo "")"
COMMAND="$(echo "$INPUT" | jq -r '.command // .tool_input.command // empty' 2>/dev/null || echo "${BASH_COMMAND:-}")"

# Only intercept bash/shell tools
if [[ "$TOOL_NAME" != "bash" && "$TOOL_NAME" != "Bash" && -z "${BASH_COMMAND:-}" ]]; then
    exit 0
fi

# ─── BLOCK patterns (exit 2) ─────────────────────────────────────────────────
# rm -rf / — absolute destruction
if echo "$COMMAND" | grep -qE "rm\s+-rf\s+/"; then
    echo '{"decision":"block","reason":"rm -rf / is BLOCKED by b00t guard","suggestion":"use trash or targeted paths"}' >&2
    exit 2
fi

# ─── WARN/REPLACE patterns (exit 1) ──────────────────────────────────────────
warnings=()

# grep → rg (ripgrep)
if echo "$COMMAND" | grep -qE "(^\||\s)grep\s"; then
    warnings+=("use rg (ripgrep) instead of grep — faster, .gitignore-aware, standardized regex")
fi

# pip install → uv pip install
if echo "$COMMAND" | grep -qE "pip\s+install\b"; then
    warnings+=("use uv pip install instead of pip install")
fi

# docker run → podman
if echo "$COMMAND" | grep -qE "docker\s+run\b"; then
    warnings+=("use podman --device nvidia.com/gpu=all instead of docker run")
fi

# huggingface-cli → hf
if echo "$COMMAND" | grep -qE "huggingface-cli\b"; then
    warnings+=("use hf download instead of huggingface-cli")
fi

if [[ ${#warnings[@]} -gt 0 ]]; then
    printf '%s\n' "${warnings[@]}" >&2
    exit 1
fi

exit 0
