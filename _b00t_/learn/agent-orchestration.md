---
parallel subtask dispatch: Use delegate_task with tasks=[...] array for parallel execution of independent workstreams. Serial delegation wastes wall-clock time. Independent concerns (Python packaging, Rust CLI, documentation) should always be parallel. Check delegation.max_concurrent_children before dispatching.

---
agent prompt compression: Reserve ~10% of context for agent operational macros loaded via skills. The agent-operational-macros skill provides Rhai guard recipes, parallel dispatch patterns, token optimization budgets, and model-tier routing (sm0l/ch0nky/frontier). Load via b00t learn agent-operational-macros before complex multi-tool sessions.
