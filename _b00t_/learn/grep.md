---
grep: NEVER use grep — always use rg (ripgrep). The grep tool in opencode is an MCP wrapper but bash `grep` is strictly prohibited. AGENTS.md rule: "prefer fdfind over find" implies rg over grep as well. Violation cost: slower searches, missed .gitignore filtering, non-standard regex.
