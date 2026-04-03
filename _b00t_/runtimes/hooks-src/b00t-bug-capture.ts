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
