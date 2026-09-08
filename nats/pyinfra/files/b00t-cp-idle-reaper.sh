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

# --- 3. dstack has active runs -------------------------------------------
#   `dstack ps -a` on 0.20.28 prints a table; --project comes from
#   $DSTACK_PROJECT. Fail-safe: a non-zero exit => stay up.
runs="$("$DSTACK" ps -a 2>/dev/null)" || stay "dstack ps failed (fail-safe)"
if printf '%s\n' "$runs" | grep -qiE 'provisioning|pending|running|terminating'; then
  stay "dstack has a non-terminal run"
fi

# --- 4. dstack fleet not empty ----------------------------------------------
fleet="$("$DSTACK" fleet 2>/dev/null)" || stay "dstack fleet query failed (fail-safe)"
# Header line always prints; a data row means an instance exists.
if [ "$(printf '%s\n' "$fleet" | sed '1d' | grep -c .)" -gt 0 ]; then
  stay "dstack fleet non-empty"
fi

# --- nothing holding it — power off --------------------------------------
log "POWEROFF: no operator hold, no recent client, no connection, no runs, empty fleet"
sync
exec systemctl poweroff
