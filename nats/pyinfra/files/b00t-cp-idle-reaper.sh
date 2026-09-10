#!/usr/bin/env bash
# b00t control-node idle reaper. Plan gleaming-jingling-nygaard, Phase 2.5.
#
# Fired every 5 min by dstack-idle-reaper.timer. Powers the VM off ONLY when
# dstack demonstrably has no work AND no client has touched it recently.
# Fail-safe: any uncertainty (a query errors, output unparseable) => STAY UP.
#
# The pingap waker starts the box again on the next inbound request; run/job
# state persists in ~/.dstack on the boot disk across poweroff.
set -uo pipefail

IDLE_GRACE_MIN="${IDLE_GRACE_MIN:-45}"                 # keep > fleet idle_duration (30m)
DSTACK="${DSTACK_BIN:-/home/brianh/.local/bin/dstack}"
# dstack reads --project from $DSTACK_PROJECT natively. "main" is the project
# the server auto-creates locally (see ~/.dstack/config.yml); it just works.
export DSTACK_PROJECT="${DSTACK_PROJECT:-main}"
KEEPALIVE_FILE="/run/b00t-cp-keepalive"                 # waker touches this per proxied request
HOLD_FILE="/run/b00t-cp-hold"                           # `just remote-keepalive` sets this
LOG_TAG="cp-reaper"

log() { logger -t "$LOG_TAG" -- "$*"; printf '%s %s: %s\n' "$(date -u +%FT%TZ)" "$LOG_TAG" "$*" >&2; }
stay() { log "HOLD: $1"; exit 0; }

# --- 0. explicit operator hold ------------------------------------------------
[ -e "$HOLD_FILE" ] && stay "operator hold ($HOLD_FILE present)"

# --- 1. recent client activity ---------------------------------------------
if [ -f "$KEEPALIVE_FILE" ]; then
  if [ "$(find "$KEEPALIVE_FILE" -mmin "-${IDLE_GRACE_MIN}" 2>/dev/null)" ]; then
    stay "keepalive younger than ${IDLE_GRACE_MIN}m"
  fi
fi

# --- 2. an established client connection on :3000 -------------------------
if ss -Htn state established '( sport = :3000 )' 2>/dev/null | grep -q .; then
  stay "established connection on :3000"
fi

# --- 3. dstack has a non-terminal run ----------------------------------
#   Parse `dstack ps --format json` and HOLD only when a run's status is
#   non-terminal. `dstack ps` (plain, no -a) STILL lists finished runs for
#   a retention window — a prior "any row => hold" check pinned the node
#   up for 6h+ after a `failed` partition (2026-09-10). And an all-status-
#   word allow-list missed the no-capacity retry state and powered off
#   mid-`dstack apply` (run 34419799571 -> CLI 503). Terminal set =
#   done|failed|terminated|aborted; everything else (submitted, pending,
#   provisioning, running, terminating, pulling, retrying) => real work.
#   Fail-safe: query error or unparseable JSON => stay up.
runs="$("$DSTACK" ps --format json 2>/dev/null)" || stay "dstack ps failed (fail-safe)"
live="$(printf '%s' "$runs" | jq -r '
    [.runs[]?.status]
    | map(select(. != "done" and . != "failed" and . != "terminated" and . != "aborted"))
    | length' 2>/dev/null)"
case "${live:-x}" in
  x|"") stay "dstack ps JSON unparseable (fail-safe)" ;;
  0)    : ;;
  *)    stay "dstack has ${live} non-terminal run(s)" ;;
esac

# --- 4. dstack fleet has a LIVE instance ----------------------------------
#   0.21.x prints one row per fleet plus an `instance=N` row per node. A
#   `kubernetes`-backend fleet (kept up as a standing prereq for k8s runs)
#   shows its instance `terminated` forever — it provisions no VM and costs
#   nothing, so it must NOT pin the control node up. Only an instance in a
#   live state counts. Fail-safe: a query error, or fleet rows present with
#   no recognizable `instance=` line, => stay up.
fleet="$("$DSTACK" fleet 2>/dev/null)" || stay "dstack fleet query failed (fail-safe)"
if [ "$(printf '%s\n' "$fleet" | sed '1d' | grep -c .)" -gt 0 ]; then
  inst_rows="$(printf '%s\n' "$fleet" | grep -cE '^[[:space:]]+instance=')"
  live_rows="$(printf '%s\n' "$fleet" | grep -E '^[[:space:]]+instance=' \
                | grep -cviE 'terminated|failed')"
  if [ "$inst_rows" -eq 0 ] || [ "$live_rows" -gt 0 ]; then
    stay "dstack fleet has a live (or unrecognized) instance"
  fi
  log "fleet present but all ${inst_rows} instance row(s) terminated/failed — not a hold"
fi

# --- nothing holding it — power off --------------------------------------
log "POWEROFF: no operator hold, no recent client, no connection, no runs, empty fleet"
sync
exec systemctl poweroff
