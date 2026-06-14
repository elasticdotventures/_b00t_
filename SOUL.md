# b00t SOUL

## 2026-06-14 True Grit x Knowledge Catalog Fork

Objective: make b00t agent dynamics forkable as catalog knowledge, not just local prompt text.

Sources:
- GitButler True Grit: <https://blog.gitbutler.com/true-grit>
- GoogleCloudPlatform knowledge-catalog: <https://github.com/GoogleCloudPlatform/knowledge-catalog>
- OKF proof-of-concept: <https://github.com/GoogleCloudPlatform/knowledge-catalog/tree/main/okf>

Catalog frame:
- Knowledge Catalog exposes tools, agents, and samples for context management, enrichment, and retrieval.
- OKF represents portable knowledge as markdown plus frontmatter, versioned in git, readable by humans and agents.
- b00t SOUL.md is the local fork root for agent operating knowledge; catalog exports SHOULD preserve links, provenance, OKRs, verification, and handoff state.
- Postel/DWIW belongs in the catalog as deterministic routing from intent to known guarded commands, not fuzzy silent execution.

True Grit integration OKR:
- Objective: keep long-running b00t swarms directed, measurable, resource-aware, and hard to game.
- KR1: AGENTS.md contains objective/key-result, baseline, directed-work, anti-cheat, resource-gate, handoff, cost, and harness-audit rules.
- KR2: MULTI_AGENT_GOSPEL.md contains captain/crew swarm discipline.
- KR3: `_b00t_/learn/agent-orchestration.md` carries reusable true-grit swarm lessons.
- KR4: `_b00t_/hive-guards.hive.toml` guards raw sed usage, recommends `rg`, and escalates repeated violations.
- KR5: guard JSONL records can carry `panopticon`, `aberrant`, and `abberant` tags for repeat violations.
- KR6: exploration and verification use narrow subagents with compressed handoff back to executive context.
- KR7: `b00t agent dashboard` exposes online/configured status, quota tracking, last-use evidence, and infraction score in TOON/JSON form for executive review.
- KR8: Agent datum names are disambiguated by cardinal runtime kind: `agent.cli`, `agent.sdk`, `agent.ide.vsix`, `agent.gui`, with root `agent` retained as the trait metatype.

Current state:
- Branch: `fix/hive-guard-test-logic`
- b00t task `#21`: True Grit agent dynamics integration.
- b00t task `#22`: raw sed guard and CLI-through-b00t guardrail.
- Pre-existing dirty files were not reverted.
- `just -l` fails because `vendor/irontology-mcp/irontology.just` is unavailable.

Handoff:
- Run targeted Rust tests for `test_guard_raw_sed_recommends_rg` and shipped guard expression coverage.
- Use `b00t hive run --strict -- <command>` for external CLI guard checks.
- Do not invoke raw sed; use `rg -n`, `rg -n -A/-B`, or structured parsers.

# b00t:map v1
# summary: Forkable SOUL state for True Grit agent dynamics and Knowledge Catalog/OKF-style context artifacts
# tags: true-grit, knowledge-catalog, okf, soul, guards, rg, panopticon, okr
# tier: frontier
# cmds: b00t task list, b00t hive status, b00t hive run --strict -- rg -n PATTERN PATH
# complexity: 7
