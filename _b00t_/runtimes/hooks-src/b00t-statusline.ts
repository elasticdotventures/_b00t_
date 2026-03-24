// b00t-statusline.ts — statusLine hook
// Writes context bridge file; outputs status line string

import * as fs from 'fs';
import * as path from 'path';
import * as child_process from 'child_process';

const input = JSON.parse(process.argv[2] || '{}');
const sessionId: string = input?.session_id ?? 'unknown';
const model: string = input?.model ?? '?';
const contextTokensUsed: number = input?.context_tokens_used ?? 0;
const contextTokensMax: number = input?.context_tokens_max ?? 200000;

const remainingPct = contextTokensMax > 0
  ? Math.round(((contextTokensMax - contextTokensUsed) / contextTokensMax) * 100)
  : 100;

// Write bridge for context-monitor
const bridgeFile = path.join('/tmp', `b00t-ctx-${sessionId}.json`);
try {
  fs.writeFileSync(bridgeFile, JSON.stringify({ remaining_pct: remainingPct, updated_at: Date.now() }));
} catch { /* non-fatal */ }

// Get b00t version
let b00tVersion = '?';
try {
  b00tVersion = child_process.execSync('b00t-cli --version 2>/dev/null', { timeout: 500 })
    .toString().trim().split(' ').pop() ?? '?';
} catch { /* not installed */ }

const statusLine = `🥾 b00t ${b00tVersion} | ${model} | ctx ${remainingPct}%`;
process.stdout.write(JSON.stringify({ statusLine }));
process.exit(0);
