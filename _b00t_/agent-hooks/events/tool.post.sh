#!/usr/bin/env bash
# b00t agent-hook: tool.post  (PostToolUse)
#
# Runs AFTER tool succeeds. Cannot block the tool that just ran.
# Use for: auto-validation, test execution, audit logging, lint-on-save.
#
# 🤓 PostToolUse exit 2 can inject feedback to Claude (shown as stderr context).
#    Perfect for: "tests failed after your commit — here's the output"
#    exit 0 with no JSON = silent pass.

set -euo pipefail

INPUT="${B00T_HOOK_INPUT:-$(cat)}"
HOOK_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib/worktree-env.sh
source "${HOOK_DIR}/../../../scripts/lib/worktree-env.sh"
TOOL="$(     echo "$INPUT" | jq -r '.tool_name           // ""' 2>/dev/null)"
COMMAND="$(  echo "$INPUT" | jq -r '.tool_input.command  // ""' 2>/dev/null)"
FILE_PATH="$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null)"
STDOUT="$(   echo "$INPUT" | jq -r '.tool_response.output // ""' 2>/dev/null || echo "")"

# ─── Auto-test after git commit ──────────────────────────────────────────────
if [ "$TOOL" = "Bash" ] && echo "$COMMAND" | grep -qE 'git\s+commit\b'; then
    # Detect test runner from project type
    CWD="${CLAUDE_PROJECT_DIR:-$(pwd)}"

    if [ -f "$CWD/Cargo.toml" ]; then
        echo "🧪 b00t: running cargo test after commit..." >&2
        TEST_OUT="$(cd "$CWD" && cargo test --quiet 2>&1 | tail -20 || true)"
        if echo "$TEST_OUT" | grep -qE '^test result: FAILED|^error\['; then
            echo "⚠️  b00t: cargo test failed after commit:" >&2
            echo "$TEST_OUT" >&2
            # exit 2 feeds this to Claude as context (non-blocking on PostToolUse display)
            exit 2
        fi
        echo "✅ b00t: cargo test passed" >&2

    elif [ -f "$CWD/package.json" ]; then
        PKG_MGR="$(b00t_detect_package_manager "$CWD")"
        echo "🧪 b00t: running $PKG_MGR test after commit..." >&2
        TEST_OUT="$(cd "$CWD" && $PKG_MGR test --passWithNoTests 2>&1 | tail -20 || true)"
        if echo "$TEST_OUT" | grep -qiE 'FAIL|ERROR|failed'; then
            echo "⚠️  b00t: $PKG_MGR test failed after commit:" >&2
            echo "$TEST_OUT" >&2
            exit 2
        fi
        echo "✅ b00t: $PKG_MGR test passed" >&2

    elif [ -f "$CWD/pyproject.toml" ] || [ -f "$CWD/setup.py" ]; then
        echo "🧪 b00t: running pytest after commit..." >&2
        TEST_OUT="$(cd "$CWD" && uv run pytest -q 2>&1 | tail -20 || true)"
        if echo "$TEST_OUT" | grep -qE '^FAILED|ERROR '; then
            echo "⚠️  b00t: pytest failed after commit:" >&2
            echo "$TEST_OUT" >&2
            exit 2
        fi
        echo "✅ b00t: pytest passed" >&2
    fi
fi

# ─── Audit log ────────────────────────────────────────────────────────────────
# 🤓 Lightweight audit trail — tool name + truncated command, not full output.
AUDIT_LOG="${B00T_DIR:-$HOME/.b00t/_b00t_}/agent-hooks/audit.log"
SESSION_ID="$(echo "$INPUT" | jq -r '.session_id // "unknown"' 2>/dev/null)"
printf '%s | session=%s tool=%s cmd=%.120s\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    "$SESSION_ID" \
    "$TOOL" \
    "$COMMAND$FILE_PATH" \
    >> "$AUDIT_LOG" 2>/dev/null || true

# ─── Lint-on-save for Rust files ─────────────────────────────────────────────
if [ "$TOOL" = "Edit" ] || [ "$TOOL" = "Write" ]; then
    if echo "$FILE_PATH" | grep -qE '\.rs$'; then
        # Make clippy-on-save opt-in and only run in Rust projects
        if [ "${B00T_CLIPPY_ON_SAVE:-0}" = "1" ]; then
            CWD="${CLAUDE_PROJECT_DIR:-$(pwd)}"
            if [ -f "$CWD/Cargo.toml" ]; then
                CLIPPY_OUT="$(cd "$CWD" && cargo clippy --quiet 2>&1 | grep -E '^error' | head -10 || true)"
                if [ -n "$CLIPPY_OUT" ]; then
                    echo "⚠️  b00t: clippy errors after edit:" >&2
                    echo "$CLIPPY_OUT" >&2
                    exit 2
                fi
            fi
        fi
    fi
fi

exit 0
