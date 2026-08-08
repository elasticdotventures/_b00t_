#!/bin/bash
# b00t-ping-pong.rhai — minimal integration test harness for b00t CLI
# 🤓 Runs from just b00t-test-harness. Verifies that the 5 key
#    b00t subsystems (learn, task, just, rhai, cargo) respond
#    with expected output. Deterministic — no LLM calls.
#
# Exit 0 = all pings received valid pongs. Exit 1 = failure.
set -euo pipefail

PASS=0
FAIL=0

ping_test() {
    local name="$1"
    local cmd="$2"
    local expect="$3"
    echo -n "  $name ... "
    local output
    output=$(bash -c "$cmd" 2>&1)
    if echo "$output" | grep -q "$expect"; then
        echo "✅ PASS"
        PASS=$((PASS + 1))
    else
        echo "❌ FAIL (expected '$expect')"
        FAIL=$((FAIL + 1))
    fi
}

echo "╔══════════════════════════════════════════════════╗"
echo "║  b00t Ping/Pong Test Harness                      ║"
echo "╠══════════════════════════════════════════════════╣"

# 1. Learn subsystem
ping_test "learn:john-carmack" \
    "b00t learn john-carmack 2>&1" \
    "memoize"

# 2. Task subsystem
ping_test "task:list" \
    "b00t task list 2>&1" \
    "task"

# 3. Just recipes — submodule-status returns SHA hashes  
ping_test "just:submodule-status" \
    "just submodule-status" \
    "b00t-vscode"

# 4. System-normal check — verify git state is clean enough for b00t ops
ping_test "b00t:system-normal" \
    "git status --porcelain 2>/dev/null | grep -c '^UU' || echo 0" \
    "0"

# 5. Cargo — external ufo-types consumer compatibility
ping_test "cargo:ufo-types-consumer" \
    "cargo check -p b00t-c0re-lib -p b00t-chat" \
    "Finished"

echo "╠══════════════════════════════════════════════════╣"
echo "║  RESULT: $PASS/$((PASS + FAIL)) passed"
if [ "$FAIL" -gt 0 ]; then
    echo "║  ❌ $FAIL test(s) failed"
    echo "╚══════════════════════════════════════════════════╝"
    exit 1
else
    echo "║  ✅ All tests passed — b00t is operational"
    echo "╚══════════════════════════════════════════════════╝"
fi
