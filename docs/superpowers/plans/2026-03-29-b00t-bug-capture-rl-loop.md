# b00t Bug Capture + RL Loop Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `PostToolUseFailure` hook that captures failed Bash commands to a local JSONL file, queries `b00t grok ask` for ontology-based alternatives, injects the suggestion back into agent context, and provides the scaffolding for a reinforcement-learning loop where agents explain failures and b00t updates skills.

**Architecture:** New TypeScript hook (`b00t-bug-capture.ts`) follows the existing hooks-src pattern: TypeScript source → esbuild bundle via `just build-hooks` → distributed to all 5 runtime dirs. Hook writes JSONL to `${BUGS_DIR_OVERRIDE}` (tests) → `${cwd}/.bugs/` (if dir exists) → `~/.b00t/bugs/` (always created). Synchronous `b00t grok ask` call (2s timeout, shell-safe argv form) provides ontology suggestion returned as `additionalContext`. `settings_fragment.json` gains a `PostToolUseFailure` entry.

**Tech Stack:** TypeScript, Node.js 18+, esbuild (via `just build-hooks`), `node:test`, `b00t grok ask` CLI

> 🤓 **Schema note:** PostToolUseFailure delivers `tool_input.command` (same as PostToolUse), NOT `input.input.command` which is the PreToolUse-specific wrapper. `b00t-datum-guard.ts` uses `input?.input?.command` because it is a PreToolUse hook. Different event types, different payload shapes.

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `_b00t_/runtimes/hooks-src/b00t-bug-capture.ts` | Hook source: parse failure, detect topic, write JSONL, query grok |
| Create | `_b00t_/runtimes/hooks-src/b00t-bug-capture.test.js` | Node test runner tests for the built hook |
| Modify | `_b00t_/runtimes/hooks-src/build.js` | Add `'b00t-bug-capture'` to `HOOKS` array |
| Modify | `_b00t_/runtimes/claude/settings_fragment.json` | Add `PostToolUseFailure` entry (full file replace — see Task 3) |
| Modify | `b00t-cli/src/install/runtimes/claude.rs` | Add `_note` to uninstall key list + new test |
| Auto | `_b00t_/runtimes/*/hooks/b00t-bug-capture.js` | Built + distributed by `just build-hooks` (all 5 runtimes) |

---

## Task 1: Write the failing test (TDD — red first)

**Files:**
- Create: `_b00t_/runtimes/hooks-src/b00t-bug-capture.test.js`

> ⚠️ All tests set `BUGS_DIR_OVERRIDE` to a temp dir to prevent side-effects on `~/.b00t/bugs/`.

- [ ] **Step 1: Write the test file**

```js
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
```

- [ ] **Step 2: Commit the test (red — will fail until hook is built)**

```bash
git add _b00t_/runtimes/hooks-src/b00t-bug-capture.test.js
git commit -m "test(hooks): b00t-bug-capture — 4 tests, red (hook not yet built)"
```

- [ ] **Step 3: Run tests — confirm they fail**

```bash
cd /home/brianh/.dotfiles
node --test _b00t_/runtimes/hooks-src/b00t-bug-capture.test.js 2>&1 | head -15
```

Expected: `Error` or `FAIL` — `b00t-bug-capture.js` does not exist yet.

---

## Task 2: Implement the hook source

**Files:**
- Create: `_b00t_/runtimes/hooks-src/b00t-bug-capture.ts`

- [ ] **Step 1: Write the TypeScript source**

```typescript
// b00t-bug-capture.ts — PostToolUseFailure hook
// Captures failed Bash commands to .bugs/YYYY-MM-DD.jsonl
// Queries b00t grok ask for ontology-based alternatives (shell-safe argv form)
// 🤓 PostToolUseFailure payload: tool_input.command (NOT input.input.command — that is PreToolUse)
// 🤓 NEVER exit non-zero — advisory only, never blocks agent

import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';
import * as child_process from 'child_process';

const input = JSON.parse(process.argv[2] || '{}');
const toolName: string = input?.tool_name ?? '';

// Only handle Bash failures
if (toolName !== 'Bash') { process.exit(0); }

const sessionId: string = (input?.session_id ?? 'unknown').replace(/[^a-zA-Z0-9_]/g, '_');
const cmd: string = input?.tool_input?.command ?? '';
const errorMsg: string = String(input?.tool_response?.error ?? '');
const exitCode: number = Number(input?.tool_response?.exit_code ?? -1); // coerce string→number
const ts: string = new Date().toISOString();

// Topic detection: map first command token to known b00t grok topics
const TOPIC_MAP: Record<string, string> = {
  cargo: 'rust', rustc: 'rust',
  pip: 'python', python: 'python', python3: 'python', uv: 'python',
  npm: 'typescript', node: 'typescript', npx: 'typescript', tsc: 'typescript',
  git: 'git', gh: 'git',
  docker: 'docker', podman: 'docker',
  kubectl: 'k8s', helm: 'k8s',
  just: 'just',
};
const firstToken = cmd.trim().split(/\s+/)[0] ?? '';
const topic: string = TOPIC_MAP[firstToken] ?? 'bash';

// Query b00t grok via execFileSync (argv array — no shell injection)
// 🚩 execSync with shell interpolation was intentionally avoided here
let grokSuggestion: string | null = null;
try {
  const truncatedCmd = cmd.slice(0, 200);
  const result = child_process.execFileSync(
    'b00t', ['grok', 'ask', truncatedCmd, '-t', topic],
    { timeout: 2000, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] }
  ).trim();
  if (result) grokSuggestion = result.slice(0, 500);
} catch { /* non-fatal — grok may not be running */ }

// Resolve bugs directory: env override (tests) → cwd/.bugs (if exists) → ~/.b00t/bugs
function resolveBugsDir(): string | null {
  if (process.env.BUGS_DIR_OVERRIDE) return process.env.BUGS_DIR_OVERRIDE;
  const cwdBugs = path.join(process.cwd(), '.bugs');
  if (fs.existsSync(cwdBugs)) return cwdBugs;
  const homeBugs = path.join(os.homedir(), '.b00t', 'bugs');
  try { fs.mkdirSync(homeBugs, { recursive: true }); return homeBugs; } catch { return null; }
}

const bugsDir = resolveBugsDir();
if (bugsDir) {
  const dateStr = ts.slice(0, 10); // YYYY-MM-DD
  const logFile = path.join(bugsDir, `${dateStr}.jsonl`);
  const entry = JSON.stringify({
    ts, session_id: sessionId, tool: toolName, cmd,
    exit_code: exitCode,
    error: errorMsg.slice(0, 300),
    topic,
    grok_suggestion: grokSuggestion,
  });
  try { fs.appendFileSync(logFile, entry + '\n'); } catch { /* non-fatal */ }
}

if (grokSuggestion) {
  process.stdout.write(JSON.stringify({
    additionalContext: `🔍 b00t grok (${topic}): ${grokSuggestion}`,
  }));
}

process.exit(0);
```

---

## Task 3: Wire into build and settings, then build

**Files:**
- Modify: `_b00t_/runtimes/hooks-src/build.js`
- Modify: `_b00t_/runtimes/claude/settings_fragment.json` (full file replace — see below)

- [ ] **Step 1: Add hook to build.js HOOKS array**

Change lines 5–10 of `_b00t_/runtimes/hooks-src/build.js`:
```js
const HOOKS = [
  'b00t-statusline',
  'b00t-update-check',
  'b00t-context-monitor',
  'b00t-datum-guard',
  'b00t-bug-capture',   // ← add this line
];
```

- [ ] **Step 2: Replace settings_fragment.json**

> ⚠️ Replace the ENTIRE file content — do not surgically insert. This is the canonical source read by `register_hooks` in `claude.rs`.

```json
{
  "_note": "AUTO-GENERATED by b00t-cli install — DO NOT EDIT. Source: _b00t_/runtimes/hooks-src/",
  "hooks": {
    "SessionStart": [{"matcher": "", "hooks": [{"type": "command", "command": "node {{HOOKS_DIR}}/b00t-update-check.js"}]}],
    "PostToolUse": [{"matcher": "Bash|Edit|Write|Agent|Task", "hooks": [{"type": "command", "command": "node {{HOOKS_DIR}}/b00t-context-monitor.js"}]}],
    "PostToolUseFailure": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "node {{HOOKS_DIR}}/b00t-bug-capture.js"}]}],
    "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "node {{HOOKS_DIR}}/b00t-datum-guard.js"}]}]
  },
  "statusLine": {"command": "node {{HOOKS_DIR}}/b00t-statusline.js"}
}
```

- [ ] **Step 3: Build all hooks**

```bash
cd /home/brianh/.dotfiles
just build-hooks
```

Expected:
```
✅ Built b00t-statusline.js
✅ Built b00t-update-check.js
✅ Built b00t-context-monitor.js
✅ Built b00t-datum-guard.js
✅ Built b00t-bug-capture.js
🥾 All hooks built and distributed.
```

- [ ] **Step 4: Verify banner in built file**

```bash
head -3 _b00t_/runtimes/claude/hooks/b00t-bug-capture.js
```

Expected:
```
// AUTO-GENERATED by build-hooks — DO NOT EDIT
// Source: _b00t_/runtimes/hooks-src/b00t-bug-capture.ts
// Rebuild: just build-hooks
```

- [ ] **Step 5: Commit source + build artifacts**

```bash
git add \
  _b00t_/runtimes/hooks-src/b00t-bug-capture.ts \
  _b00t_/runtimes/hooks-src/build.js \
  _b00t_/runtimes/claude/settings_fragment.json \
  _b00t_/runtimes/codex/hooks/b00t-bug-capture.js \
  _b00t_/runtimes/copilot/hooks/b00t-bug-capture.js \
  _b00t_/runtimes/gemini/hooks/b00t-bug-capture.js \
  _b00t_/runtimes/opencode/hooks/b00t-bug-capture.js \
  _b00t_/runtimes/claude/hooks/b00t-bug-capture.js
git commit -m "feat(hooks): b00t-bug-capture PostToolUseFailure — JSONL capture + grok RL"
```

---

## Task 4: Run tests — green

- [ ] **Step 1: Run test suite**

```bash
cd /home/brianh/.dotfiles
node --test _b00t_/runtimes/hooks-src/b00t-bug-capture.test.js 2>&1
```

Expected:
```
✔ bug-capture: exits 0 on Bash failure (never blocks agent)
✔ bug-capture: outputs valid JSON or empty stdout on Bash failure
✔ bug-capture: exits 0 on non-Bash failure (no-op, no JSONL written)
✔ bug-capture: writes JSONL entry with required fields
ℹ tests 4
ℹ pass 4
ℹ fail 0
```

- [ ] **Step 2: Smoke-test directly (no grok — expect silent exit)**

```bash
BUGS_DIR_OVERRIDE=/tmp node _b00t_/runtimes/claude/hooks/b00t-bug-capture.js \
  '{"session_id":"smoke","tool_name":"Bash","tool_input":{"command":"cargo build"},"tool_response":{"error":"no Cargo.toml","exit_code":101}}'
echo "exit: $?"
```

Expected: no output (grok not running in this shell), `exit: 0`, JSONL file created in `/tmp/`.

---

## Task 5: Update installer uninstall + add test

**Files:**
- Modify: `b00t-cli/src/install/runtimes/claude.rs`

- [ ] **Step 1: Add `_note` to uninstall key list (line ~103)**

```rust
// before:
remove_from_json_settings(block_path, &["hooks", "statusLine"])?;
// after:
remove_from_json_settings(block_path, &["hooks", "statusLine", "_note"])?;
```

- [ ] **Step 2: Add test for `_note` removal**

In the `#[cfg(test)]` block, add after `test_remove_from_json_settings_deletes_keys`:

```rust
#[test]
fn test_uninstall_removes_note_key() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("settings.json");
    std::fs::write(&file, r#"{"_note":"DO NOT EDIT","hooks":{},"keep":true}"#).unwrap();

    remove_from_json_settings(&file, &["hooks", "statusLine", "_note"]).unwrap();

    let content = std::fs::read_to_string(&file).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("_note").is_none(), "_note must be removed on uninstall");
    assert!(parsed.get("keep").is_some(), "unrelated keys preserved");
}
```

- [ ] **Step 3: Run claude unit tests — expect 7 passing**

```bash
cargo test -p b00t-cli --lib runtimes::claude 2>&1 | tail -10
```

Expected: `test result: ok. 7 passed; 0 failed`

- [ ] **Step 4: Commit**

```bash
git add b00t-cli/src/install/runtimes/claude.rs
git commit -m "fix(installer): uninstall also removes _note key; add test"
```

---

## Task 6: Push and verify PR

- [ ] **Step 1: Push**

```bash
git push origin feat/claude-marketplace-skill-bundles
```

- [ ] **Step 2: Confirm PR #328 includes all new commits**

```bash
gh pr view 328 --json commits --jq '.commits[].messageHeadline'
```

Expected to include:
```
test(hooks): b00t-bug-capture — 4 tests, red (hook not yet built)
feat(hooks): b00t-bug-capture PostToolUseFailure — JSONL capture + grok RL
fix(installer): uninstall also removes _note key; add test
```

---

## RL Loop — How It Compounds

```
Bash cmd fails
  → b00t-bug-capture hook fires (PostToolUseFailure)
  → writes .bugs/YYYY-MM-DD.jsonl {ts, cmd, error, topic, grok_suggestion}
  → if grok has answer → injects as additionalContext (agent sees next turn)
  → agent acts on suggestion → b00t lfmf datum abstract "<lesson>"
  → operator reviews .bugs/ at session end → prunes resolved
  → next b00t learn <topic> includes distilled lesson
```

<!-- b00t:map v1
summary: b00t-bug-capture hook — PostToolUseFailure, JSONL, grok RL loop, no shell injection
tags: bug-capture, rl-loop, PostToolUseFailure, grok, lfmf, operator, hooks
tier: ch0nky
cmds: just build-hooks, node --test b00t-bug-capture.test.js, cargo test -p b00t-cli --lib runtimes::claude
complexity: 5
-->
