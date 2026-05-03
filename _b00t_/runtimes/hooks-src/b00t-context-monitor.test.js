const { test } = require('node:test');
const assert = require('node:assert');
const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const HOOKS_DIR = path.join(__dirname, '../claude/hooks');

test('context-monitor: >35% remaining produces no advisory', () => {
  const session = 'test-' + Date.now();
  const bridge = `/tmp/b00t-ctx-${session}.json`;
  fs.writeFileSync(bridge, JSON.stringify({ remaining_pct: 80 }));
  const out = execSync(`node "${path.join(HOOKS_DIR, 'b00t-context-monitor.js')}" '${JSON.stringify({ session_id: session })}'`,
    { cwd: HOOKS_DIR, encoding: 'utf8' });
  assert.strictEqual(out.trim(), '', 'no output when context is healthy');
  fs.unlinkSync(bridge);
});

test('context-monitor: ≤35% injects WARNING', () => {
  const session = 'test-' + Date.now();
  const bridge = `/tmp/b00t-ctx-${session}.json`;
  fs.writeFileSync(bridge, JSON.stringify({ remaining_pct: 30 }));
  const out = execSync(`node "${path.join(HOOKS_DIR, 'b00t-context-monitor.js')}" '${JSON.stringify({ session_id: session })}'`,
    { cwd: HOOKS_DIR, encoding: 'utf8' });
  const parsed = JSON.parse(out);
  assert.ok(parsed.additionalContext.includes('WARNING'));
  fs.unlinkSync(bridge);
});

test('context-monitor: ≤25% injects CRITICAL', () => {
  const session = 'test-' + Date.now();
  const bridge = `/tmp/b00t-ctx-${session}.json`;
  fs.writeFileSync(bridge, JSON.stringify({ remaining_pct: 20 }));
  const out = execSync(`node "${path.join(HOOKS_DIR, 'b00t-context-monitor.js')}" '${JSON.stringify({ session_id: session })}'`,
    { cwd: HOOKS_DIR, encoding: 'utf8' });
  const parsed = JSON.parse(out);
  assert.ok(parsed.additionalContext.includes('CRITICAL'));
  fs.unlinkSync(bridge);
});
