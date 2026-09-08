#!/usr/bin/env bash
# Mount the object-storage-backed build cache at $CACHE_MOUNT.
# Plan gleaming-jingling-nygaard, Phase 5. Called from
# dev-env/b00t-build.dev-environment.yaml `init:` on every boot/resume.
#
# Pluggable backend (perf-driven, reversible) via $CACHE_BACKEND:
#   zerofs  (default) — Barre/zerofs, encrypted POSIX over object storage,
#                       NFS + NBD. Prefer the native kernel client, not FUSE.
#   rustfs            — rustfs/rustfs, S3-API gateway over the GCS XML endpoint.
#   gcsfuse           — simplest, FUSE, weakest small-file perf (sccache-only).
#
# Ends with a bounded wait so `init:` cannot report "ready" against a cold
# cache. Exit 0 = mounted; non-zero = give up (dev-env init fails loudly).
set -uo pipefail

CACHE_BACKEND="${CACHE_BACKEND:-zerofs}"
CACHE_BUCKET="${CACHE_BUCKET:?set CACHE_BUCKET}"
CACHE_MOUNT="${CACHE_MOUNT:-/mnt/cache}"
CACHE_PREFIX="${CACHE_PREFIX:-zerofs/}"
NBD_DEV="${NBD_DEV:-/dev/nbd0}"

log() { printf '%s cache-up: %s\n' "$(date -u +%FT%TZ)" "$*" >&2; }

if mountpoint -q "$CACHE_MOUNT"; then
  log "$CACHE_MOUNT already mounted — nothing to do"
  exit 0
fi
sudo mkdir -p "$CACHE_MOUNT"

case "$CACHE_BACKEND" in
  zerofs)
    : "${CACHE_ENCRYPTION_PASSWORD:?zerofs needs CACHE_ENCRYPTION_PASSWORD (dstack secret cache_password)}"
    # Creds: ambient ADC from the build-VM metadata SA (b00t-build-vm,
    # storage.objectAdmin on the bucket — granted in b00t-tf Phase 2).
    export AWS_ENDPOINT_URL="https://storage.googleapis.com"
    export ZEROFS_ENCRYPTION_PASSWORD="$CACHE_ENCRYPTION_PASSWORD"
    # ⚠️ verify the exact zerofs invocation against the pinned image
    # (`zerofs --help`): it exposes NFS + NBD, not native FUSE. NBD path:
    log "starting zerofs (nbd) against gs://${CACHE_BUCKET}/${CACHE_PREFIX}"
    sudo modprobe nbd 2>/dev/null || true
    nohup zerofs nbd "s3://${CACHE_BUCKET}/${CACHE_PREFIX}" >/var/log/zerofs.log 2>&1 &
    for i in $(seq 1 30); do
      sudo nbd-client 127.0.0.1 10809 "$NBD_DEV" 2>/dev/null && break
      sleep 2
    done
    sudo blkid "$NBD_DEV" >/dev/null 2>&1 || sudo mkfs.ext4 -F -q "$NBD_DEV"
    sudo mount "$NBD_DEV" "$CACHE_MOUNT"
    ;;
  rustfs)
    log "rustfs backend — TODO: wire rustfs client against the GCS S3 endpoint"
    exit 2
    ;;
  gcsfuse)
    log "gcsfuse backend (FUSE; weak small-file perf — sccache-only)"
    command -v gcsfuse >/dev/null || { log "gcsfuse not installed"; exit 2; }
    gcsfuse --implicit-dirs "$CACHE_BUCKET" "$CACHE_MOUNT"
    ;;
  *)
    log "unknown CACHE_BACKEND=$CACHE_BACKEND"
    exit 2
    ;;
esac

sudo chown "$(id -u):$(id -g)" "$CACHE_MOUNT" || true

# --- bounded wait: init: must not proceed to cargo against a cold cache ---
for i in $(seq 1 60); do
  mountpoint -q "$CACHE_MOUNT" && { log "REMOUNTED-OK $CACHE_MOUNT"; exit 0; }
  sleep 2
done
log "FATAL: $CACHE_MOUNT never became a mountpoint"
exit 1
