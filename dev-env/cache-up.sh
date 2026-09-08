#!/usr/bin/env bash
# Mount the object-storage build cache at $CACHE_MOUNT. Plan
# gleaming-jingling-nygaard, Phase 5. Shipped onto the box via the dev-env
# `files:` mapping and called from `init:` before any cargo.
#
# What lives on the mount: ONLY $SCCACHE_DIR (sccache blobs — whole
# compilation outputs, write-once, gcsfuse-friendly). NOT target/ or the git
# checkout — those stay on local disk (measured 2026-09-08: target/ on gcsfuse
# is ~15-20x slower from the small-file fsync pattern). sccache-over-GCS is
# what makes a rebuild after an idle-stop warm.
#
# NON-FATAL by design: if no object backend can be mounted, bind a LOCAL dir
# at $CACHE_MOUNT and warn — the build still works, sccache just won't persist
# past an idle-stop. Exit 0 either way; exit 1 only on an internal error.
#
# $CACHE_BACKEND: gcsfuse (default, VERIFIED) | zerofs | rustfs
set -uo pipefail

CACHE_BACKEND="${CACHE_BACKEND:-gcsfuse}"
CACHE_BUCKET="${CACHE_BUCKET:?set CACHE_BUCKET}"
CACHE_MOUNT="${CACHE_MOUNT:-/mnt/cache}"
CACHE_PREFIX="${CACHE_PREFIX:-}"
NBD_DEV="${NBD_DEV:-/dev/nbd0}"

log() { printf '%s cache-up: %s\n' "$(date -u +%FT%TZ)" "$*" >&2; }

sudo mkdir -p "$CACHE_MOUNT"
if mountpoint -q "$CACHE_MOUNT"; then log "$CACHE_MOUNT already mounted"; exit 0; fi

fallback_local() {
  log "⚠️  object cache unavailable ($1) — binding a LOCAL dir at $CACHE_MOUNT."
  log "⚠️  the build works but sccache will NOT survive an idle-stop (no warm rebuild)."
  sudo mkdir -p /var/lib/b00t-cache
  sudo mount --bind /var/lib/b00t-cache "$CACHE_MOUNT"
  sudo chown -R "$(id -u):$(id -g)" "$CACHE_MOUNT" 2>/dev/null || true
  exit 0
}

install_gcsfuse() {
  command -v gcsfuse >/dev/null && return 0
  log "installing gcsfuse"
  export GCSFUSE_REPO=gcsfuse-"$(lsb_release -c -s 2>/dev/null || echo noble)"
  echo "deb https://packages.cloud.google.com/apt $GCSFUSE_REPO main" | sudo tee /etc/apt/sources.list.d/gcsfuse.list >/dev/null
  curl -fsSL https://packages.cloud.google.com/apt/doc/apt-key.gpg | sudo gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg 2>/dev/null
  sudo sed -i 's| main| main|; s|deb |deb [signed-by=/usr/share/keyrings/cloud.google.gpg] |' /etc/apt/sources.list.d/gcsfuse.list
  sudo apt-get update -qq && sudo apt-get install -y -qq gcsfuse
}

case "$CACHE_BACKEND" in
  gcsfuse)
    install_gcsfuse || fallback_local "gcsfuse install failed"
    # Creds: ambient ADC from the build-VM metadata SA (b00t-build-vm has
    # storage.objectAdmin on the bucket, granted in b00t-tf Phase 2).
    log "gcsfuse gs://${CACHE_BUCKET} -> $CACHE_MOUNT"
    gcsfuse --implicit-dirs --file-mode=0644 --dir-mode=0755 \
      "$CACHE_BUCKET" "$CACHE_MOUNT" || fallback_local "gcsfuse mount failed"
    ;;
  zerofs)
    : "${CACHE_PASSWORD:?zerofs needs CACHE_PASSWORD (dstack secret cache_password)}"
    command -v zerofs >/dev/null || {
      log "installing zerofs"
      . "$HOME/.cargo/env" 2>/dev/null || true
      cargo install zerofs --locked 2>/dev/null || fallback_local "zerofs install failed"
    }
    export AWS_ENDPOINT_URL="https://storage.googleapis.com"
    export ZEROFS_ENCRYPTION_PASSWORD="$CACHE_PASSWORD"
    # ⚠️ NOT yet verified against a real image — see the runbook. NBD path:
    sudo modprobe nbd 2>/dev/null || true
    nohup zerofs nbd "s3://${CACHE_BUCKET}/${CACHE_PREFIX:-zerofs/}" >/var/log/zerofs.log 2>&1 &
    for _ in $(seq 1 30); do sudo nbd-client 127.0.0.1 10809 "$NBD_DEV" 2>/dev/null && break; sleep 2; done
    sudo blkid "$NBD_DEV" >/dev/null 2>&1 || sudo mkfs.ext4 -F -q "$NBD_DEV"
    sudo mount "$NBD_DEV" "$CACHE_MOUNT" || fallback_local "zerofs mount failed"
    ;;
  rustfs)
    log "rustfs backend not wired yet"; fallback_local "rustfs TODO"
    ;;
  *)
    fallback_local "unknown CACHE_BACKEND=$CACHE_BACKEND"
    ;;
esac

sudo chown -R "$(id -u):$(id -g)" "$CACHE_MOUNT" 2>/dev/null || true
mountpoint -q "$CACHE_MOUNT" && { log "REMOUNTED-OK $CACHE_MOUNT ($CACHE_BACKEND)"; exit 0; }
fallback_local "post-mount check failed"
