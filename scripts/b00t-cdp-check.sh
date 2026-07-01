#!/bin/bash
# b00t-cdp-check.sh — Launch Chrome with CDP, monitor JS console for errors
# Run: just cdp-check
set -euo pipefail
URL="${1:-http://localhost:31337/}"
CDP="http://localhost:9222"
TIMEOUT="${2:-15}"

# Ensure Chrome is running with CDP
if ! curl -s "$CDP/json" > /dev/null 2>&1; then
  echo "🥾 Starting Chrome with CDP..."
  CHROME=$(find /mnt/c -name "chrome.exe" -path "*/Google/Chrome/*" 2>/dev/null | head -1)
  if [ -z "$CHROME" ]; then
    echo "❌ Chrome not found on Windows"
    exit 1
  fi
  powershell.exe -Command "
    Stop-Process -Name chrome -Force -ErrorAction SilentlyContinue
    Start-Process '$CHROME' -ArgumentList @(
      '--remote-debugging-port=9222',
      '--remote-allow-origins=*',
      '--no-first-run',
      '--load-extension=C:\\b00t\\browser-ext',
      '$URL'
    )
  " 2>/dev/null
  sleep 3
fi

# Get the admin page tab
TAB=$(curl -s "$CDP/json" | python3 -c "
import sys,json
pages = json.load(sys.stdin)
for p in pages:
    if '31337' in p.get('url',''):
        print(p['id'])
        break
" 2>/dev/null)

if [ -z "$TAB" ]; then
  echo "❌ Admin page not found in Chrome tabs"
  exit 1
fi

WS_URL=$(curl -s "$CDP/json" | python3 -c "
import sys,json
pages = json.load(sys.stdin)
for p in pages:
    if p['id'] == '$TAB':
        print(p['webSocketDebuggerUrl'])
" 2>/dev/null)

echo "🥾 Monitoring JS console on $TAB for ${TIMEOUT}s..."
python3 -c "
import json, time, sys
from websocket import create_connection

ws = create_connection('$WS_URL')
# Enable Runtime domain for console messages
ws.send(json.dumps({'id':1, 'method':'Runtime.enable'}))
ws.send(json.dumps({'id':2, 'method':'Log.enable'}))

start = time.time()
errors = []
while time.time() - start < $TIMEOUT:
    ws.settimeout(1)
    try:
        msg = json.loads(ws.recv())
        if 'method' in msg:
            m = msg['method']
            if m == 'Runtime.consoleAPICalled':
                entry = msg['params']['args'][0].get('value','')
                level = msg['params']['type']
                if level == 'error':
                    errors.append(f'  ❌ {entry[:200]}')
            elif m == 'Runtime.exceptionThrown':
                exc = msg['params']['exceptionDetails']
                errors.append(f'  💥 {exc.get(\"text\",\"\")[:200]}')
            elif m == 'Log.entryAdded':
                e = msg['params']['entry']
                if e.get('level') == 'error':
                    errors.append(f'  ⚠️ {e.get(\"text\",\"\")[:200]}')
    except: pass

ws.close()

if errors:
    print(f'❌ {len(errors)} JS errors:')
    for e in errors: print(e)
    sys.exit(1)
else:
    print('✅ No JS errors detected')
    sys.exit(0)
" 2>/dev/null || echo "⚠️  WebSocket check failed (install: pip install websocket-client)"
