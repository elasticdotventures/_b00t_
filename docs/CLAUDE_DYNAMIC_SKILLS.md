# Claude Dynamic Skills Strategy

Another branch under development will expose *role-aware skills* to the Claude desktop plugin by mounting only the relevant documentation tree, datums, and `lfmf` entries into lightweight Linux c-containers / namespaces. The goal is to keep each Claude agent focused: an engineer sees the Matlab datum when working on Matlab, while a Python-focused coder automatically gets `uv`/Python reference materials.

## Execution plan
1. Detect the active role (e.g., `claude` view sees `role=matlab`). Use `b00t status` or the MCP context bundle to discover the desired capabilities.
2. Spawn a private Linux namespace (via the upcoming c-containers service) that bind-mounts only the folders required for that role: `skills/matlab`, `learn/matlab`, `lfmf/matlab`, etc.
3. Inject the namespace into the Claude plugin by pointing `CLAUDE_SKILLS_ROOT` (or a similar env) so the correct datums resolve through the plugin's `/agents` docs.
4. Document each role in `docs/CLAUDE_SKILL_ROLES.md` (not yet created) and list which datums or `b00t` commands they should run.
5. Use `b00t stack ansible` or `b00t stack list` to ensure the necessary datums are installed before starting the containerized role.

This architecture keeps the memory footprint low, improves security, and makes `b00t` the single source of truth for what each Claude agent can see and recommend.
