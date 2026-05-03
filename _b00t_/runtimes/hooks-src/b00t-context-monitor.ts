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

/// Find git repo root by traversing upward for .git directory
function findGitRoot(cwd: string): string | null {
  let dir = cwd;
  for (let i = 0; i < 20; i++) { // max 20 levels up
    const gitDir = path.join(dir, '.git');
    if (fs.existsSync(gitDir) && fs.statSync(gitDir).isDirectory()) {
      return dir;
    }
    const parent = path.dirname(dir);
    if (parent === dir) break; // reached filesystem root
    dir = parent;
  }
  return null;
}

/// Read use_count for each capability from .git/_b00t_.toml
/// Returns Vec<(capability_name, use_count)>
function getCapabilityUseCounts(gitRoot: string): Array<{ name: string; useCount: number }> {
  const tomlPath = path.join(gitRoot, '.git', '_b00t_.toml');
  if (!fs.existsSync(tomlPath)) {
    return [];
  }

  const content = fs.readFileSync(tomlPath, 'utf8');
  const capabilities: Array<{ name: string; useCount: number }> = [];

  // Parse [numbers] section for capability:NAME:use_count keys
  const numbersMatch = content.match(/\[numbers\]([\s\S]*?)(?:# |\n\[|$)/);
  if (!numbersMatch) return [];

  const numbersSection = numbersMatch[1];
  const capRegex = /^capability:([^:]+):use_count\s*=\s*(\d+)/gm;
  let match;
  while ((match = capRegex.exec(numbersSection)) !== null) {
    const name = match[1];
    const useCount = parseInt(match[2], 10);
    if (useCount > 0) {
      capabilities.push({ name, useCount });
    }
  }

  // Sort by use_count ascending (lowest first)
  capabilities.sort((a, b) => a.useCount - b.useCount);
  return capabilities;
}

/// Get unloading suggestions for bottom 3 capabilities by use_count
function suggestUnloading(capabilities: Array<{ name: string; useCount: number }>): string | null {
  if (capabilities.length < 1) return null;

  // Get bottom 3 (or all if less than 3)
  const bottom = capabilities.slice(0, Math.min(3, capabilities.length));
  if (bottom.length === 0) return null;

  const suggestions = bottom
    .filter(c => c.useCount > 0)
    .map(c => `${c.name} (used ${c.useCount})`)
    .join(', ');

  if (!suggestions) return null;
  return suggestions;
}

// Main logic
const gitRoot = findGitRoot(process.cwd());
const capabilities = gitRoot ? getCapabilityUseCounts(gitRoot) : [];
const unloadSuggestion = contextPct <= 35 && capabilities.length > 0
  ? suggestUnloading(capabilities)
  : null;

let advisory: string | null = null;
if (contextPct <= 25) {
  const critical = 'CRITICAL';
  advisory = `🚨 ${critical} CONTEXT: Only ${contextPct}% remaining.`;
  if (unloadSuggestion) {
    advisory += ` Consider unloading: ${unloadSuggestion}`;
  } else {
    advisory += ' Run /compact or finish task.';
  }
} else if (contextPct <= 35) {
  advisory = `⚠️ CONTEXT: ${contextPct}% remaining.`;
  if (unloadSuggestion) {
    advisory += ` Consider unloading: ${unloadSuggestion}`;
  }
}

if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}

process.exit(0);
