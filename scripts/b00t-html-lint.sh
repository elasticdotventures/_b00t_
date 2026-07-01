#!/bin/bash
# b00t-html-lint.sh — validate served admin HTML for common issues
# Run: just lint-html
set -euo pipefail
URL="${1:-http://localhost:31337/}"
ERRORS=0

HTML=$(curl -s "$URL" 2>/dev/null)
if [ -z "$HTML" ]; then
    echo "❌ Server not reachable at $URL"
    exit 1
fi

echo "🥾 Linting HTML from $URL"

# 1. Merge conflict markers
if echo "$HTML" | grep -q '<<<<<<<\|=======\|>>>>>>>'; then
    echo "  ❌ Merge conflict markers found in HTML"
    ERRORS=$((ERRORS+1))
fi

# 2. Unclosed tags
python3 -c "
import sys, re
html = sys.stdin.read()
# Check critical unclosed tags
tags = ['script', 'style', 'div', 'span', 'g', 'svg']
for tag in tags:
    opens = len(re.findall(f'<{tag}[\\s>]', html))
    closes = len(re.findall(f'</{tag}>', html))
    if opens != closes:
        print(f'  ⚠️  {tag}: {opens} opens, {closes} closes')
" <<< "$HTML" 2>/dev/null

# 3. Missing CDN scripts
if ! echo "$HTML" | grep -q 'mermaid.min.js'; then
    echo "  ⚠️  mermaid CDN missing"
fi
if ! echo "$HTML" | grep -q 'cytoscape.min.js'; then
    echo "  ⚠️  cytoscape CDN missing"
fi

# 4. Hardcoded version mismatch
API_VER=$(curl -s "$URL/api/admin/health" 2>/dev/null | python3 -c "import sys,json; print(json.load(sys.stdin).get('version','?'))" 2>/dev/null || echo "?")
HTML_VER=$(echo "$HTML" | grep -oP 'v\d+\.\d+\.\d+' | head -1 | tr -d 'v')
if [ "$API_VER" != "?" ] && [ "$HTML_VER" != "$API_VER" ]; then
    echo "  ⚠️  Version mismatch: HTML=$HTML_VER API=$API_VER"
fi

# 5. JS syntax check
bash "${0%/*}/b00t-js-check.sh" "$URL" 2>/dev/null || ERRORS=$((ERRORS+1))

# 6. Check for common JS template errors (unconverted {{ or }})
if echo "$HTML" | grep -P '\{\{(?!\s)' | grep -v 'dummy' > /dev/null 2>&1; then
    echo "  ⚠️  Possible unconverted Rust template braces {{ }} in output"
fi

# 7. Responsive viewport
if ! echo "$HTML" | grep -q 'viewport'; then
    echo "  ⚠️  Missing viewport meta tag"
fi

if [ $ERRORS -eq 0 ]; then
    echo "✅ HTML lint passed"
else
    echo "❌ $ERRORS lint issue(s)"
    exit 1
fi
