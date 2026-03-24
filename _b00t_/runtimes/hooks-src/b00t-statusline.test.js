const { test } = require('node:test');
const assert = require('node:assert');
const { execSync } = require('child_process');
const path = require('path');
const HOOKS_DIR = path.join(__dirname, '../claude/hooks');

test('statusline: output is valid JSON with statusLine string', () => {
  const out = execSync(`node "${path.join(HOOKS_DIR, 'b00t-statusline.js')}" '${JSON.stringify({ session_id: 'test', model: 'claude-test', context_tokens_used: 50000, context_tokens_max: 200000 })}'`,
    { cwd: HOOKS_DIR, encoding: 'utf8' });
  const parsed = JSON.parse(out);
  assert.ok(typeof parsed.statusLine === 'string', 'statusLine must be a string');
  assert.ok(parsed.statusLine.includes('%'), 'statusLine must include context percentage');
});
