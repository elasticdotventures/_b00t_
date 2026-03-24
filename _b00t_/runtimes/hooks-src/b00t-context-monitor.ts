// b00t-context-monitor.ts — PostToolUse hook
// Monitors context window usage and injects warnings when running low
// 🤓 Bridge: reads /tmp/b00t-ctx-{session}.json written by statusline hook

import * as fs from 'fs';
import * as path from 'path';

const input = JSON.parse(process.argv[2] || '{}');
const sessionId: string = input?.session_id ?? process.env.CLAUDE_SESSION_ID ?? 'unknown';
const bridgeFile = path.join('/tmp', `b00t-ctx-${sessionId}.json`);

let contextPct = 100;
try {
  const bridge = JSON.parse(fs.readFileSync(bridgeFile, 'utf8'));
  contextPct = bridge.remaining_pct ?? 100;
} catch {
  // No bridge file yet — first tool use
}

let advisory: string | null = null;
if (contextPct <= 25) {
  advisory = `🚨 CONTEXT CRITICAL: Only ${contextPct}% context remaining. Run /compact or finish current task.`;
} else if (contextPct <= 35) {
  advisory = `⚠️ CONTEXT WARNING: ${contextPct}% context remaining. Consider /compact soon.`;
}

if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}

process.exit(0);
