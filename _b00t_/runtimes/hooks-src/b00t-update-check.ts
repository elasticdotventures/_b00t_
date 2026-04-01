// b00t-update-check.ts — SessionStart hook
// Checks for newer b00t-cli version; result cached for 24h

import * as fs from 'fs';
import * as path from 'path';
import * as https from 'https';
import * as os from 'os';

const CACHE_FILE = path.join(os.homedir(), '.b00t', 'cache', 'update-check.json');
const CACHE_TTL_MS = 24 * 60 * 60 * 1000;  // 24h

function isCacheFresh(): boolean {
  try {
    const cache = JSON.parse(fs.readFileSync(CACHE_FILE, 'utf8'));
    return Date.now() - cache.checked_at < CACHE_TTL_MS;
  } catch { return false; }
}

if (isCacheFresh()) { process.exit(0); }

// Async check — fire and forget pattern (don't block agent startup)
const req = https.get('https://api.github.com/repos/promptexecution/b00t/releases/latest', {
  headers: { 'User-Agent': 'b00t-update-check' }
}, (res) => {
  let data = '';
  res.on('data', (chunk) => data += chunk);
  res.on('end', () => {
    try {
      const latest = JSON.parse(data).tag_name?.replace(/^v/, '');
      fs.mkdirSync(path.dirname(CACHE_FILE), { recursive: true });
      fs.writeFileSync(CACHE_FILE, JSON.stringify({ checked_at: Date.now(), latest }));
    } catch { /* non-fatal */ }
  });
});
req.on('error', () => { /* non-fatal */ });
req.setTimeout(3000, () => req.destroy());

// process exits naturally when event loop drains (after HTTP response or timeout)
