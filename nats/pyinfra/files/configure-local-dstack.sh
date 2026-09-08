#!/usr/bin/env bash
# Verify the local dstack CLI can reach the local server so the idle reaper can
# introspect. `dstack server` on first start auto-writes ~/.dstack/config.yml
# with the "main" project (URL + admin token) — so normally there is nothing to
# do here beyond confirming it. Run by deploy_cp_node.py after
# dstack-server.service is up. Idempotent. Plan gleaming-jingling-nygaard, Phase 3.
set -euo pipefail

DSTACK="${DSTACK_BIN:-$HOME/.local/bin/dstack}"
PROJECT="${DSTACK_PROJECT:-main}"

# Wait for the server.
for _ in $(seq 1 30); do
  curl -sf http://127.0.0.1:3000/ >/dev/null 2>&1 && break
  sleep 2
done

# Already configured (the common case)?
if "$DSTACK" fleet --project "$PROJECT" >/dev/null 2>&1; then
  echo "configure-local-dstack: '$PROJECT' already reachable"
  exit 0
fi

# Fallback: register it from the server config's admin token.
TOKEN="$(python3 - "$HOME/.dstack/server/config.yml" <<'PY'
import sys, yaml
d = yaml.safe_load(open(sys.argv[1])) or {}
for k in ("admin_token", "token"):
    if d.get(k):
        print(d[k]); break
else:
    print((d.get("default_permissions") or {}).get("admin_token", ""))
PY
)"
if [ -z "$TOKEN" ]; then
  echo "configure-local-dstack: '$PROJECT' not reachable and no admin token found — check dstack-server.service" >&2
  exit 1
fi
"$DSTACK" project add "$PROJECT" --url http://127.0.0.1:3000 --token "$TOKEN" 2>/dev/null || \
  "$DSTACK" project add --name "$PROJECT" --url http://127.0.0.1:3000 --token "$TOKEN" || true
echo "configure-local-dstack: registered '$PROJECT' -> http://127.0.0.1:3000"
