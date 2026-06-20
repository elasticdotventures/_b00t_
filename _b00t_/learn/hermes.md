---
acp-adapter: ACP transport is stdio JSON-RPC subprocess. Replace Copilot binary with b00t adapter routing to local GPU (vllm/ollama). Onboard via fzf menu at install.

---
onboarding: b00t-hermes integration has 9 hook interfaces (terminal, ACP, context, MCP, skills, memory, cron, inference, security). Wizard at scripts/hermes-onboarding-wizard.sh uses fzf menus with default=ask/backup. Run via: bash setup.sh or b00t setup hermes-integration.

---
subagent verification: sub-agents return SELF-REPORTED results that need independent verification. Check file creation, git state, and test output yourself. Sub-agents find bugs you're blind to (2 Critical shell-injection bugs found in this session alone) but also hallucinate completions.

---
MCP audit gap: Cloudflare MCP surface (Workers/Pages/D1/R2/KV) does NOT exist in local config. 50 MCP configs found, 9 broken. If CF edge deployment tooling is needed, wrangler or a CF MCP server must be configured — this is a deployment gap not a code gap.

---
bash-to-rust transition timing: prototype rapidly in bash (≤200 lines), but plan the Rust rewrite BEFORE the bash PoC grows legs. This session lost 3 turns fixing bash string interpolation when the correct answer was Rust from the start. Write the plan file, then delegate implementation.
