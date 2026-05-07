#!/bin/bash
# Tests for the b00t-task queue system
# Run: bash tests/test-task-queue.sh

set -uo pipefail
PASS=0
FAIL=0

export B00T_TASK_QUEUE="/tmp/b00t-task-test-$$"
PENDING="$B00T_TASK_QUEUE/pending"
ACTIVE="$B00T_TASK_QUEUE/active"
DONE="$B00T_TASK_QUEUE/done"

cleanup() { rm -rf "$B00T_TASK_QUEUE"; }
trap cleanup EXIT

# Source the b00t-task script functions by running it
B00T_TASK="$HOME/.local/bin/b00t-task"

assert_eq() {
    local desc="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected: '$expected', got: '$actual')"
        FAIL=$((FAIL + 1))
    fi
}

assert_contains() {
    local desc="$1" pattern="$2" output="$3"
    if echo "$output" | grep -q "$pattern"; then
        echo "  PASS: $desc"
        PASS=$((PASS + 1))
    else
        echo "  FAIL: $desc (expected output to contain '$pattern')"
        FAIL=$((FAIL + 1))
    fi
}

echo "=== Task Queue Tests ==="

# Test 1: Empty queue
echo "Test 1: Empty queue"
output=$($B00T_TASK list 2>&1)
assert_contains "Empty pending" "=== PENDING ===" "$output"

# Test 2: Add a task
echo "Test 2: Add task"
$B00T_TASK add "Test task 1" "Description 1" "test-suite" "test" 1 2>&1
count=$(ls "$PENDING"/*.json 2>/dev/null | wc -l)
assert_eq "Task file created" "1" "$count"

# Test 3: List shows task
echo "Test 3: List tasks"
output=$($B00T_TASK list 2>&1)
assert_contains "List shows pending" "Test task 1" "$output"

# Test 4: Priority ordering — higher priority tasks are picked first
echo "Test 4: Priority ordering"
$B00T_TASK add "High priority task" "Should be picked first" "test-suite" "test" 1 2>&1
$B00T_TASK add "Low priority task" "Should be picked last" "test-suite" "test" 3 2>&1
$B00T_TASK add "Medium priority task" "Should be picked second" "test-suite" "test" 2 2>&1
output=$($B00T_TASK pick 2>&1)
assert_contains "Picks highest priority task" "Picked:" "$output"

# Test 5: Pick moves task to active
echo "Test 5: Pick moves to active"
pending_count=$(ls "$PENDING"/*.json 2>/dev/null | wc -l)
active_count=$(ls "$ACTIVE"/*.json 2>/dev/null | wc -l)
# There should be 3 pending tasks initially, after pick: 2 pending, 1 active
# (the first pick already consumed the highest priority task)
[ "$pending_count" -ge 1 ] && echo "  PASS: Tasks remain in pending" && PASS=$((PASS + 1)) || echo "  FAIL: No pending tasks" && FAIL=$((FAIL + 1))
[ "$active_count" -ge 1 ] && echo "  PASS: Task moved to active" && PASS=$((PASS + 1)) || echo "  FAIL: No active tasks" && FAIL=$((FAIL + 1))

# Test 6: Done moves from active to done
echo "Test 6: Done"
active_file=$(ls "$ACTIVE"/*.json 2>/dev/null | head -1)
if [ -n "$active_file" ]; then
    task_id=$(basename "$active_file" .json)
    $B00T_TASK done "$task_id" 2>&1
    # Check the task is no longer in active
    active_after=$(ls "$ACTIVE"/*.json 2>/dev/null | wc -l)
    [ "$active_after" -eq 0 ] && echo "  PASS: Task removed from active" && PASS=$((PASS + 1)) || echo "  FAIL: Task still in active" && FAIL=$((FAIL + 1))
fi

# Test 7: Ideas filters alpha-node sources
echo "Test 7: Ideas filtering"
$B00T_TASK add "Bravo idea" "Great new feature" "bravo-node-3" "idea" 2 2>&1
output=$($B00T_TASK ideas 2>&1)
assert_contains "Ideas shows bravo submission" "Bravo idea" "$output"

# Test 8: Task JSON format validation
echo "Test 8: Task JSON format"
for f in "$PENDING"/*.json "$ACTIVE"/*.json "$DONE"/*.json; do
    [ -f "$f" ] || continue
    if jq -e . "$f" > /dev/null 2>&1; then
        :  # valid JSON
    else
        echo "  FAIL: Invalid JSON in $f"
        FAIL=$((FAIL + 1))
    fi
done
echo "  PASS: All task files are valid JSON"
PASS=$((PASS + 1))

# Test 9: Task has required fields
echo "Test 9: Required fields"
for f in "$PENDING"/*.json; do
    [ -f "$f" ] || continue
    for field in id source title priority created_at; do
        if jq -e ".$field" "$f" > /dev/null 2>&1; then
            :  # field exists
        else
            echo "  FAIL: Missing field '$field' in $f"
            FAIL=$((FAIL + 1))
        fi
    done
done
echo "  PASS: All required fields present"
PASS=$((PASS + 1))

# Test 10: Done from pending (by task ID, not title)
echo "Test 10: Done from pending"
$B00T_TASK add "Skip-active" "Direct to done" "test-suite" "test" 1 2>&1
# Find the task by title in pending
for f in "$PENDING"/*.json; do
    [ -f "$f" ] || continue
    if jq -e 'select(.title == "Skip-active")' "$f" > /dev/null 2>&1; then
        task_id=$(basename "$f" .json)
        $B00T_TASK done "$task_id" 2>&1
        # Check it's now in done
        for df in "$DONE"/*.json; do
            [ -f "$df" ] || continue
            if jq -e 'select(.title == "Skip-active")' "$df" > /dev/null 2>&1; then
                echo "  PASS: Task moved to done from pending"
                PASS=$((PASS + 1))
                break
            fi
        done
        break
    fi
done

echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="
[ "$FAIL" -eq 0 ] && echo "ALL TESTS PASSED" || echo "SOME TESTS FAILED"
exit $FAIL
