#!/usr/bin/env bash
# b00t-cp-waker wake core — start the dstack control-node VM if it is not
# RUNNING, then wait until dstack answers on READINESS_PATH.
#
# Deployment-agnostic: works on Cloud Run (metadata ADC) and on a tailnet host
# (GCP_SA_KEY_FILE). Idempotent — safe to call on every inbound request; a
# no-op when the VM is already up.
#
# Exit: 0 = VM up and dstack ready. 1 = start failed or readiness timed out.
set -euo pipefail

: "${PROJECT_ID:?}" "${CONTROL_ZONE:?}" "${CONTROL_INSTANCE:?}" "${DSTACK_UPSTREAM_ADDR:?}"
READINESS_PATH="${READINESS_PATH:-/healthz}"
START_DEADLINE_SECONDS="${START_DEADLINE_SECONDS:-120}"

log() { printf '%s wake: %s\n' "$(date -u +%FT%TZ)" "$*" >&2; }

# --- access token -----------------------------------------------------------
# Cloud Run (network_mode=public): metadata ADC, no key.
# Tailnet host (network_mode=tailnet): GCP_SA_KEY_FILE -> `gcloud auth
#   print-access-token` with CLOUDSDK_AUTH_CREDENTIAL_FILE_OVERRIDE, or
#   google-auth. Not wired yet — that deployment target is Phase 2.75 TODO.
access_token() {
  if [[ -n "${GCP_SA_KEY_FILE:-}" ]]; then
    CLOUDSDK_AUTH_CREDENTIAL_FILE_OVERRIDE="$GCP_SA_KEY_FILE" \
      gcloud auth print-access-token 2>/dev/null \
      || { log "GCP_SA_KEY_FILE set but gcloud unavailable — wire google-auth (Phase 2.75)"; return 1; }
  else
    curl -sf -H 'Metadata-Flavor: Google' \
      "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token" \
      | python3 -c 'import sys,json;print(json.load(sys.stdin)["access_token"])'
  fi
}

TOKEN="$(access_token)"
API="https://compute.googleapis.com/compute/v1/projects/${PROJECT_ID}/zones/${CONTROL_ZONE}/instances/${CONTROL_INSTANCE}"

status="$(curl -sf -H "Authorization: Bearer ${TOKEN}" "$API" \
  | python3 -c 'import sys,json;print(json.load(sys.stdin).get("status","UNKNOWN"))')"
log "instance status = ${status}"

case "$status" in
  RUNNING) : ;;  # nothing to do; still fall through to the readiness wait
  TERMINATED|STOPPED|SUSPENDED)
    log "issuing instances.start"
    curl -sf -X POST -H "Authorization: Bearer ${TOKEN}" -H 'Content-Length: 0' \
      "${API}/start" >/dev/null
    ;;
  STOPPING|SUSPENDING)
    log "instance is ${status}; waiting for it to settle then retrying"
    sleep 10; exec "$0" "$@"
    ;;
  *)
    log "unexpected status ${status}; attempting start anyway"
    curl -sf -X POST -H "Authorization: Bearer ${TOKEN}" -H 'Content-Length: 0' \
      "${API}/start" >/dev/null || true
    ;;
esac

# --- readiness wait --------------------------------------------------------
deadline=$(( $(date +%s) + START_DEADLINE_SECONDS ))
while (( $(date +%s) < deadline )); do
  if curl -sf -o /dev/null --max-time 3 "http://${DSTACK_UPSTREAM_ADDR}${READINESS_PATH}"; then
    log "dstack ready at ${DSTACK_UPSTREAM_ADDR}${READINESS_PATH}"
    exit 0
  fi
  sleep 3
done

log "readiness timed out after ${START_DEADLINE_SECONDS}s"
exit 1
