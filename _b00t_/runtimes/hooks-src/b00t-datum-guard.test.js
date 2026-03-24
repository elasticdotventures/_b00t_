const { test } = require('node:test');
const assert = require('node:assert');
const { execSync } = require('child_process');
const path = require('path');
const HOOKS_DIR = path.join(__dirname, '../claude/hooks');

function runHook(input) {
  try {
    return execSync(`node "${path.join(HOOKS_DIR, 'b00t-datum-guard.js')}" '${JSON.stringify(input)}'`,
      { cwd: HOOKS_DIR, timeout: 2000, encoding: 'utf8' });
  } catch (e) { return e.stdout ?? ''; }
}

test('datum-guard: pip install triggers advisory', () => {
  const out = runHook({ input: { command: 'pip install requests' } });
  assert.ok(out.includes('b00t cli install'), 'advisory must mention b00t cli install');
});

test('datum-guard: non-package-manager command produces no output', () => {
  const out = runHook({ input: { command: 'ls -la' } });
  assert.strictEqual(out.trim(), '', 'no output for safe commands');
});

test('datum-guard: apt install triggers advisory', () => {
  const out = runHook({ input: { command: 'apt install curl' } });
  assert.ok(out.includes('b00t'));
});

test('datum-guard: always exits 0', () => {
  // execSync throws on non-zero exit; if we get here, exit was 0
  execSync(`node "${path.join(HOOKS_DIR, 'b00t-datum-guard.js')}" '{"input":{"command":"pip install foo"}}'`,
    { cwd: HOOKS_DIR, timeout: 2000 });
});
