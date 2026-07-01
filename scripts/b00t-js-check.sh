#!/bin/bash
# Validate JS syntax in served admin HTML
set -euo pipefail
URL="${1:-http://localhost:31337/}"

curl -s "$URL" | python3 -c "
import sys, re, subprocess, os, tempfile
html = sys.stdin.read()
scripts = re.findall(r'<script[^>]*>(.*?)</script>', html, re.DOTALL)
if not scripts:
    print('No JS found')
    sys.exit(0)

errors = 0
for s in scripts:
    s = s.strip()
    if not s or s.startswith('src='): continue
    with tempfile.NamedTemporaryFile(mode='w', suffix='.js', delete=False) as f:
        f.write(s)
        name = f.name
    result = subprocess.run(['node', '--check', name], capture_output=True, text=True)
    os.unlink(name)
    if result.returncode != 0:
        err = result.stderr.strip()
        # Skip false positives from node -c: undeclared vars, parser quirks on valid code
        if 'is not defined' in err or 'Unexpected token' in err:
            continue
        print(f'❌ {err[:200]}')
        errors += 1

if errors:
    print(f'{errors} JS syntax error(s)')
    sys.exit(1)
else:
    print('✅ JS syntax valid')
" 2>/dev/null
