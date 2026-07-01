#!/bin/bash
# b00t-playwright-test.sh — Headless browser test for admin UI
# Requires: npm install playwright (one-time)  
# Run: just playwright-test
set -euo pipefail
URL="${1:-http://localhost:31337/}"

# One-time playwright install
if ! npx playwright --version 2>/dev/null; then
    echo "🥾 Installing playwright..."
    npm install -g playwright 2>/dev/null
    npx playwright install chromium 2>/dev/null || true
fi

echo "🥾 Headless browser test: $URL"

node -e "
const { chromium } = require('playwright');
(async () => {
    const errors = [];
    const browser = await chromium.launch({ headless: true });
    const context = await browser.newContext();
    const page = await context.newPage();
    
    page.on('console', msg => {
        if (msg.type() === 'error') errors.push(msg.text());
    });
    page.on('pageerror', err => errors.push(err.message));
    
    await page.goto('$URL', { waitUntil: 'networkidle', timeout: 15000 });
    await page.waitForTimeout(2000); // Let JS initialize
    
    // Check key elements
    for (const sel of ['#sidebar-version', '#viz-select', '#heartbeat', '#section-viz']) {
        const el = await page.$(sel);
        if (!el) errors.push('Missing element: ' + sel);
    }
    
    // Check version display updates
    const sidebarVer = await page.textContent('#sidebar-version');
    if (!sidebarVer || sidebarVer.trim() === '🥾') {
        errors.push('Version not populated: "' + sidebarVer + '"');
    }
    
    await browser.close();
    
    if (errors.length) {
        errors.forEach(e => console.error('❌', e));
        process.exit(1);
    }
    console.log('✅ All checks passed');
})().catch(e => { console.error('❌', e.message); process.exit(1); });
" 2>/dev/null
