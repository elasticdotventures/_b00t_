#!/usr/bin/env bash
# b00t Ralph loop entrypoint used by b00t-cli up.
# Accepts: --tool <tool> [--max-iterations <n>] [n]

set -euo pipefail

# Defaults (can be overridden by env or flags)
TOOL="${TOOL:-${B00T_TOOL:-claude}}"
MAX_ITERATIONS=10
ROLE="${B00T_ROLE:-developer}"
LOOP_SLEEP_SECONDS="${LOOP_SLEEP_SECONDS:-3}"
STATE_DIR="${B00T_STATE_DIR:-.b00t/ralph}"
STATUS_FILE="${STATE_DIR}/status.json"
LOG_FILE="${STATE_DIR}/loop.log"
MISTRALRS_PORT="${MISTRALRS_PORT:-8181}"
MISTRALRS_API_BASE="${MISTRALRS_API_BASE:-http://localhost:${MISTRALRS_PORT}/v1}"
MISTRALRS_MODEL_ID="${MISTRALRS_MODEL_ID:-mistralai/Mistral-7B-Instruct-v0.3}"
MISTRALRS_MODEL_NAME="${MISTRALRS_MODEL_NAME:-mistral}"

# pi / local-inference configuration
# PI_PROVIDER selects backend: "openai" targets liter-llm gateway (:1234, unified),
#   "llama-cpp" targets direct local OpenAI-compatible inference (:8001). Override via env.
PI_PROVIDER="${PI_PROVIDER:-openai}"
PI_GATEWAY_PORT="${PI_GATEWAY_PORT:-1234}"
PI_DIRECT_PORT="${PI_DIRECT_PORT:-8001}"
if [[ -z "${PI_BASE_URL:-}" ]]; then
    PI_BASE_URL="http://127.0.0.1:${PI_GATEWAY_PORT}/v1"
fi
PI_MODEL="${PI_MODEL:-ch0nky}"
PI_API_KEY="${PI_API_KEY:-local-b00t}"
# 🤓 direct Gemma4 vLLM works through pi's llama-cpp provider; openai provider expects cloud auth semantics.
PI_DIRECT_PROVIDER="${PI_DIRECT_PROVIDER:-llama-cpp}"
OPENCODE_MODEL="${OPENCODE_MODEL:-gemma4-local/ch0nky}"
SELF_IMPROVE_MODE="${B00T_SELF_IMPROVE:-auto}"
ISSUE_FEED="${B00T_GH_ISSUES:-}"
BACKLOG_SNIPPET=""
VALIDATION_SNIPPET=""

# Parse CLI args: --tool <tool> [--max-iterations <n>] [<n>]
while [[ $# -gt 0 ]]; do
    case "$1" in
        --tool) TOOL="$2"; shift 2 ;;
        --max-iterations) MAX_ITERATIONS="$2"; shift 2 ;;
        --role) ROLE="$2"; shift 2 ;;
        --sleep) LOOP_SLEEP_SECONDS="$2"; shift 2 ;;
        [0-9]*) MAX_ITERATIONS="$1"; shift ;;
        --*) echo "[ralph] warning: unrecognized flag '$1', ignoring" >&2; shift ;;
        *) shift ;;
    esac
done

mkdir -p "${STATE_DIR}"

log() { echo "[ralph] $*" >&2; }

http_ok() {
    local url="$1"
    curl -fsS --max-time 2 "${url}" >/dev/null 2>&1
}

resolve_pi_transport() {
    if [[ -n "${PI_BASE_URL:-}" && "${PI_BASE_URL}" != "http://127.0.0.1:${PI_GATEWAY_PORT}/v1" ]]; then
        if [[ -z "${PI_PROVIDER:-}" || "${PI_PROVIDER}" == "openai" ]]; then
            PI_PROVIDER="${PI_DIRECT_PROVIDER}"
        fi
        return 0
    fi

    local gateway_url="http://127.0.0.1:${PI_GATEWAY_PORT}/v1/models"
    local direct_url="http://127.0.0.1:${PI_DIRECT_PORT}/v1/models"

    if [[ "${PI_PROVIDER}" != "llama-cpp" ]] && http_ok "${gateway_url}"; then
        PI_BASE_URL="http://127.0.0.1:${PI_GATEWAY_PORT}/v1"
        return 0
    fi

    if http_ok "${direct_url}"; then
        if [[ "${PI_PROVIDER}" != "${PI_DIRECT_PROVIDER}" ]]; then
            log "gateway unavailable on :${PI_GATEWAY_PORT}; falling back to direct Gemma4 on :${PI_DIRECT_PORT}"
        fi
        PI_PROVIDER="${PI_DIRECT_PROVIDER}"
        PI_BASE_URL="http://127.0.0.1:${PI_DIRECT_PORT}/v1"
        return 0
    fi

    log "no local PI backend reachable (gateway :${PI_GATEWAY_PORT}, direct :${PI_DIRECT_PORT})"
    return 1
}

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

collect_backlog_snippet() {
    local todo_file="TODO-next.md"
    if [[ ! -f "${todo_file}" ]]; then
        BACKLOG_SNIPPET="No TODO-next.md backlog. Start with harness/hive validation."
        return 0
    fi

    BACKLOG_SNIPPET="$(
        sed -n '1,40p' "${todo_file}" \
        | sed 's/\t/ /g' \
        | sed 's/[[:space:]]\+/ /g' \
        | head -20
    )"
}

collect_validation_snippet() {
    VALIDATION_SNIPPET=$'Validation commands:\n- b00t hive status\n- git status --short\n- just validate-mcp\n- cargo test -p b00t-cli commands::agent::tests -- --nocapture\n- cargo test -p b00t-c0re-lib agent_manager -- --nocapture\n- curl -fsS http://127.0.0.1:8001/v1/models'
}

collect_issue_feed() {
    if [[ -n "${ISSUE_FEED}" ]]; then
        return 0
    fi

    if ! command -v gh >/dev/null 2>&1 || ! command -v jq >/dev/null 2>&1; then
        ISSUE_FEED="No GH issue feed available."
        return 0
    fi

    local cli_feed=""
    local b00t_feed=""

    cli_feed="$(
        timeout 5 gh issue list --limit 3 --repo cli/cli --json number,title 2>/dev/null \
        | jq -r '.[] | "- cli/cli#\(.number): \(.title)"' 2>/dev/null || true
    )"
    b00t_feed="$(
        timeout 5 gh issue list --limit 3 --repo elasticdotventures/_b00t_ --json number,title 2>/dev/null \
        | jq -r '.[] | "- _b00t_#\(.number): \(.title)"' 2>/dev/null || true
    )"

    ISSUE_FEED="$(printf "%s\n%s\n" "${cli_feed}" "${b00t_feed}" | sed '/^$/d')"
    if [[ -z "${ISSUE_FEED}" ]]; then
        ISSUE_FEED="No GH issue feed available."
    fi
}

is_self_improve_mode() {
    case "${SELF_IMPROVE_MODE}" in
        true|1|yes) return 0 ;;
        false|0|no) return 1 ;;
        auto)
            [[ "${ROLE}" == "operator" && ( "${TOOL}" == "gemma4" || "${TOOL}" == "opencode" || "${TOOL}" == "pi" ) ]]
            return
            ;;
        *) return 1 ;;
    esac
}

build_prompt() {
    local loop_number="$1"
    local pending
    pending="$(pending_tasks_count)"
    if is_self_improve_mode; then
        cat <<EOF
You are running in b00t Ralph self-improvement loop.
role=${ROLE}
tool=${TOOL}
loop=${loop_number}/${MAX_ITERATIONS}
pending_taskmaster=${pending}

Mission:
- use ONLY self-hosted Gemma4-facing tooling
- improve b00t as an agentic harness
- validate hive state and datum health
- take inspiration from current gh issues, but implement only local repo changes

Hard constraints:
- NEVER use cloud inference providers
- prefer opencode + local Gemma4, local shell, cargo, just, git, b00t, gh read-only
- keep diffs tight; run focused verification after edits
- if blocked, leave a concrete next action instead of hand-waving
- stay inside Gemma4/operator harness scope unless a dependency is directly blocking it
- REJECT unrelated qwen, mistral-only, or generic backlog detours

Current backlog focus:
${BACKLOG_SNIPPET}

Current issue inspiration:
${ISSUE_FEED}

Primary local scope:
- ralphs/ralph-plus-_b00t_/ralph.sh
- b00t.sh
- b00t-cli/src/commands/agent.rs
- b00t-cli/src/commands/up.rs
- b00t-c0re-lib/src/aaiii.rs
- _b00t_/inference-gemma4.hive.toml
- _b00t_/mistralrs-proxy.hive.toml
- justfile

${VALIDATION_SNIPPET}

Operate autonomously for one highest-value step in this iteration.
Return exactly:
1) NEXT_ACTION: what you changed or the exact next local step
2) EXIT_SIGNAL=true|false
EOF
    else
        cat <<EOF
You are running in b00t Ralph loop.
role=${ROLE}
loop=${loop_number}/${MAX_ITERATIONS}
pending_taskmaster=${pending}

Return:
1) NEXT_ACTION: one concise next step
2) EXIT_SIGNAL=true|false
EOF
    fi
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
                opencode run --model "${OPENCODE_MODEL}" "${prompt}" 2>/dev/null || true
            fi
            ;;
        gemma4)
            if command -v opencode >/dev/null 2>&1; then
                opencode run --model "${OPENCODE_MODEL}" "${prompt}" 2>/dev/null || true
            fi
            ;;
        pi)
            # 🤓 Use `openai` + `OPENAI_BASE_URL` for the gateway (:1234); use `llama-cpp` + `LLAMA_CPP_BASE_URL` for direct Gemma4/llama.cpp endpoints.
            if command -v pi >/dev/null 2>&1 && resolve_pi_transport; then
                local _pi_base_url_var
                [[ "${PI_PROVIDER}" == "llama-cpp" ]] && _pi_base_url_var="LLAMA_CPP_BASE_URL" || _pi_base_url_var="OPENAI_BASE_URL"
                env "${_pi_base_url_var}=${PI_BASE_URL}" \
                OPENAI_API_KEY="${PI_API_KEY}" \
                pi -p --provider "${PI_PROVIDER}" --model "${PI_MODEL}" "${prompt}" 2>/dev/null || true
            fi
            ;;
        *)
            ;;
    esac
}

should_exit() {
    local output="$1"
    if echo "${output}" | grep -Eqi 'EXIT_SIGNAL[[:space:]]*[:=][[:space:]]*true'; then
        return 0
    fi
    return 1
}

sanitize_output() {
    local output="$1"

    if is_self_improve_mode; then
        if echo "${output}" | grep -Eqi 'qwen|inference-qwen|qwen3|mistral-7b|foundry'; then
            cat <<'EOF'
NEXT_ACTION: Run `b00t hive status` and validate direct Gemma4 operator files: `_b00t_/inference-gemma4.hive.toml`, `ralphs/ralph-plus-_b00t_/ralph.sh`, `b00t-cli/src/commands/up.rs`, and `b00t-c0re-lib/src/aaiii.rs`.
EXIT_SIGNAL=false
EOF
            return 0
        fi
    fi

    printf '%s\n' "${output}"
}

write_status() {
    local loop_num="$1"
    local state="$2"
    local last="$3"
    local pending
    pending="$(pending_tasks_count)"

    if command -v jq >/dev/null 2>&1; then
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
            > "${STATUS_FILE}" || {
                echo "ralph.sh: failed to write status using jq" >&2
                exit 1
            }
    else
        # Fallback: minimal non-jq status format (key=value lines)
        {
            printf 'timestamp=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
            printf 'tool=%s\n' "${TOOL}"
            printf 'role=%s\n' "${ROLE}"
            printf 'status=%s\n' "${state}"
            printf 'loop=%s\n' "${loop_num}"
            printf 'max_iterations=%s\n' "${MAX_ITERATIONS}"
            printf 'pending_tasks=%s\n' "${pending}"
            printf 'last_output=%s\n' "${last}"
        } > "${STATUS_FILE}"
    fi
}

log "starting b00t Ralph loop: tool=${TOOL} max_iterations=${MAX_ITERATIONS}"
collect_backlog_snippet
collect_validation_snippet
if is_self_improve_mode; then
    collect_issue_feed
fi

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

    output="$(sanitize_output "${output}")"

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
