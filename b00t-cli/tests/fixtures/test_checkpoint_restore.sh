#!/usr/bin/env bash
# H2 test: verify b00t.sh restore_task_state reads task_state.json and populates tasks.json
# Usage: bash test_checkpoint_restore.sh
# Exit: 0 = PASS, 1 = FAIL
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "${TMPDIR}"' EXIT

# ── Setup temp state and working dirs ─────────────────────────────────────────
STATE_DIR="${TMPDIR}/ralph"
TASKS_DIR="${TMPDIR}/.b00t"
mkdir -p "${STATE_DIR}" "${TASKS_DIR}"

CHECKPOINT="${STATE_DIR}/task_state.json"

# Write mock checkpoint with tasks payload
cat > "${CHECKPOINT}" << 'JSON'
{
  "checkpoint_ts": "2026-01-01T00:00:00Z",
  "loop": 3,
  "tasks": {
    "tasks": [
      {"id": 1, "title": "Test task A", "status": "pending"},
      {"id": 2, "title": "Test task B", "status": "done"}
    ]
  }
}
JSON

# ── Source b00t.sh in test mode to load functions only ────────────────────────
# B00T_TEST_MODE=1 skips the main loop execution
# B00T_STATE_DIR overrides STATE_DIR inside b00t.sh

# shellcheck disable=SC1090
B00T_TEST_MODE=1 B00T_STATE_DIR="${STATE_DIR}" \
    bash -c "source '${REPO_ROOT}/b00t.sh'; restore_task_state" 2>/dev/null

# Note: tasks.json path is relative to CWD inside b00t.sh's restore_task_state
# We need to run from TMPDIR so .b00t/tasks.json lands there
(
    cd "${TMPDIR}"
    B00T_TEST_MODE=1 B00T_STATE_DIR="${STATE_DIR}" \
        bash -c "source '${REPO_ROOT}/b00t.sh'; restore_task_state" 2>/dev/null
)

# ── Assertions ────────────────────────────────────────────────────────────────
TASKS_JSON="${TMPDIR}/.b00t/tasks.json"

if [[ ! -f "${TASKS_JSON}" ]]; then
    echo "FAIL: tasks.json not created at ${TASKS_JSON}" >&2
    exit 1
fi

# Verify tasks array has 2 entries
TASK_COUNT="$(jq '.tasks | length' "${TASKS_JSON}" 2>/dev/null || echo 0)"
if [[ "${TASK_COUNT}" -ne 2 ]]; then
    echo "FAIL: expected 2 tasks in tasks.json, got ${TASK_COUNT}" >&2
    cat "${TASKS_JSON}" >&2
    exit 1
fi

echo "PASS: restore_task_state correctly populated .b00t/tasks.json (${TASK_COUNT} tasks)"
exit 0
