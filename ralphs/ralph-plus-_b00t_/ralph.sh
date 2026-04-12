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


# R1: Metric gate + rollback (karpathy/autoresearch pattern)
# 🤓 score = test-pass-rate after each iteration; rollback if regresses vs baseline
RALPH_METRIC_GATE="${RALPH_METRIC_GATE:-false}"  # enable with RALPH_METRIC_GATE=true
# R5: GOAL.md fitness contract
RALPH_REQUIRE_CRITERIA="${RALPH_REQUIRE_CRITERIA:-false}"
RALPH_TRIAL_BUDGET_SECS="${RALPH_TRIAL_BUDGET_SECS:-300}"  # per-iteration time cap
SCORES_FILE="${STATE_DIR}/scores.jsonl"           # {ts, loop, metric, value}
BASELINE_SCORE=""                                  # set on first successful trial
# Adversarial gemma4 pattern: writer → reviewer → gate
# 🤓 REVIEWER needs CMDB context injected — pure LLM judgment on b00t compliance is unreliable
ADVERSARIAL="${B00T_ADVERSARIAL:-false}"
ADVERSARIAL_REVIEW_THRESHOLD="${B00T_ADVERSARIAL_THRESHOLD:-50}"  # min lines of diff to trigger review

# Task-state compression: preserve task progress across context compression events
# 🤓 Port of OpenHarness v0.1.6 pattern: checkpoint task state to disk before compaction
TASK_STATE_CHECKPOINT="${STATE_DIR}/task_state.json"

# Friction report: worker agents append here; operator triages async
FRICTION_DIR="${STATE_DIR}/friction"

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
    local tasks_file=".b00t/tasks.json"
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
    # 🤓 Prefer b00t task list (native) over TODO-next.md; TODO-next.md is human planning doc
    #    b00t task list --status active --json gives structured pending tasks for ralph
    if command -v b00t-cli >/dev/null 2>&1; then
        local task_json
        task_json="$(b00t-cli task list --status active --json 2>/dev/null || true)"
        if [[ -n "${task_json}" && "${task_json}" != "[]" && "${task_json}" != "no tasks"* ]]; then
            BACKLOG_SNIPPET="Active tasks (b00t task):
${task_json}"
            return 0
        fi
    fi
    # Fallback: first 20 lines of TODO-next.md
    local todo_file="TODO-next.md"
    if [[ ! -f "${todo_file}" ]]; then
        BACKLOG_SNIPPET="No tasks or TODO-next.md backlog. Start with harness/hive validation."
        return 0
    fi
    BACKLOG_SNIPPET="$(sed -n '1,40p' "${todo_file}" | sed 's/	/ /g' | head -20)"
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
pending_tasks=${pending}

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
pending_tasks=${pending}

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

# ── Adversarial writer→reviewer gate (gemma4 only) ────────────────────────────
# 🤓 Reviewer receives CMDB guards as context — without this, compliance check is theatre
run_adversarial_review() {
    local draft="$1"   # path to draft diff/output file
    local task="$2"    # original task description (for reviewer context)

    if [[ "${ADVERSARIAL}" != "true" ]]; then
        echo "PASS"
        return 0
    fi

    # Only review if diff is substantial enough to warrant cost
    local line_count
    line_count=$(wc -l < "${draft}" 2>/dev/null || echo 0)
    if [[ "${line_count}" -lt "${ADVERSARIAL_REVIEW_THRESHOLD}" ]]; then
        echo "PASS (trivial diff, review skipped)"
        return 0
    fi

    if ! command -v pi >/dev/null 2>&1; then
        log "pi not available; skipping adversarial review"
        echo "PASS (pi unavailable)"
        return 0
    fi

    if ! http_ok "http://127.0.0.1:${PI_DIRECT_PORT}/v1/models"; then
        log "gemma4 not reachable; skipping adversarial review"
        echo "PASS (gemma4 unavailable)"
        return 0
    fi

    local guards=""
    local guards_file="${B00T_REPO_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || echo .)}/_b00t_/hive-guards.hive.toml"
    [[ -f "${guards_file}" ]] && guards="$(cat "${guards_file}")"

    local review_prompt
    review_prompt="[REVIEWER] You are an adversarial b00t hive compliance reviewer.

Active guards:
${guards}

Task: ${task}

Draft output (diff/code):
$(cat "${draft}")

Check ONLY:
1. Guard violations (e.g., pip install, docker run, rm -rf without justification)
2. DRY violations (new code that duplicates known OSS functionality)
3. Non-laconic commentary (platitudes, apologies, over-explanation)
4. b00t gospel violations (cloud inference, raw template reads, etc.)

Output exactly one line: PASS or FAIL:<specific reason>"

    local result
    result="$(LLAMA_CPP_BASE_URL="http://127.0.0.1:${PI_DIRECT_PORT}/v1" \
        OPENAI_API_KEY=local-b00t \
        pi -p "${review_prompt}" \
        --provider llama-cpp --model "${PI_MODEL}" \
        2>/dev/null | tail -1 || echo "PASS (reviewer error)")"

    echo "${result}"
}

# ── Task-state checkpoint (preserve across context compression) ───────────────
# 🤓 Checkpoints pending b00t task state to disk so ralph can resume
#    after context compression without losing work-in-progress task state.
checkpoint_task_state() {
    local loop_num="$1"
    local tasks_file=".b00t/tasks.json"

    mkdir -p "${STATE_DIR}"

    local tmp_checkpoint
    tmp_checkpoint="$(mktemp "${STATE_DIR}/.tmp_checkpoint_XXXXXX")" || return 0
    if command -v jq >/dev/null 2>&1 && [[ -f "${tasks_file}" ]]; then
        if jq -nc \
            --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
            --argjson loop "${loop_num}" \
            --argjson tasks "$(cat "${tasks_file}")" \
            '{checkpoint_ts:$ts, loop:$loop, tasks:$tasks}' \
            > "${tmp_checkpoint}" 2>/dev/null; then
            mv "${tmp_checkpoint}" "${TASK_STATE_CHECKPOINT}"
        else
            rm -f "${tmp_checkpoint}"
        fi
    else
        if printf '{"checkpoint_ts":"%s","loop":%s}\n' \
            "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "${loop_num}" \
            > "${tmp_checkpoint}" 2>/dev/null; then
            mv "${tmp_checkpoint}" "${TASK_STATE_CHECKPOINT}"
        else
            rm -f "${tmp_checkpoint}"
        fi
    fi
}

restore_task_state() {
    if [[ ! -f "${TASK_STATE_CHECKPOINT}" ]]; then
        return 0
    fi

    local tasks_file=".b00t/tasks.json"
    if command -v jq >/dev/null 2>&1 && [[ -f "${TASK_STATE_CHECKPOINT}" ]]; then
        local restored_loop
        restored_loop="$(jq -r '.loop // 0' "${TASK_STATE_CHECKPOINT}" 2>/dev/null || echo 0)"
        log "restored task state from checkpoint (loop ${restored_loop})"
        # Restore b00t tasks if present in checkpoint and tasks.json missing
        if [[ ! -f "${tasks_file}" ]] && jq -e '.tasks' "${TASK_STATE_CHECKPOINT}" >/dev/null 2>&1; then
            mkdir -p .b00t
            jq '.tasks' "${TASK_STATE_CHECKPOINT}" > "${tasks_file}" 2>/dev/null || true
            log "restored .b00t/tasks.json from checkpoint"
        fi
    fi
}

# ── Friction report (worker end-of-session artifact) ──────────────────────────
# 🤓 Workers append friction here; executive/operator NEVER reads raw reports inline
append_friction_report() {
    local agent_id="${AGENT_ID:-ralph-${TOOL}}"
    local task_summary="${1:-unknown task}"
    local friction="${2:-none}"
    local confidence="${3:-MEDIUM}"

    mkdir -p "${FRICTION_DIR}"
    local report_file="${FRICTION_DIR}/${agent_id}-$(date +%Y%m%dT%H%M%S).md"
    cat > "${report_file}" << EOF
## Friction Report — ${agent_id} @ $(date -u +%Y-%m-%dT%H:%M:%SZ)
### Task: ${task_summary}
### Friction:
${friction}
### Confidence: ${confidence}
EOF
    log "friction report written: ${report_file}"
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

# ── R2: Keyword tier pre-filter (hermes-agent smart_model_routing pattern) ─────
# Zero-cost routing: classify task before invoking inference.
# Routing order is intentional:
#   1) backtick presence        -> ch0nky
#   2) complex keyword match    -> ch0nky
#   3) otherwise, sm0l iff (char_count ≤ 160 OR word_count ≤ 28)
#   4) else                     -> ch0nky
_COMPLEX_KEYWORDS="debug|implement|architecture|refactor|design|analyze|integrate|migrate|security|performance"
route_to_tier() {
    local prompt="$1"
    local char_count="${#prompt}"
    local word_count
    word_count="$(echo "${prompt}" | wc -w)"
    # backtick → always ch0nky
    if echo "${prompt}" | grep -q '`'; then
        echo "ch0nky"; return
    fi
    # complex keyword match → ch0nky
    if echo "${prompt}" | grep -Eiq "${_COMPLEX_KEYWORDS}"; then
        echo "ch0nky"; return
    fi
    # short pure prose → sm0l
    if [[ "${char_count}" -le 160 || "${word_count}" -le 28 ]]; then
        echo "sm0l"; return
    fi
    echo "ch0nky"
}

run_external_step() {
    local prompt="$1"
    # Keyword gate: override TOOL to sm0l tier when prompt is trivial
    local effective_tool="${TOOL}"
    if [[ "${TOOL}" == "pi" || "${TOOL}" == "gemma4" || "${TOOL}" == "opencode" ]]; then
        local tier
        tier="$(route_to_tier "${prompt}")"
        if [[ "${tier}" == "sm0l" ]]; then
            log "tier-routed: sm0l (keyword-gate) — bypassing ch0nky for this prompt"
            effective_tool="pi-sm0l"
        fi
    fi
    case "${effective_tool}" in
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
        pi-sm0l)
            # R2: sm0l tier — routed here by route_to_tier() keyword gate
            # 🤓 Derive base URL + model from env (B00T_AI_SM0L_BASE / B00T_AI_SM0L_MODEL);
            #    default port :8000 / model qwen3-coder matches inference-sm0l.hive.toml
            if command -v pi >/dev/null 2>&1; then
                local sm0l_base="${B00T_AI_SM0L_BASE:-http://127.0.0.1:8000/v1}"
                local sm0l_model="${B00T_AI_SM0L_MODEL:-qwen3-coder}"
                LLAMA_CPP_BASE_URL="${sm0l_base}" \
                OPENAI_API_KEY="${PI_API_KEY}" \
                pi -p --provider llama-cpp --model "${sm0l_model}" "${prompt}" 2>/dev/null || true
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


# ── R1: Metric gate helpers (karpathy/autoresearch) ──────────────────────────
# 🤓 Metric = test-pass-rate: (passing / total) × 100, sourced from cargo test --message-format json
# If cargo not available, metric = 0 (no regression — don't rollback non-testable changes)
measure_test_score() {
    if ! command -v cargo >/dev/null 2>&1; then
        echo "0"
        return 0
    fi
    local pass=0 fail=0
    while IFS= read -r line; do
        local event
        event="$(echo "${line}" | jq -r '.event // empty' 2>/dev/null)"
        [[ "${event}" == "test" ]] || continue
        local result
        result="$(echo "${line}" | jq -r '.type // empty' 2>/dev/null)"
        [[ "${result}" == "ok" ]]      && pass=$((pass + 1))
        [[ "${result}" == "FAILED" ]]  && fail=$((fail + 1))
    done < <(timeout "${RALPH_TRIAL_BUDGET_SECS}" cargo test --message-format json 2>/dev/null || true)
    local total=$((pass + fail))
    if [[ "${total}" -eq 0 ]]; then echo "0"; return 0; fi
    echo $((pass * 100 / total))
}

record_score() {
    local loop_num="$1" score="$2"
    mkdir -p "${STATE_DIR}"
    printf '{"ts":"%s","loop":%s,"metric":"test-pass-rate","value":%s}
'         "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "${loop_num}" "${score}"         >> "${SCORES_FILE}" 2>/dev/null || true
}

metric_gate_check() {
    local loop_num="$1"
    [[ "${RALPH_METRIC_GATE}" != "true" ]] && return 0  # gate disabled
    local score
    score="$(measure_test_score)"
    record_score "${loop_num}" "${score}"
    log "metric gate loop=${loop_num}: score=${score}% baseline=${BASELINE_SCORE}%"
    if [[ -z "${BASELINE_SCORE}" ]]; then
        BASELINE_SCORE="${score}"  # first trial sets baseline
        log "metric baseline set: ${BASELINE_SCORE}%"
        return 0
    fi
    if [[ "${score}" -lt "${BASELINE_SCORE}" ]]; then
        log "metric regression: ${score}% < ${BASELINE_SCORE}% — rolling back"
        git stash 2>/dev/null || true
        append_friction_report "metric-gate-${loop_num}"             "- Score regressed ${BASELINE_SCORE}% → ${score}%; git stash applied"             "MEDIUM"
        return 1  # signal rollback
    fi
    BASELINE_SCORE="${score}"  # promote new baseline
    return 0
}


# -- R5: GOAL.md fitness contract (karpathy/lazy-developer) ----------------
# 🤓 emit_goal_md writes trial intent; check_task_has_criteria enforces AC gate
emit_goal_md() {
    local task_title="$1" task_criteria="$2" loop_num="$3"
    local goal_file="${STATE_DIR}/GOAL-${loop_num}.md"
    {
        printf "# GOAL -- loop %s\n\n## Task\n%s\n\n" "${loop_num}" "${task_title}"
        printf "## Acceptance Criteria\n%s\n\n" "${task_criteria}"
        printf "## Metric\ntest-pass-rate -- see scores.jsonl\n\n"
        printf "## Constraint\n- RALPH_TRIAL_BUDGET_SECS=%s\n- TOOL=%s\n" "${RALPH_TRIAL_BUDGET_SECS}" "${TOOL}"
    } > "${goal_file}"
    log "goal written: ${goal_file}"
}

check_task_has_criteria() {
    local task_json="$1"
    [[ "${RALPH_REQUIRE_CRITERIA}" != "true" ]] && return 0
    command -v jq >/dev/null 2>&1 || return 0
    local n
    n="$(echo "${task_json}" | jq '.acceptance_criteria | length // 0' 2>/dev/null || echo 0)"
    if [[ "${n}" -eq 0 ]]; then
        log "task missing acceptance_criteria -- skipping (RALPH_REQUIRE_CRITERIA=true)"
        return 1
    fi
    return 0
}


# -- R7: Elo scoring for adversarial loop (autoevolve pattern) --------------
# 🤓 writer_elo and reviewer_elo stored in scores.jsonl; K=32; initial=1200
#    Pareto front tracked over (test_pass_rate, diff_penalty); used for best-trial selection
ELO_FILE="${STATE_DIR}/elo.json"  # {"writer":1200,"reviewer":1200}

elo_load() {
    if [[ -f "${ELO_FILE}" ]] && command -v jq >/dev/null 2>&1; then
        ELO_WRITER="$(jq -r '.writer // 1200' "${ELO_FILE}" 2>/dev/null || echo 1200)"
        ELO_REVIEWER="$(jq -r '.reviewer // 1200' "${ELO_FILE}" 2>/dev/null || echo 1200)"
    else
        ELO_WRITER=1200; ELO_REVIEWER=1200
    fi
}

elo_update() {
    local verdict="$1"  # PASS or FAIL
    command -v jq >/dev/null 2>&1 || return 0
    elo_load
    local K=32 scale=400
    # Expected score for writer vs reviewer (logistic)
    # e_w = 1/(1+10^((r-w)/400));  e_r = 1-e_w
    local e_w e_r
    e_w=$(python3 -c "import math; r,w=${ELO_REVIEWER},${ELO_WRITER}; print(round(1/(1+10**((r-w)/400)),4))" 2>/dev/null || echo "0.5")
    if [[ "${verdict}" == "PASS" ]]; then
        # Writer wins: writer +, reviewer -
        ELO_WRITER=$(python3 -c "print(round(${ELO_WRITER}+${K}*(1-${e_w})))" 2>/dev/null || echo "${ELO_WRITER}")
        ELO_REVIEWER=$(python3 -c "print(round(${ELO_REVIEWER}+${K}*(${e_w}-1)))" 2>/dev/null || echo "${ELO_REVIEWER}")
    else
        # Reviewer wins: reviewer +, writer -
        ELO_WRITER=$(python3 -c "print(round(${ELO_WRITER}+${K}*(0-${e_w})))" 2>/dev/null || echo "${ELO_WRITER}")
        ELO_REVIEWER=$(python3 -c "print(round(${ELO_REVIEWER}+${K}*(1-${e_w})))" 2>/dev/null || echo "${ELO_REVIEWER}")
    fi
    jq -n --argjson w "${ELO_WRITER}" --argjson r "${ELO_REVIEWER}" '{writer:$w,reviewer:$r}' > "${ELO_FILE}" 2>/dev/null || true
    log "elo update: writer=${ELO_WRITER} reviewer=${ELO_REVIEWER} (${verdict})"
}

# record_score_with_elo: extends R1 record_score to include Elo and diff_penalty
record_score_elo() {
    local loop_num="$1" test_score="$2" diff_lines="$3" verdict="$4"
    elo_load
    local ts
    ts="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    printf '{"ts":"%s","loop":%s,"metric":"test-pass-rate","value":%s,"diff_penalty":%s,"writer_elo":%s,"reviewer_elo":%s,"verdict":"%s"}\n' \
        "${ts}" "${loop_num}" "${test_score}" "${diff_lines}" "${ELO_WRITER}" "${ELO_REVIEWER}" "${verdict}" \
        >> "${SCORES_FILE}" 2>/dev/null || true
}


# -- R3: Trajectory compression -> RL training data (hermes-agent pattern) --
# 🤓 Captures first 3 + last 4 turns; compresses middle via sm0l summarizer
#    Writes .b00t/ralph/trajectory-<ts>.jsonl for future fine-tune pipeline
RALPH_TRAJECTORY_TOKENS="${RALPH_TRAJECTORY_TOKENS:-15250}"

emit_trajectory_jsonl() {
    local exit_reason="$1"
    local log_file="${LOG_FILE}"
    [[ ! -f "${log_file}" ]] && return 0

    local ts
    ts="$(date -u +%Y%m%dT%H%M%S)"
    local traj_file="${STATE_DIR}/trajectory-${ts}.jsonl"

    # Load log lines as turns; log format: "[ralph] ..." lines are system, rest are model
    local total_lines
    total_lines="$(wc -l < "${log_file}" 2>/dev/null || echo 0)"
    local protect_first=3 protect_last=4

    # Emit header record
    printf '{"type":"header","ts":"%s","tool":"%s","role":"%s","loops":%s,"exit":"%s","scores_file":"%s"}\n' \
        "${ts}" "${TOOL}" "${ROLE}" "${loop:-0}" "${exit_reason}" "${SCORES_FILE}" \
        > "${traj_file}" 2>/dev/null || return 0

    # Emit first N turns verbatim
    local line_num=0
    while IFS= read -r line && [[ "${line_num}" -lt "${protect_first}" ]]; do
        printf '{"type":"turn","idx":%s,"protected":true,"content":%s}\n' \
            "${line_num}" "$(printf '%s' "${line}" | jq -Rs . 2>/dev/null || echo 'null')" \
            >> "${traj_file}" 2>/dev/null
        line_num=$((line_num + 1))
    done < "${log_file}"

    # Middle turns: emit compressed placeholder (sm0l summarization deferred to pipeline)
    local middle_count=$(( total_lines - protect_first - protect_last ))
    if [[ "${middle_count}" -gt 0 ]]; then
        printf '{"type":"compressed","start_idx":%s,"end_idx":%s,"line_count":%s,"note":"compress via sm0l"}\n' \
            "${protect_first}" "$(( total_lines - protect_last ))" "${middle_count}" \
            >> "${traj_file}" 2>/dev/null
    fi

    # Emit last N turns verbatim
    local last_start=$(( total_lines - protect_last + 1 ))
    [[ "${last_start}" -lt 1 ]] && last_start=1
    tail -n "${protect_last}" "${log_file}" | while IFS= read -r line; do
        printf '{"type":"turn","protected":true,"content":%s}\n' \
            "$(printf '%s' "${line}" | jq -Rs . 2>/dev/null || echo 'null')" \
            >> "${traj_file}" 2>/dev/null
    done

    # Append Elo + score summary
    elo_load 2>/dev/null || true
    printf '{"type":"footer","writer_elo":%s,"reviewer_elo":%s,"target_tokens":%s}\n' \
        "${ELO_WRITER:-1200}" "${ELO_REVIEWER:-1200}" "${RALPH_TRAJECTORY_TOKENS}" \
        >> "${traj_file}" 2>/dev/null

    log "trajectory written: ${traj_file} (${total_lines} turns)"
}

# 🤓 B00T_TEST_MODE=1: source this script to load functions without executing the main loop
# Enables: `B00T_TEST_MODE=1 source b00t.sh; restore_task_state` in unit tests
if [[ "${B00T_TEST_MODE:-}" == "1" ]]; then
    return 0 2>/dev/null || exit 0
fi

log "starting b00t Ralph loop: tool=${TOOL} max_iterations=${MAX_ITERATIONS} adversarial=${ADVERSARIAL}"
mkdir -p "${FRICTION_DIR}"
collect_backlog_snippet
collect_validation_snippet
restore_task_state
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

    # R5: GOAL.md + acceptance_criteria gate
    if command -v b00t-cli >/dev/null 2>&1; then
        _task_json="$(b00t-cli task next --json 2>/dev/null || true)"
        if [[ -n "${_task_json}" && "${_task_json}" != "no actionable"* ]]; then
            _task_title="$(echo "${_task_json}" | jq -r '.title // empty' 2>/dev/null || true)"
            _task_criteria="$(echo "${_task_json}" | jq -r '.acceptance_criteria[]?' 2>/dev/null | sed 's/^/- /' || true)"
            [[ -n "${_task_title}" ]] && emit_goal_md "${_task_title}" "${_task_criteria}" "${loop}"
            if ! check_task_has_criteria "${_task_json}"; then
                _skip_id="$(echo "${_task_json}" | jq -r '.id // empty' 2>/dev/null || true)"
                [[ -n "${_skip_id}" ]] && b00t-cli task update "${_skip_id}" --status deferred 2>/dev/null || true
                loop=$((loop + 1)); continue
            fi
        fi
    fi

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

    # Adversarial review gate: write draft, reviewer checks compliance
    if [[ "${ADVERSARIAL}" == "true" ]]; then
        draft_file="${STATE_DIR}/draft_${loop}.txt"
        echo "${output}" > "${draft_file}"
        review_result="$(run_adversarial_review "${draft_file}" "$(echo "${prompt}" | head -3)")"
        log "adversarial review loop=${loop}: ${review_result}"
        # R7: Elo update based on adversarial verdict
        if echo "${review_result}" | grep -q "^PASS"; then
            elo_update "PASS"
        else
            elo_update "FAIL"
        fi
        if echo "${review_result}" | grep -q "^FAIL:"; then
            # Append reviewer rejection as friction signal and re-queue
            append_friction_report \
                "loop-${loop}" \
                "- Reviewer rejected output: ${review_result}" \
                "LOW"
            output="NEXT_ACTION: reviewer rejected previous output (${review_result}). Revise and retry.
EXIT_SIGNAL=false"
        fi
    fi

    # Checkpoint task state before writing status (survives context compression)
    checkpoint_task_state "${loop}"

    # R1: metric gate — measure test-pass-rate; rollback if regression
    if ! metric_gate_check "${loop}"; then
        log "metric gate triggered rollback at loop ${loop}; continuing"
        output="NEXT_ACTION: metric regression detected and rolled back. Revise approach.
EXIT_SIGNAL=false"
    fi

    write_status "${loop}" "running" "$(echo "${output}" | head -c 500)"
    echo "${output}" >> "${LOG_FILE}"

    if should_exit "${output}"; then
        write_status "${loop}" "completed" "$(echo "${output}" | head -c 500)"
        log "loop completed at iteration ${loop}"
        # Final friction report on clean exit (captures any residual friction)
        if [[ "${ADVERSARIAL}" == "true" ]]; then
            append_friction_report "loop-final" "- Clean exit at iteration ${loop}" "HIGH"
        fi
        emit_trajectory_jsonl "completed"
        exit 0
    fi

    if [[ "${loop}" -lt "${MAX_ITERATIONS}" ]]; then
        sleep "${LOOP_SLEEP_SECONDS}"
    fi
    loop=$((loop + 1))
done

write_status "${MAX_ITERATIONS}" "tempfail" "max iterations reached"
log "max iterations reached; requesting restart via exit 75"
# Friction report on tempfail: likely a hard problem; operator should inspect
append_friction_report "loop-tempfail" \
    "- Reached max iterations (${MAX_ITERATIONS}) without EXIT_SIGNAL=true\n- Tool: ${TOOL}, Role: ${ROLE}" \
    "LOW"
emit_trajectory_jsonl "tempfail"
exit 75
