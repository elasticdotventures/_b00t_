---
parallel subtask dispatch: Use delegate_task with tasks=[...] array for parallel execution of independent workstreams. Serial delegation wastes wall-clock time. Independent concerns (Python packaging, Rust CLI, documentation) should always be parallel. Check delegation.max_concurrent_children before dispatching.

---
agent prompt compression: Reserve ~10% of context for agent operational macros loaded via skills. The agent-operational-macros skill provides Rhai guard recipes, parallel dispatch patterns, token optimization budgets, and model-tier routing (sm0l/ch0nky/frontier). Load via b00t learn agent-operational-macros before complex multi-tool sessions.

---
true-grit swarm OKR: For long-running parallel agent work, declare Objective + measurable Key Results before dispatch. Use b00t task as the source of truth, run b00t hive status before resource-heavy parallelism, direct work bottom-up by dependency order, and require each agent handoff to include artifacts, verification, blockers, and next slice.

---
true-grit anti-cheat: A passing test count is not enough. Guard against agents shelling out to reference tools, implementing fixture-only behavior, or satisfying metadata assertions while missing semantic support. Add or identify one behavioral check that proves the implementation is real.

---
true-grit harness audit: If a parallel swarm reports a sudden broad regression, inspect the test harness, environment, and config before assuming the implementation regressed. Broken harness state can look like product failure.

---
narrow subagents: Use small exploration and verification agents to protect executive context. Return `DONE/FAIL + files + evidence`, not raw logs.

---
guarded cli routing: Canonical external command path is `b00t hive run --strict -- <cmd>`. Raw sed is guarded; prefer `rg -n`, `rg -n -A/-B`, structured parsers, or reviewed recipes.

---
postel dwiw: DWIW means deterministic intent-to-command routing with visible normalization and guarded execution. It does not permit fuzzy silent execution.
---
agent dashboard: Use `b00t agent dashboard --limit N` for context-saving TOON review of configured agents. It reports status, quota fields, last-used evidence, infractions, and malformed input counts so harness failures become deterministic learning signals.
agent cardinal kinds: Keep `agent` as the generic metatype. Use `agent.cli`, `agent.sdk`, `agent.ide.vsix`, and `agent.gui` for concrete carriers; dashboard rows include `kind` and datums SHOULD set `[b00t.agent.traits]` booleans for future Rust macro validation.
