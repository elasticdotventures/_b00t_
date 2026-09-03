#!/usr/bin/env bash
# agent-progress.sh — sourced library. Every hive agent/loop emits regular,
# timestamped progress + an ETA, and registers the ETA forecast (and later its
# accuracy) with ledgrrr.
#
#   source scripts/lib/agent-progress.sh
#   pr_forecast  <exp_id> <predicted_secs> [service]      # once, at task start
#   pr_progress  <task> <pct> [eta_secs] [note]           # every cycle
#   pr_settle    <exp_id> <actual_secs> [service]         # once, at task end → prints accuracy
#
# Writes:
#   .b00t/agent-progress.jsonl        one line per pr_progress
#   .b00t/forecasts.jsonl             forecast + settle rows (predicted, actual, abs_err_s, pct_err)
#   .b00t/ledgrrr-focus-queue.jsonl   pending mcp__ledgrrr__ledgerr_focus calls (drained by an MCP-capable agent)
# Publishes (best-effort): NATS b00t.hive.mesh.progress.<task> , b00t.hive.mesh.forecast.<exp>
set -u
: "${B00T_REPO_ROOT:=/home/brianh/.b00t}"
: "${PR_AGENT:=${AGENT_ID:-$(basename "${0:-agent}" .sh)}}"
_PR_DIR="${B00T_REPO_ROOT}/.b00t"; mkdir -p "$_PR_DIR" 2>/dev/null || true
_PR_NATS_BIN="/home/brianh/.local/bin/nats"
_PR_NATS_ENV="${HOME}/.b00t/secrets/hive-nats.env"

_pr_iso() { date -Is; }
_pr_pub() { # subject payload
  [ "${PR_NO_NATS:-0}" = "1" ] && return 0
  [ -x "$_PR_NATS_BIN" ] && [ -r "$_PR_NATS_ENV" ] || return 0
  # shellcheck disable=SC1090
  ( set -a; . "$_PR_NATS_ENV"; set +a
    "$_PR_NATS_BIN" pub --user "${HIVE_NATS_USER:-}" --password "${HIVE_NATS_PASSWORD:-}" \
      --server "${NATS_URL:-nats://localhost:4222}" "$1" "$2" ) >/dev/null 2>&1 || true
}
_pr_queue_focus() { # billing service exp variant billed
  printf '{"tool":"mcp__ledgrrr__ledgerr_focus","action":"append_focus_record","billing_account_id":"%s","service_name":"%s","agent_id":"%s","experiment_id":"%s","variant":"%s","billed_cost":%s,"effective_cost":%s,"queued_ts":"%s"}\n' \
    "b00t-hive" "$2" "$PR_AGENT" "$3" "$4" "$5" "$5" "$(_pr_iso)" >> "$_PR_DIR/ledgrrr-focus-queue.jsonl"
}

pr_progress() { # task pct [eta_secs] [note]
  local task="$1" pct="${2:-}" eta="${3:-}" note="${4:-}" ts eta_ts=""
  ts="$(_pr_iso)"
  [ -n "$eta" ] && eta_ts="$(date -Is -d "+${eta} seconds" 2>/dev/null || true)"
  local line
  line="$(printf '{"ts":"%s","agent":"%s","task":"%s","pct":%s,"eta_secs":%s,"eta_ts":"%s","note":"%s"}' \
    "$ts" "$PR_AGENT" "$task" "${pct:-null}" "${eta:-null}" "$eta_ts" "${note//\"/\\\"}")"
  printf '%s\n' "$line" >> "$_PR_DIR/agent-progress.jsonl"
  _pr_pub "b00t.hive.mesh.progress.${task//[^A-Za-z0-9_.-]/_}" "$line"
}

pr_forecast() { # exp_id predicted_secs [service]
  local exp="$1" secs="$2" svc="${3:-forecast.$exp}" ts; ts="$(_pr_iso)"
  printf '{"ts":"%s","kind":"forecast","agent":"%s","exp":"%s","predicted_secs":%s}\n' \
    "$ts" "$PR_AGENT" "$exp" "$secs" >> "$_PR_DIR/forecasts.jsonl"
  _pr_queue_focus b00t-hive "$svc" "$exp" forecast "$secs"
  _pr_pub "b00t.hive.mesh.forecast.${exp//[^A-Za-z0-9_.-]/_}" \
    "{\"ts\":\"$ts\",\"exp\":\"$exp\",\"predicted_secs\":$secs}"
}

pr_settle() { # exp_id actual_secs [service]  — prints accuracy
  local exp="$1" act="$2" svc="${3:-forecast.$exp}" ts pred abs pcterr
  ts="$(_pr_iso)"
  pred="$(grep "\"exp\":\"$exp\"" "$_PR_DIR/forecasts.jsonl" 2>/dev/null | grep '"kind":"forecast"' | tail -1 | sed -n 's/.*"predicted_secs":\([0-9.]*\).*/\1/p')"
  pred="${pred:-0}"
  [ "$pred" = "0" ] && echo "pr_settle: no forecast row for $exp (registered out-of-band?)" >&2
  abs="$(awk "BEGIN{d=$act-$pred; if(d<0)d=-d; printf \"%.0f\", d}")"
  pcterr="$(awk "BEGIN{if($pred>0) printf \"%.1f\", 100*($act-$pred)/$pred; else print \"null\"}")"
  printf '{"ts":"%s","kind":"settle","agent":"%s","exp":"%s","predicted_secs":%s,"actual_secs":%s,"abs_err_secs":%s,"pct_err":%s}\n' \
    "$ts" "$PR_AGENT" "$exp" "$pred" "$act" "$abs" "$pcterr" >> "$_PR_DIR/forecasts.jsonl"
  _pr_queue_focus b00t-hive "$svc" "$exp" actual "$act"
  _pr_pub "b00t.hive.mesh.forecast.${exp//[^A-Za-z0-9_.-]/_}" \
    "{\"ts\":\"$ts\",\"exp\":\"$exp\",\"predicted_secs\":$pred,\"actual_secs\":$act,\"abs_err_secs\":$abs,\"pct_err\":$pcterr}"
  echo "forecast $exp : predicted ${pred}s, actual ${act}s → abs_err ${abs}s (${pcterr}%)"
}
