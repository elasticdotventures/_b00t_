#!/usr/bin/env bash
# ralph-poller — the system-watcher Ralph. Does no coding; samples the hive and
# heralds it so the other Ralphs (and the operator) can see the GPU stay warm.
#
# Env: POLL_INTERVAL_SECS (default 20), POLL_ONESHOT=1, POLL_NO_NATS=1
set -u
INTERVAL="${POLL_INTERVAL_SECS:-20}"
REPO="${B00T_REPO_ROOT:-/home/brianh/.b00t}"
LOG="${REPO}/.b00t/ralph-poller.jsonl"
NATS_BIN="/home/brianh/.local/bin/nats"
NATS_ENV="${HOME}/.b00t/secrets/hive-nats.env"
mkdir -p "${REPO}/.b00t" 2>/dev/null || true
# shellcheck source=scripts/lib/agent-progress.sh
. "$(dirname "$0")/lib/agent-progress.sh" 2>/dev/null || true

sample() {
  local ts gpu_util gpu_mem gpu_temp gpu_pow tasks_pending qwen_health heat
  ts="$(date -Is)"
  read -r gpu_util gpu_mem gpu_temp gpu_pow < <(
    nvidia-smi --query-gpu=utilization.gpu,memory.used,temperature.gpu,power.draw \
      --format=csv,noheader,nounits 2>/dev/null | tr -d ',' | awk '{print $1, $2, $3, $4}'
  )
  gpu_util="${gpu_util:-0}"; gpu_temp="${gpu_temp:-0}"; gpu_pow="${gpu_pow:-0}"
  tasks_pending="$(b00t-cli task list --filter pending 2>/dev/null | grep -c '^\[pending\]' || true)"; tasks_pending="${tasks_pending:-0}"
  if curl -sf -m3 http://127.0.0.1:8001/health >/dev/null 2>&1; then qwen_health="ok"; else qwen_health="down"; fi
  # "heat" = is the office actually getting warmed? power draw is the honest signal.
  heat="cold"; awk "BEGIN{exit !(${gpu_pow%.*} > 120)}" && heat="warming"
  awk "BEGIN{exit !(${gpu_pow%.*} > 250)}" && heat="toasty"

  printf '{"ts":"%s","gpu_util_pct":%s,"gpu_mem_mib":%s,"gpu_temp_c":%s,"gpu_power_w":%s,"heat":"%s","tasks_pending":%s,"qwen_8001":"%s"}\n' \
    "$ts" "${gpu_util:-0}" "${gpu_mem:-0}" "$gpu_temp" "$gpu_pow" "$heat" "$tasks_pending" "$qwen_health" | tee -a "$LOG"

  if [ "${POLL_NO_NATS:-0}" != "1" ] && [ -x "$NATS_BIN" ] && [ -r "$NATS_ENV" ]; then
    # shellcheck disable=SC1090
    ( set -a; . "$NATS_ENV"; set +a
      "$NATS_BIN" pub --user "${HIVE_NATS_USER:-}" --password "${HIVE_NATS_PASSWORD:-}" \
        --server "${NATS_URL:-nats://localhost:4222}" "b00t.hive.mesh.health.gpu" \
        "$(tail -1 "$LOG")" ) >/dev/null 2>&1 || true
  fi
  command -v pr_progress >/dev/null && PR_AGENT=ralph-poller pr_progress "hive.gpu" "${gpu_util:-0}" "" "heat=$heat pending=$tasks_pending qwen=$qwen_health power=${gpu_pow}w"
}

[ "${POLL_ONESHOT:-0}" = "1" ] && { sample; exit 0; }
echo "ralph-poller: every ${INTERVAL}s → $LOG + nats b00t.hive.mesh.health.gpu" >&2
while true; do sample; sleep "$INTERVAL"; done
