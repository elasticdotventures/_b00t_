// _b00t_/runtimes/hooks-src/b00t-bug-capture.test.js
const { test } = require('node:test');
const assert = require('node:assert');
const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');

const HOOKS_DIR = path.join(__dirname, '../claude/hooks');
const HOOK = path.join(HOOKS_DIR, 'b00t-bug-capture.js');

// Shared temp dir — created once, cleaned up at end
const TMP = fs.mkdtempSync(path.join(os.tmpdir(), 'b00t-bug-test-'));
const BUGS_DIR = path.join(TMP, '.bugs');
fs.mkdirSync(BUGS_DIR);

const ENV = { ...process.env, BUGS_DIR_OVERRIDE: BUGS_DIR };

const BASH_FAILURE = JSON.stringify({
  session_id: 'test-session-001',
  tool_name: 'Bash',
  // 🤓 PostToolUseFailure: tool_input (not input.input which is PreToolUse-specific)
  tool_input: { command: 'cargo build --release' },
  tool_response: { error: 'error[E0308]: mismatched types', exit_code: 101 },
});

const NON_BASH_FAILURE = JSON.stringify({
  session_id: 'test-session-002',
  tool_name: 'Write',
  tool_input: { file_path: '/etc/shadow', content: 'x' },
  tool_response: { error: 'permission denied', exit_code: 1 },
});

test('bug-capture: exits 0 on Bash failure (never blocks agent)', () => {
  assert.doesNotThrow(() =>
    execSync(`node "${HOOK}" '${BASH_FAILURE}'`, { encoding: 'utf8', env: ENV })
  );
});

test('bug-capture: outputs valid JSON or empty stdout on Bash failure', () => {
  const out = execSync(`node "${HOOK}" '${BASH_FAILURE}'`, { encoding: 'utf8', env: ENV }).trim();
  if (out) {
    const parsed = JSON.parse(out);
    assert.ok(typeof parsed === 'object', 'output must be JSON object');
  }
});

test('bug-capture: exits 0 on non-Bash failure (no-op, no JSONL written)', () => {
  const before = fs.readdirSync(BUGS_DIR).length;
  execSync(`node "${HOOK}" '${NON_BASH_FAILURE}'`, { encoding: 'utf8', env: ENV });
  const after = fs.readdirSync(BUGS_DIR).length;
  assert.strictEqual(before, after, 'no JSONL written for non-Bash tools');
});

test('bug-capture: writes JSONL entry with required fields', () => {
  execSync(`node "${HOOK}" '${BASH_FAILURE}'`, { encoding: 'utf8', env: ENV });

  const files = fs.readdirSync(BUGS_DIR).filter(f => f.endsWith('.jsonl'));
  assert.ok(files.length >= 1, 'at least one JSONL file created');

  const lines = fs.readFileSync(path.join(BUGS_DIR, files[0]), 'utf8')
    .trim().split('\n').filter(Boolean);
  assert.ok(lines.length >= 1, 'at least one JSONL entry');

  const entry = JSON.parse(lines[lines.length - 1]); // most recent
  assert.strictEqual(entry.tool, 'Bash');
  assert.ok(entry.cmd.includes('cargo'), 'command captured');
  assert.ok(typeof entry.ts === 'string', 'timestamp present');
  assert.strictEqual(typeof entry.exit_code, 'number', 'exit_code is number');
  assert.ok('topic' in entry, 'topic detected');
});

// Cleanup
process.on('exit', () => { try { fs.rmSync(TMP, { recursive: true }); } catch {} });
