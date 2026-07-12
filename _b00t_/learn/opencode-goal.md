---
moto: "opencode-goal-plugin (willytop8) — session-scoped /goal command for OpenCode. Sets a goal, auto-continues on idle, survives session compaction, stops on [goal:complete]+[goal:evidence] or [goal:blocked] markers. Analogous to Claude Code's /goal and Codex goal mode."

canonical: "willytop8/OpenCode-goal-plugin is the canonical implementation (71 commits, 136★, JS). forked to PromptExecution/opencode-goal-plugin-b00t. prevalentWare/opencode-goal-plugin (85★, TS/Bun) is a derivative that credits willytop8's hardening ideas."

features: "Auto-continue on session.idle with safety limits (turns, minutes, tokens, no-progress detection). Budget wrap-up at 80% token threshold. Compaction survival via deterministic summary injection. Multi-goal support with focus/background. Ordered (sisyphus) sequences. Evidence-based completion — [goal:complete] only honored with preceding [goal:evidence] line. Persisted to project-local .opencode/goals/state.json with append-only lifecycle ledger."

integration: "For b00t: the goal plugin pattern maps to DARED acceptance criteria (evidence-based completion) and Carmack memoization (deterministic compaction summaries). A b00t agent using OpenCode benefits from /goal for long-running autonomous tasks with safety limits."

errata: "Multiple implementations exist (willytop8, prevalentWare, others on GitHub). Polyseme — same concept, different npm packages, different feature sets. willytop8 canonical per operator designation."

# b00t:map v1
# summary: opencode-goal-plugin — session-scoped goal mode for OpenCode (willytop8, canonical, JS, 136★)
# tags: opencode, goal, plugin, auto-continue, session, safety-limits, evidence, polyseme
# tier: ch0nky
# cmds: npm install opencode-goal-plugin, /goal <objective>
# complexity: 6
