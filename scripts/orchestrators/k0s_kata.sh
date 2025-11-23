#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-}"
if [[ -z "$MODE" ]]; then
  echo "Usage: $0 <start|stop|check> [inventory]" >&2
  echo "Environment:" >&2
  echo "  K0S_KATA_INVENTORY   Override inventory path (default: ~/.config/b00t/k0s-inventory.yaml)" >&2
  echo "  K0S_KATA_EXTRA_ARGS  Extra args passed to ansible-playbook (e.g. \"-e foo=bar\")" >&2
  echo "  K0S_KATA_PLAYBOOK    Override playbook path (default: ansible/playbooks/k0s_kata.yaml)" >&2
  echo "  K0S_KATA_STOP_PLAYBOOK Override stop playbook (default: ansible/playbooks/k0s_kata_stop.yaml)" >&2
  echo "  B00T_ORCH_LOG_DIR    Where to store orchestrator logs (default: ./logs/orchestrators)" >&2
  exit 1
fi

ROOT="${B00T_REPO_ROOT:-$(git rev-parse --show-toplevel 2>/dev/null || pwd)}"
cd "$ROOT"

INVENTORY="${2:-${K0S_KATA_INVENTORY:-$HOME/.config/b00t/k0s-inventory.yaml}}"
EXTRA_ARGS="${K0S_KATA_EXTRA_ARGS:-}"
PLAYBOOK="${K0S_KATA_PLAYBOOK:-ansible/playbooks/k0s_kata.yaml}"
STOP_PLAYBOOK="${K0S_KATA_STOP_PLAYBOOK:-ansible/playbooks/k0s_kata_stop.yaml}"
LOG_DIR="${B00T_ORCH_LOG_DIR:-$ROOT/logs/orchestrators}"
mkdir -p "$LOG_DIR"
TIMESTAMP="$(date -u +%Y%m%dT%H%M%SZ)"
LOG_FILE="$LOG_DIR/k0s-kata-${MODE}-${TIMESTAMP}.log"

run_cmd() {
  echo "🥾 [k0s-kata:${MODE}] logging to $LOG_FILE"
  {
    echo "=== b00t orchestrator :: k0s-kata :: $MODE @ $TIMESTAMP ==="
    echo "Inventory: $INVENTORY"
    echo "Extra args: ${EXTRA_ARGS:-<none>}"
    echo "Playbook: ${PLAYBOOK}"
    echo "Stop playbook: ${STOP_PLAYBOOK}"
    echo "----------------------------------------------------------------"
    "$@"
  } |& tee "$LOG_FILE"
}

case "$MODE" in
  start)
    run_cmd just ansible-k0s INVENTORY="$INVENTORY" PLAYBOOK="$PLAYBOOK" EXTRA_ARGS="$EXTRA_ARGS"
    ;;
  stop)
    run_cmd just ansible-k0s-stop INVENTORY="$INVENTORY" PLAYBOOK="$STOP_PLAYBOOK" EXTRA_ARGS="$EXTRA_ARGS"
    ;;
  check)
    run_cmd just ansible-k0s-check PLAYBOOK="$PLAYBOOK"
    ;;
  *)
    echo "Unknown mode: $MODE" >&2
    exit 1
    ;;
esac
