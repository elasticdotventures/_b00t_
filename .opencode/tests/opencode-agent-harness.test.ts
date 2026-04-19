/**
 * OpenCode Agent Harness Validation Test
 * 
 * Tests that opencode CLI is functional as an agent harness.
 * Run: npx tsx .opencode/tests/opencode-agent-harness.test.ts
 */

import { execSync } from 'child_process';
import { existsSync } from 'fs';
import { join } from 'path';

const OPENCODE_DIR = '/home/brianh/.b00t/.opencode';

interface TestResult {
  name: string;
  pass: boolean;
  message: string;
}

const results: TestResult[] = [];

// Helper to run test
function test(name: string, fn: () => boolean, message: string) {
  try {
    const pass = fn();
    results.push({ name, pass, message });
  } catch (e) {
    results.push({ name, pass: false, message: `Error: ${e}` });
  }
}

// Test 1: opencode CLI exists
test(
  'opencode-cli-exists',
  () => existsSync('/home/brianh/.local/share/pnpm/opencode'),
  'OpenCode CLI binary exists'
);

// Test 2: Version check
test(
  'opencode-version',
  () => {
    const version = execSync('opencode --version', { encoding: 'utf8' }).trim();
    return version.length > 0 && !version.includes('Error');
  },
  'OpenCode version command works'
);

// Test 3: Context directory exists
test(
  'context-directory',
  () => existsSync(join(OPENCODE_DIR, 'context')),
  'Context directory exists'
);

// Test 4: Code quality standards exist
test(
  'code-quality-standards',
  () => existsSync(join(OPENCODE_DIR, 'context/core/standards/code-quality.md')),
  'Code quality standards exist'
);

// Test 5: Agent metadata exists
test(
  'agent-metadata',
  () => existsSync(join(OPENCODE_DIR, 'config/agent-metadata.json')),
  'Agent metadata exists'
);

// Test 6: Core agents defined
test(
  'core-agents',
  () => {
    const agents = execSync('ls /home/brianh/.b00t/.opencode/agent/core/*.md 2>/dev/null | wc -l', { encoding: 'utf8' });
    return parseInt(agents.trim()) >= 1;
  },
  'Core agents defined'
);

// Test 7: Subagents exist
test(
  'subagents',
  () => {
    const subagents = glob(join(OPENCODE_DIR, 'agent/subagents/**/*.md'));
    return subagents.length >= 5;
  },
  'Subagents exist'
);

// Simple glob implementation
function glob(pattern: string): string[] {
  const { execSync } = require('child_process');
  try {
    const result = execSync(`ls -1 ${pattern.replace('*', '').replace('**/', '')} 2>/dev/null || echo ""`, { 
      encoding: 'utf8',
      shell: '/bin/bash'
    });
    return result.split('\n').filter(Boolean);
  } catch {
    return [];
  }
}

// Print results
console.log('\n=== OpenCode Agent Harness Validation ===\n');
let allPass = true;
for (const r of results) {
  console.log(`${r.pass ? '✅' : '❌'} ${r.name}: ${r.message}`);
  if (!r.pass) allPass = false;
}
console.log(`\nTotal: ${results.filter(r => r.pass).length}/${results.length} passed`);
console.log(`Status: ${allPass ? 'PASS' : 'FAIL'}\n`);

process.exit(allPass ? 0 : 1);