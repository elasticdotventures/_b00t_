---
narrow surface for sub-agents: impl_mcp_tool! keeps all tool structs compilable but create_full_mcp_registry() is debug-only; create_mcp_registry() MUST stay at 5 tools — sandboxed sub-agents get b00t_exec + b00t_discover as their only entry points

---
github-mcp(2026-06-17): github-mcp-server npm pkg (jungchihoon) is local git CLI NOT GitHub API. b00t datum split: github.mcp.toml → @modelcontextprotocol/server-github (API). git-local.mcp.toml → github-mcp-server (local git, broken on Node v22).

---
b00t-mcp arg-passing: MCP adapter sends --flag=value but b00t-cli expects positional args (b00t-cli agent delegate <WORKER> <TASK_ID> <DESCRIPTION>). Use bash for b00t-cli commands.
