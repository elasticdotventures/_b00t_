#!/bin/bash
set -euo pipefail
kill $(pgrep -f "target/debug/b00t-admin") 2>/dev/null || true
sleep 1
nohup /home/brianh/.dotfiles/target/debug/b00t-admin > /tmp/b00t-admin.log 2>&1 &
sleep 2
curl -s http://localhost:31337/api/admin/health | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'v{d[\"version\"]} git={d[\"git\"][:8]}')" 2>/dev/null || echo "FAILED"
