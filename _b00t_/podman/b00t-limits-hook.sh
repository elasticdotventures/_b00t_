#!/usr/bin/env bash
# b00t-limits OCI prestart hook — reject containers without a memory cap.
# 🤓 Shared-node protocol (sm3lly, 2026-07-18): uncapped container workloads
#    crashed the node twice on 2026-07-17. This hook is the hard enforcement
#    layer beneath the b00t rhai guards (which only see `b00t sh`/`b00t run`).
# 🤓 Escape hatch: `podman run --annotation b00t.unlimited=ack ...` bypasses,
#    so resident services can opt out EXPLICITLY and auditable-y.
# Install (operator, root):
#   sudo install -m755 b00t-limits-hook.sh /usr/local/bin/b00t-limits-hook
#   sudo install -m644 b00t-limits-hook.json /usr/share/containers/oci/hooks.d/
# ⚠️ Test with a throwaway container BEFORE trusting a session to it; a broken
#    always-hook takes down ALL container starts (see oci-nvidia-hook/kube-play).

set -euo pipefail
# OCI state arrives on stdin: {"bundle": "/path/to/bundle", ...}
state="$(cat)"
bundle="$(printf '%s' "$state" | python3 -c 'import json,sys; print(json.load(sys.stdin)["bundle"])')"
cfg="$bundle/config.json"
[ -r "$cfg" ] || exit 0  # cannot inspect — do not block the world

python3 - "$cfg" <<'PY'
import json, sys
cfg = json.load(open(sys.argv[1]))
ann = cfg.get("annotations") or {}
if ann.get("b00t.unlimited") == "ack":
    sys.exit(0)
limit = ((cfg.get("linux") or {}).get("resources") or {}).get("memory", {}).get("limit")
if not limit or limit <= 0:
    sys.stderr.write(
        "b00t-limits: container has no memory cap. Shared node (sm3lly) protocol: "
        "add --memory/--memory-swap, or opt out explicitly with "
        "--annotation b00t.unlimited=ack\n"
    )
    sys.exit(1)
PY
