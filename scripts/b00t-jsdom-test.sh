#!/bin/bash
# b00t-jsdom-test.sh — Execute served inline JS in a DOM context, check for runtime errors
# Requires: npm install jsdom (one-time)
set -euo pipefail
URL="${1:-http://localhost:31337/}"

# One-time jsdom install
if ! node -e "require('jsdom')" 2>/dev/null; then
    echo "🥾 Installing jsdom..."
    npm install -g jsdom 2>/dev/null || true
fi

curl -s "$URL" | node -e "
const { JSDOM } = require('jsdom');
const fs = require('fs');

// Read HTML from stdin
const chunks = [];
process.stdin.on('data', c => chunks.push(c));
process.stdin.on('end', () => {
    const html = Buffer.concat(chunks).toString();
    
    // Create DOM with all the globals a browser would have
    const dom = new JSDOM(html, {
        url: '$URL',
        runScripts: 'dangerously',
        resources: 'usable',
        pretendToBeVisual: true,
    });
    
    const { window } = dom;
    global.window = window;
    global.document = window.document;
    
    // Collect errors
    const errors = [];
    window.onerror = (msg) => errors.push(String(msg));
    
    // Wait for scripts to execute, then report
    setTimeout(() => {
        if (errors.length) {
            errors.forEach(e => console.error('❌', e));
            process.exit(1);
        }
        console.log('✅ JS executed without runtime errors');
        
        // Verify key functions exist
        for (const fn of ['toggleSection', 'beat', 'renderMermaid', 'onVizSelect']) {
            if (typeof window[fn] !== 'function') {
                console.error('❌ Missing function:', fn);
                process.exit(1);
            }
        }
        console.log('✅ All key functions defined');
        process.exit(0);
    }, 1000);
});
" 2>/dev/null
