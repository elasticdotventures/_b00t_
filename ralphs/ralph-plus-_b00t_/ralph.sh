#!/usr/bin/env bash
# b00t Ralph loop entrypoint used by b00t-cli up.
# Accepts: --tool <tool> [--max-iterations <n>] [n]

set -euo pipefail

# Thin wrapper that delegates to the canonical workspace-root b00t.sh
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="${SCRIPT_DIR}/../../b00t.sh"

if [[ ! -x "${TARGET}" ]]; then
    echo "Error: canonical b00t.sh not found or not executable at: ${TARGET}" >&2
    exit 1
fi

exec "${TARGET}" "$@"
pending_tasks_count() {
    local tasks_file=".taskmaster/tasks/tasks.json"
    if [[ ! -f "${tasks_file}" ]]; then
        echo "0"
        return 0
    fi
    if command -v jq >/dev/null 2>&1; then
        jq '[.tasks[]? | select(.status != "done" and .status != "completed")] | length' "${tasks_file}" 2>/dev/null || echo "0"
    else
        echo "0"
    fi
}

build_prompt() {
    local loop_number="$1"
    local pending
    pending="$(pending_tasks_count)"
    cat <<EOF
You are running in b00t Ralph loop.
role=${ROLE}
loop=${loop_number}/${MAX_ITERATIONS}
pending_taskmaster=${pending}

Return:
1) NEXT_ACTION: one concise next step
2) EXIT_SIGNAL=true|false
EOF
}

ensure_mistralrs_server() {
    if curl -fsS "${MISTRALRS_API_BASE}/models" >/dev/null 2>&1; then
        return 0
    fi

    if ! command -v mistralrs-server >/dev/null 2>&1; then
        log "mistralrs-server not found. install with: b00t-cli cli install mistralrs"
        return 1
    fi

    log "starting mistralrs-server on ${MISTRALRS_API_BASE} with model ${MISTRALRS_MODEL_ID}"
    mistralrs-server \
        --port "${MISTRALRS_PORT}" \
        --served-model-name "${MISTRALRS_MODEL_NAME}" \
        --hf-model-id "${MISTRALRS_MODEL_ID}" \
        >> "${STATE_DIR}/mistralrs-server.log" 2>&1 &

    local pid="$!"
    echo "${pid}" > "${STATE_DIR}/mistralrs-server.pid"

    local i
    for i in $(seq 1 30); do
        if curl -fsS "${MISTRALRS_API_BASE}/models" >/dev/null 2>&1; then
            log "mistralrs-server ready"
            return 0
        fi
        sleep 1
    done

    log "mistralrs-server did not become ready in time"
    return 1
}

run_mistralrs_step() {
    local prompt="$1"
    ensure_mistralrs_server || return 1

    if ! command -v jq >/dev/null 2>&1; then
        log "jq is required to build mistralrs request payloads and parse responses. Please install jq (e.g., 'sudo apt install jq' or use your OS package manager)."
        return 1
    fi

    local payload
    payload="$(jq -nc \
        --arg model "${MISTRALRS_MODEL_NAME}" \
        --arg prompt "${prompt}" \
        '{model: $model, temperature: 0.2, max_tokens: 300, messages: [{role:"user", content:$prompt}]}' \
    )"

    local response
    response="$(curl -fsS \
        -H "Content-Type: application/json" \
        -d "${payload}" \
        "${MISTRALRS_API_BASE}/chat/completions" 2>/dev/null || true)"

    if [[ -z "${response}" ]]; then
        log "empty response from mistralrs"
        return 1
    fi

    echo "${response}" | jq -r '.choices[0].message.content // ""' 2>/dev/null
}

run_external_step() {
    local prompt="$1"
    case "${TOOL}" in
        claude)
            if command -v claude >/dev/null 2>&1; then
                claude -p "${prompt}" 2>/dev/null || true
            fi
            ;;
        codex)
            if command -v codex >/dev/null 2>&1; then
                codex exec "${prompt}" 2>/dev/null || true
            fi
            ;;
        amp)
            if command -v amp >/dev/null 2>&1; then
                amp "${prompt}" 2>/dev/null || true
            fi
            ;;
        opencode)
            if command -v opencode >/dev/null 2>&1; then
                opencode "${prompt}" 2>/dev/null || true
            fi
            ;;
        *)
            ;;
    esac
}

should_exit() {
    local output="$1"
    if echo "${output}" | rg -qi 'EXIT_SIGNAL\s*[:=]\s*true'; then
        return 0
    fi
    return 1
}

write_status() {
    local loop_num="$1"
    local state="$2"
    local last="$3"
    local pending
    pending="$(pending_tasks_count)"

    jq -nc \
        --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
        --arg tool "${TOOL}" \
        --arg role "${ROLE}" \
        --arg state "${state}" \
        --arg last "${last}" \
        --argjson loop_num "${loop_num}" \
        --argjson max "${MAX_ITERATIONS}" \
        --argjson pending "${pending}" \
        '{timestamp:$ts, tool:$tool, role:$role, status:$state, loop:$loop_num, max_iterations:$max, pending_tasks:$pending, last_output:$last}' \
        > "${STATUS_FILE}"
}

log "starting b00t Ralph loop: tool=${TOOL} max_iterations=${MAX_ITERATIONS}"

if [[ "${MAX_ITERATIONS}" -eq 0 ]]; then
    write_status 0 "completed" "no-op"
    exit 0
fi

loop=1
while [[ "${loop}" -le "${MAX_ITERATIONS}" ]]; do
    prompt="$(build_prompt "${loop}")"
    output=""

    if [[ "${TOOL}" == "mistralrs" ]]; then
        output="$(run_mistralrs_step "${prompt}" || true)"
    else
        output="$(run_external_step "${prompt}")"
    fi

    if [[ -z "${output}" ]]; then
        output="NEXT_ACTION: run b00t-cli status and continue.
EXIT_SIGNAL=false"
    fi

    write_status "${loop}" "running" "$(echo "${output}" | head -c 500)"
    echo "${output}" >> "${LOG_FILE}"

    if should_exit "${output}"; then
        write_status "${loop}" "completed" "$(echo "${output}" | head -c 500)"
        log "loop completed at iteration ${loop}"
        exit 0
    fi

    if [[ "${loop}" -lt "${MAX_ITERATIONS}" ]]; then
        sleep "${LOOP_SLEEP_SECONDS}"
    fi
    loop=$((loop + 1))
done

write_status "${MAX_ITERATIONS}" "tempfail" "max iterations reached"
log "max iterations reached; requesting restart via exit 75"
exit 75
