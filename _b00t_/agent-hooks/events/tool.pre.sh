#!/usr/bin/env bash
# b00t agent-hook: tool.pre  (PreToolUse)
#
# Gates tool execution before it runs. exit 2 = block (stderr shown to model).
# Handles: Bash security gates, Write/Edit size gates.
#
# 🤓 This is the critical safety layer. Rules:
#    - Block HARD: destructive system commands, credential harvesting, mass deletes
#    - Warn SOFT:  prefer uv over pip, podman over docker (guard redirects)
#    - Inject context: additionalContext field enriches model understanding of why

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
TOOL="$(    echo "$INPUT" | jq -r '.tool_name        // ""' 2>/dev/null)"
COMMAND="$( echo "$INPUT" | jq -r '.tool_input.command // ""' 2>/dev/null)"
FILE_PATH="$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null)"
CONTENT="$(  echo "$INPUT" | jq -r '.tool_input.content   // ""' 2>/dev/null)"

# ─── Bash gates ──────────────────────────────────────────────────────────────
if [ "$TOOL" = "Bash" ]; then

    # HARD BLOCK: destructive filesystem commands
    if echo "$COMMAND" | grep -qE 'rm\s+-rf\s+(/|~|/home|/etc|/usr|/var)\b'; then
        echo "🚫 b00t: destructive rm -rf blocked — specify exact path" >&2
        exit 2
    fi

    # HARD BLOCK: credential harvesting patterns
    if echo "$COMMAND" | grep -qE 'cat\s+~/.ssh/id|cat\s+~/.aws/credentials|cat\s+~/.env\b|env\s*\|\s*grep\s+-i\s*(key|token|secret|pass)'; then
        echo "🚫 b00t: credential harvesting pattern blocked" >&2
        echo "   If legitimately needed, read specific file via Read tool" >&2
        exit 2
    fi

    # HARD BLOCK: silent exfiltration
    if echo "$COMMAND" | grep -qE 'curl\s+.*\|\s*(bash|sh)\b|wget\s+.*\|\s*(bash|sh)\b'; then
        if echo "$COMMAND" | grep -qE 'curl\s+.*\$\(env\)|curl\s+.*\$\(cat\s+~/'; then
            echo "🚫 b00t: potential credential exfiltration blocked" >&2
            exit 2
        fi
    fi

    # SOFT WARN: pip install → prefer uv
    if echo "$COMMAND" | grep -qE '^\s*pip\s+install\b'; then
        jq -n '{
            hookSpecificOutput: {
                hookEventName: "PreToolUse",
                permissionDecision: "ask",
                permissionDecisionReason: "b00t guard: pip install detected — prefer `uv pip install` for venv isolation. Proceed anyway?",
                additionalContext: "b00t preference: uv pip install <pkg> (faster, isolated, respects .venv)"
            }
        }'
        exit 0
    fi

    # SOFT WARN: docker run → prefer podman
    if echo "$COMMAND" | grep -qE '^\s*docker\s+run\b'; then
        jq -n '{
            hookSpecificOutput: {
                hookEventName: "PreToolUse",
                additionalContext: "b00t guard: prefer podman over docker. RTX3090 setup: podman --device nvidia.com/gpu=all --security-opt=label=disable"
            }
        }'
        exit 0
    fi

    # CONTEXT: inject hive resource warning before vllm/huggingface downloads
    if echo "$COMMAND" | grep -qE '(hf\s+download|huggingface-cli\s+download|vllm\s+serve)'; then
        HIVE_STATUS="$(b00t-cli hive status 2>/dev/null | head -5 || echo "hive status unavailable")"
        jq -n --arg status "$HIVE_STATUS" '{
            hookSpecificOutput: {
                hookEventName: "PreToolUse",
                additionalContext: ("b00t hive: check RAM/GPU before large downloads.\n" + $status)
            }
        }'
        exit 0
    fi
fi

# ─── Write/Edit gates ─────────────────────────────────────────────────────────
if [ "$TOOL" = "Write" ] && [ -n "$CONTENT" ]; then
    LINE_COUNT="$(echo "$CONTENT" | wc -l)"
    if [ "$LINE_COUNT" -gt 800 ]; then
        jq -n --argjson lines "$LINE_COUNT" '{
            hookSpecificOutput: {
                hookEventName: "PreToolUse",
                permissionDecision: "ask",
                permissionDecisionReason: ("b00t: file has " + ($lines | tostring) + " lines — b00t gospel max is 800. Split into modules?"),
                additionalContext: "b00t alignment: prefer many small files over few large files (200-400 lines ideal, 800 hard cap)"
            }
        }'
        exit 0
    fi
fi

# Default: allow
exit 0
