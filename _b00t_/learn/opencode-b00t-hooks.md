---
moto: "opencode-b00t-hooks — exposes b00t capabilities as OpenCode slash commands and agent tools. Thin wrapper: each /b00t-* command delegates to a just recipe or b00t CLI call. Memoizes the bridge between b00t's idiom and OpenCode's plugin surface."

commands: |
  /b00t-review — run adversarial multi-framework review on staged changes
  /b00t-system-normal — pre-flight precriteria gate (stash, merge, branch, submodule)
  /b00t-carmack — energy audit (GPU watts, memoization hits, efficiency ratio)
  /b00t-learn <topic> — load a b00t blessing/skill
  /b00t-task <action> — manage task queue (list, add, next, done)

tools: |
  b00t_review — invoke reviewer gate, return VERDICT
  b00t_system_normal — return {status, checks[], summary}
  b00t_carmack_audit — return EnergyBudget stats
  b00t_learn — load datum into context
  b00t_task_list — show pending tasks

install: |
  npm install opencode-b00t-hooks
  # or local:
  opencode plugin file:///path/to/b00t-hooks/src/server.js

architecture: |
  The plugin is a thin bridge:
  /b00t-review → just pr-validate goal="..."
  /b00t-system-normal → just check-system-normal
  /b00t-carmack → b00t learn john-carmack + energy tracking
  /b00t-learn → b00t learn <topic>
  /b00t-task → b00t task <action>

  This keeps b00t's justfile as the source of truth. The plugin never
  reimplements b00t logic — it delegates everything to just recipes.

# b00t:map v1
# summary: opencode-b00t-hooks — bridge b00t capabilities into OpenCode plugin surface
# tags: opencode, plugin, b00t, bridge, hooks, commands, agent-tools
# tier: ch0nky
# cmds: opencode plugin opencode-b00t-hooks, /b00t-review, /b00t-system-normal
# complexity: 5
