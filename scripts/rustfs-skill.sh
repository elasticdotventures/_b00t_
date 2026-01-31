#!/usr/bin/env bash
set -euo pipefail
SKILL_NAME="${1:-}"
SKILL_MODE="${2:-start}"
DATA_ROOT="${3:-$HOME/.dotfiles/.rustfs-skill}"
if [[ -z "$SKILL_NAME" ]]; then
  echo "Usage: $0 <skill> [start|stop|status] [data-root]" >&2
  exit 1
fi
SKILL_DIR="$(realpath ./skills/$SKILL_NAME 2>/dev/null || printf '')"
if [[ -z "$SKILL_DIR" ]]; then
  echo "Skill directory not found: skills/$SKILL_NAME" >&2
  exit 1
fi
CONTAINER_NAME="rustfs-${SKILL_NAME}" 
DATA_PATH="$DATA_ROOT/$SKILL_NAME"
function stop_container() {
  docker stop "$CONTAINER_NAME" >/dev/null 2>&1 || true
  docker rm "$CONTAINER_NAME" >/dev/null 2>&1 || true
}
case "$SKILL_MODE" in
  start)
    stop_container
    mkdir -p "$DATA_PATH"
    if [[ -n "${SKILL_BUCKET:-}" ]]; then
      if ! command -v s3fs >/dev/null 2>&1; then
        echo "⚠️  s3fs not installed; SKILL_BUCKET requires s3fs" >&2
        exit 1
      fi
      umount "$DATA_PATH" >/dev/null 2>&1 || true
      s3fs "$SKILL_BUCKET" "$DATA_PATH" -o allow_other
    else
      rsync -a "$SKILL_DIR/" "$DATA_PATH/"
    fi
    docker run -d \
      --name "$CONTAINER_NAME" \
      -p 9500:9000 \
      -p 9501:9001 \
      -e RUSTFS_EXTERNAL_ADDRESS=":9000" \
      -e RUSTFS_CORS_ALLOWED_ORIGINS="http://localhost:9001" \
      -e RUSTFS_ACCESS_KEY="rustfs-$SKILL_NAME" \
      -e RUSTFS_SECRET_KEY="secret-$SKILL_NAME" \
      -v "$DATA_PATH:/data" \
      rustfs/rustfs:latest
    printf "Skill '%s' mounted at %s and running as %s\n" "$SKILL_NAME" "$DATA_PATH" "$CONTAINER_NAME"
    ;;
  stop)
    stop_container
    printf "Stopped RustFS skill container %s\n" "$CONTAINER_NAME"
    ;;
  status)
    docker ps --filter "name=$CONTAINER_NAME"
    ;;
  *)
    echo "Unknown mode: $SKILL_MODE" >&2
    exit 1
    ;;
esac
