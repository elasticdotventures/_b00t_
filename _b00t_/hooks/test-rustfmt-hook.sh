#!/usr/bin/env bash
# Integration tests for rustfmt-post-edit hook.
# Usage: bash test-rustfmt-hook.sh
# Exit 0 = all pass; non-zero = failure count.

set -euo pipefail
HOOK="$(dirname "$0")/rustfmt-post-edit"
PASS=0; FAIL=0

run() {
    local desc="$1"; local input="$2"; local expect_exit="${3:-0}"
    local exit_code=0
    printf '%s' "$input" | bash "$HOOK" >/dev/null 2>&1 || exit_code=$?
    if [[ "$exit_code" -eq "$expect_exit" ]]; then
        printf '  ✅ %s\n' "$desc"; PASS=$((PASS+1))
    else
        printf '  ❌ %s (got exit=%d want %d)\n' "$desc" "$exit_code" "$expect_exit"; FAIL=$((FAIL+1))
    fi
}

echo "=== rustfmt-post-edit hook tests ==="

# 1. Non-.rs file → skip (exit 0)
run "non-rs file skipped" \
    '{"tool_name":"Edit","tool_input":{"file_path":"/tmp/foo.py"}}' 0

# 2. Wrong tool → skip (exit 0)
run "wrong tool skipped" \
    '{"tool_name":"Bash","tool_input":{"command":"cargo build"}}' 0

# 3. Read tool → skip (exit 0)
run "Read tool skipped" \
    '{"tool_name":"Read","tool_input":{"file_path":"/tmp/foo.rs"}}' 0

# 4. Missing file path → skip (exit 0)
run "missing file_path skipped" \
    '{"tool_name":"Edit","tool_input":{}}' 0

# 5. Nonexistent file → skip with warning (exit 0, not error)
run "nonexistent file skipped gracefully" \
    '{"tool_name":"Edit","tool_input":{"file_path":"/tmp/nonexistent-XXXX.rs"}}' 0

# 6. Valid .rs file → rustfmt runs (exit 0)
TMPRS=$(mktemp /tmp/hook-test-XXXXXX.rs)
printf 'fn main(){println!("hello");}' > "$TMPRS"
run "valid rs file formatted" \
    "{\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$TMPRS\"}}" 0
rm -f "$TMPRS"

# 7. Write tool also triggers (exit 0)
TMPRS=$(mktemp /tmp/hook-test-XXXXXX.rs)
printf 'fn foo() { }' > "$TMPRS"
run "Write tool triggers formatting" \
    "{\"tool_name\":\"Write\",\"tool_input\":{\"file_path\":\"$TMPRS\"}}" 0
rm -f "$TMPRS"

# 8. Concurrent invocations don't share temp files
TMPRS1=$(mktemp /tmp/hook-test-XXXXXX.rs); printf 'fn a(){}' > "$TMPRS1"
TMPRS2=$(mktemp /tmp/hook-test-XXXXXX.rs); printf 'fn b(){}' > "$TMPRS2"
printf '%s' "{\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$TMPRS1\"}}" | bash "$HOOK" >/dev/null 2>&1 &
printf '%s' "{\"tool_name\":\"Edit\",\"tool_input\":{\"file_path\":\"$TMPRS2\"}}" | bash "$HOOK" >/dev/null 2>&1 &
wait
# No shared /tmp/rustfmt-hook.err should exist after concurrent runs
if [[ ! -f /tmp/rustfmt-hook.err ]]; then
    printf '  ✅ concurrent runs leave no shared tempfile\n'; PASS=$((PASS+1))
else
    printf '  ❌ shared /tmp/rustfmt-hook.err still exists\n'; FAIL=$((FAIL+1))
fi
rm -f "$TMPRS1" "$TMPRS2"

echo ""
echo "Results: $PASS passed, $FAIL failed"
[[ "$FAIL" -eq 0 ]]
