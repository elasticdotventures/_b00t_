---
opencode-config: OpenCode 0.15.3 rejects nested permission.edit/write maps in agent markdown frontmatter; use scalar ask/allow/deny permissions and project-root opencode.json for local provider registration.

---
mcp-config-target: OpenCode MCP config is under opencode.json key mcp; local servers use {type:'local', command:[...], enabled:true, environment:{...}}, remote servers use {type:'remote', url:'...', enabled:true}. Installed opencode 0.15.3 exposes only interactive opencode mcp add, so deterministic b00t installs should edit opencode.json directly.
