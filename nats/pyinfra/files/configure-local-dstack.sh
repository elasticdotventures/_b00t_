#!/usr/bin/env bash
# Point the local dstack CLI at the local server so the idle reaper can
# introspect runs/fleet. Run by deploy_cp_node.py after dstack-server.service
# is up. Idempotent. Plan gleaming-jingling-nygaard, Phase 3.
set -euo pipefail

DSTACK="${DSTACK_BIN:-$HOME/.local/bin/dstack}"
PROJECT="${DSTACK_PROJECT:-b00t}"
SERVER_CONFIG="$HOME/.dstack/server/config.yml"

# Wait for the server to finish first-start token generation.
for _ in $(seq 1 30); do
  curl -sf http://127.0.0.1:3000/ >/dev/null 2>&1 && break
  sleep 2
done

# The admin token is written into the server config on first start. Key name
# has moved across dstack versions — try the known spellings.
TOKEN="$(python3 - "$SERVER_CONFIG" <<'PY'
import sys, yaml
d = yaml.safe_load(open(sys.argv[1])) or {}
for k in ("admin_token", "token"):
    if d.get(k):
        print(d[k]); break
else:
    dp = d.get("default_permissions") or {}
    print(dp.get("admin_token", ""))
PY
)"

if [ -z "$TOKEN" ]; then
  echo "configure-local-dstack: no admin token in $SERVER_CONFIG yet — skipping CLI project config" >&2
  exit 0
fi

"$DSTACK" project add --name "$PROJECT" --url http://127.0.0.1:3000 --token "$TOKEN" --yes 2>/dev/null || \
  "$DSTACK" project add --name "$PROJECT" --url http://127.0.0.1:3000 --token "$TOKEN" || true
"$DSTACK" project set-default "$PROJECT" || true
echo "configure-local-dstack: CLI project '$PROJECT' -> http://127.0.0.1:3000"
