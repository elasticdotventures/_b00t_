const { test } = require('node:test');
const assert = require('node:assert');
const { execFileSync, spawnSync } = require('child_process');
const fs = require('fs');
const os = require('os');
const path = require('path');

const HOOKS_DIR = path.join(__dirname, '../claude/hooks');
const CACHE_FILE = path.join(os.homedir(), '.b00t', 'cache', 'update-check.json');

test('update-check: fresh cache causes immediate exit with no output', () => {
  // Write a fresh cache entry
  fs.mkdirSync(path.dirname(CACHE_FILE), { recursive: true });
  fs.writeFileSync(CACHE_FILE, JSON.stringify({ checked_at: Date.now(), latest: '0.0.0-test' }));

  const result = spawnSync('node', [path.join(HOOKS_DIR, 'b00t-update-check.js')],
    { timeout: 2000, encoding: 'utf8' });

  assert.strictEqual(result.status, 0, 'exit 0');
  assert.strictEqual(result.stdout.trim(), '', 'no stdout output');
});

test('update-check: always exits 0', () => {
  // Stale/missing cache — will attempt HTTP, but must still exit 0
  // Remove cache to force the slow path
  try { fs.unlinkSync(CACHE_FILE); } catch { /* ok */ }

  const result = spawnSync('node', [path.join(HOOKS_DIR, 'b00t-update-check.js')],
    { timeout: 5000, encoding: 'utf8' });

  assert.strictEqual(result.status, 0, 'exit 0 even on stale cache');
});
