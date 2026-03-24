// b00t-datum-guard.ts — PreToolUse hook
// Intercepts direct package manager invocations and soft-redirects to b00t cli install <datum>
// 🤓 NEVER exit non-zero — always advisory only (additionalContext)

const input = JSON.parse(process.argv[2] || '{}');
const command: string = input?.input?.command ?? '';

const PACKAGE_MANAGER_PATTERNS = [
  { regex: /^\s*pip\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*npm\s+install\s+-g\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*apt(-get)?\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*brew\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
  { regex: /^\s*cargo\s+install\b/, hint: 'b00t cli install <datum-name>.cli' },
];

let advisory: string | null = null;
for (const { regex, hint } of PACKAGE_MANAGER_PATTERNS) {
  if (regex.test(command)) {
    advisory = `⚠️ b00t datum-guard: prefer \`${hint}\` over direct package managers.\nCheck available datums with \`b00t cli desires\`.\nDirect install will work but won't be tracked in the b00t hive.`;
    break;
  }
}

if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}

process.exit(0);  // ALWAYS exit 0 — never block
