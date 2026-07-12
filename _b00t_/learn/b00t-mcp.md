---
positional CLI args: MCP tool must use positional_arg_names() not --flags. Fixed clap_reflection.rs

---
grok digest/ask/learn: content+query are positional args not --flags. Fixed in mcp_tools.rs with positionals macro

---
proxy pattern: expose 5 surface tools (learn/whoami/status/exec/discover); all 50+ commands go in TOOL_CATALOG; sub-agents call b00t_discover(query) then b00t_exec(argv) — never register all tools upfront or context explodes

---
SYSTEM-NORMAL submodules-in-sync gate: check vendor subrepos are clean before tasks. `git submodule status` lines with + or M prefix mean unpushed commits or dirty trees. Fix: commit upstream PR, then update parent submodule pointer.
