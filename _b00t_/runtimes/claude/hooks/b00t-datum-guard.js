"use strict";

// b00t-datum-guard.ts
var input = JSON.parse(process.argv[2] || "{}");
var command = input?.input?.command ?? "";
var PACKAGE_MANAGER_PATTERNS = [
  { regex: /^\s*pip\s+install\b/, hint: "b00t cli install <datum-name>.cli" },
  { regex: /^\s*npm\s+install\s+-g\b/, hint: "b00t cli install <datum-name>.cli" },
  { regex: /^\s*apt(-get)?\s+install\b/, hint: "b00t cli install <datum-name>.cli" },
  { regex: /^\s*brew\s+install\b/, hint: "b00t cli install <datum-name>.cli" },
  { regex: /^\s*cargo\s+install\b/, hint: "b00t cli install <datum-name>.cli" }
];
var advisory = null;
for (const { regex, hint } of PACKAGE_MANAGER_PATTERNS) {
  if (regex.test(command)) {
    advisory = `\u26A0\uFE0F b00t datum-guard: prefer \`${hint}\` over direct package managers.
Check available datums with \`b00t cli desires\`.
Direct install will work but won't be tracked in the b00t hive.`;
    break;
  }
}
if (advisory) {
  process.stdout.write(JSON.stringify({ additionalContext: advisory }));
}
process.exit(0);
