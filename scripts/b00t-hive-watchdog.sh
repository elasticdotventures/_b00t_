#!/usr/bin/env bash
# b00t-hive-watchdog — the hive Sentinel.
#
# Census of every hive systemd --user service each cycle; breaks crash-loops by
# stopping any unit that restarts too fast; heralds findings on the NATS bus.
#
# Env knobs:
#   WATCHDOG_INTERVAL_SECS   loop interval           (default 15)
#   WATCHDOG_ONESHOT=1       run one cycle, exit 0
#   WATCHDOG_MAX_RESTARTS    trip threshold          (default 5)
#   WATCHDOG_WINDOW_SECS     trip window             (default 120)
#   WATCHDOG_EXTRA_UNITS     extra space-separated unit names to watch (test hook)
#   WATCHDOG_SELF_UNIT       own unit name, never stopped (default b00t-hive-watchdog.service)
#   WATCHDOG_NO_NATS=1       skip NATS publishing
#
# State: ${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/b00t-watchdog.state  (unit<TAB>nrestarts<TAB>first_seen_epoch)
# Log:   <repo>/.b00t/hive-watchdog.jsonl                            (one JSON object per line)
set -u

INTERVAL_SECS="${WATCHDOG_INTERVAL_SECS:-15}"
MAX_RESTARTS="${WATCHDOG_MAX_RESTARTS:-5}"
WINDOW_SECS="${WATCHDOG_WINDOW_SECS:-120}"
SELF_UNIT="${WATCHDOG_SELF_UNIT:-b00t-hive-watchdog.service}"
EXTRA_UNITS="${WATCHDOG_EXTRA_UNITS:-}"

REPO_ROOT="${B00T_REPO_ROOT:-/home/brianh/.b00t}"
LOG_FILE="${REPO_ROOT}/.b00t/hive-watchdog.jsonl"
STATE_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
STATE_FILE="${STATE_DIR}/b00t-watchdog.state"
NATS_BIN="/home/brianh/.local/bin/nats"
NATS_ENV="${HOME}/.b00t/secrets/hive-nats.env"

mkdir -p "${REPO_ROOT}/.b00t" 2>/dev/null || true
: > "${STATE_FILE}.tmp" 2>/dev/null || true
touch "$STATE_FILE" 2>/dev/null || true

now_epoch() { date +%s; }
now_iso()   { date -Is; }

# json_line KEY VAL KEY VAL ... -> emits {"ts":"...", ...} to stdout (string values only; ints pass through if numeric)
json_line() {
  local out='{"ts":"'"$(now_iso)"'"' k v
  while [ "$#" -ge 2 ]; do
    k="$1"; v="$2"; shift 2
    if printf '%s' "$v" | grep -Eq '^-?[0-9]+$'; then
      out="${out},\"${k}\":${v}"
    else
      v="${v//\\/\\\\}"; v="${v//\"/\\\"}"
      out="${out},\"${k}\":\"${v}\""
    fi
  done
  printf '%s}\n' "$out"
}

log_event() { json_line "$@" | tee -a "$LOG_FILE"; }

# List hive units: b00t-hive-*.service + b00t@*.service (+ EXTRA_UNITS test hook).
list_units() {
  systemctl --user list-units --all --type=service --no-legend --plain 'b00t-hive-*.service' 'b00t@*.service' 2>/dev/null \
    | awk '{print $1}' | grep -E '\.service$' || true
  for u in $EXTRA_UNITS; do printf '%s\n' "$u"; done
}

unit_prop() { systemctl --user show -p "$2" --value "$1" 2>/dev/null; }

state_get() { grep -P "^\Q$1\E\t" "$STATE_FILE" 2>/dev/null | head -1; }

publish_nats() {
  [ "${WATCHDOG_NO_NATS:-0}" = "1" ] && return 0
  [ -x "$NATS_BIN" ] || return 0
  [ -r "$NATS_ENV" ] || return 0
  local subject="$1" payload="$2"
  # shellcheck disable=SC1090
  ( set -a; . "$NATS_ENV"; set +a
    "$NATS_BIN" pub \
      --user "${HIVE_NATS_USER:-}" --password "${HIVE_NATS_PASSWORD:-}" \
      --server "${NATS_URL:-nats://localhost:4222}" \
      "$subject" "$payload" ) >/dev/null 2>&1 \
    || echo "watchdog: nats pub $subject failed (ignored)" >&2
}

run_cycle() {
  local epoch tripped=0 units n=0
  epoch="$(now_epoch)"
  : > "${STATE_FILE}.tmp"
  units="$(list_units)"
  [ -z "$units" ] && { log_event event census units 0 note "no hive units found" >/dev/null; }

  while IFS= read -r unit; do
    [ -n "$unit" ] || continue
    n=$((n + 1))
    local nr active sub ems prev prev_nr prev_seen delta
    nr="$(unit_prop "$unit" NRestarts)";       nr="${nr:-0}"
    active="$(unit_prop "$unit" ActiveState)"; active="${active:-unknown}"
    sub="$(unit_prop "$unit" SubState)";       sub="${sub:-unknown}"
    ems="$(unit_prop "$unit" ExecMainStatus)"; ems="${ems:-0}"
    printf '%s\t%s\t%s\t%s\t%s\n' "$epoch" "$unit" "$nr" "$active" "$sub" >/dev/null

    log_event unit "$unit" NRestarts "$nr" ActiveState "$active" SubState "$sub" ExecMainStatus "$ems" >/dev/null

    prev="$(state_get "$unit")"
    if [ -n "$prev" ]; then
      prev_nr="$(printf '%s' "$prev" | cut -f2)"
      prev_seen="$(printf '%s' "$prev" | cut -f3)"
    else
      prev_nr="$nr"; prev_seen="$epoch"
    fi
    printf '%s\t%s\t%s\n' "$prev_nr" "$nr" "$prev_seen" >/dev/null
    [ -z "$prev_nr" ] && prev_nr="$nr"
    [ -z "$prev_seen" ] && prev_seen="$epoch"

    delta=$(( nr - prev_nr ))
    local age=$(( epoch - prev_seen ))
    # keep the window anchor; reset it once the window elapses without a trip
    if [ "$age" -ge "$WINDOW_SECS" ]; then
      prev_nr="$nr"; prev_seen="$epoch"; delta=0
    fi

    if [ "$delta" -ge "$MAX_RESTARTS" ] && [ "$unit" != "$SELF_UNIT" ]; then
      tripped=$((tripped + 1))
      systemctl --user stop "$unit" >/dev/null 2>&1
      log_event event crashloop unit "$unit" delta "$delta" window_secs "$WINDOW_SECS" action stopped
      publish_nats "b00t.hive.mesh.health.crashloop" \
        "$(json_line event crashloop unit "$unit" delta "$delta" window_secs "$WINDOW_SECS" action stopped)"
      # reset anchor after acting
      prev_nr="$nr"; prev_seen="$epoch"
    fi

    printf '%s\t%s\t%s\n' "$unit" "$prev_nr" "$prev_seen" >> "${STATE_FILE}.tmp"
  done <<EOF
$units
EOF

  mv "${STATE_FILE}.tmp" "$STATE_FILE" 2>/dev/null || true
  publish_nats "b00t.hive.mesh.health.snapshot" \
    "$(json_line event snapshot units "$n" tripped "$tripped")"
  return 0
}

if [ "${WATCHDOG_ONESHOT:-0}" = "1" ]; then
  run_cycle
  exit 0
fi

echo "b00t-hive-watchdog: interval=${INTERVAL_SECS}s trip=${MAX_RESTARTS}/${WINDOW_SECS}s self=${SELF_UNIT}" >&2
while true; do
  run_cycle
  sleep "$INTERVAL_SECS"
done
