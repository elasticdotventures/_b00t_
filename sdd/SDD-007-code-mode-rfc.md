# SDD-007: b00t-mcp Code Mode — `search()` + `execute()` Consolidation

## Status
Draft — awaiting review

## Context

b00t-mcp currently exposes **~40+ individual MCP tools** (`b00t_mcp_list`, `b00t_cli_detect`, `b00t_agent_discover`, `b00t_grok_ask`, `b00t_task_add`, …) as flat, first-class tools in the MCP protocol. Each tool carries its own JSON schema, description, and execution path. This flat enumeration causes three concrete problems:

1. **Token bloat** — Every tool definition consumes context window in the LLM system prompt. With 40+ tools, tool descriptions alone can exceed 4k tokens.
2. **Selection friction** — The LLM must reason across a large namespace to pick the right tool. Error rate scales with namespace size.
3. **Schema drift** — Adding a new b00t-cli subcommand requires adding a new CLAP struct + `impl_mcp_tool!` + registry entry + test. The surface area grows linearly.

The proxy subsystem (`proxy_mcp_tools.rs`) already solves this for *external* MCP servers via `proxy_discover` + `proxy_execute`. We propose applying the same pattern to *internal* b00t tools.

## Goal

Consolidate the flat b00t-mcp tool surface into **two primary tools**:

| Tool | Purpose |
|------|---------|
| `b00t_search` | Discover available b00t commands by keyword, category, or tag |
| `b00t_execute` | Run any discovered command by name with a JSON parameter object |

All existing CLAP-derived commands remain intact as the backing registry. Only the **MCP exposure layer** changes.

## Design

### 1. Registry Duality

`McpCommandRegistry` already separates *tool metadata* (`get_tools()`) from *execution* (`execute(name, params)`). We exploit this:

```
┌─────────────────────────────────────────────┐
│           MCP Protocol Layer                │
│  ┌─────────────┐      ┌─────────────────┐   │
│  │ b00t_search │      │ b00t_execute    │   │
│  └──────┬──────┘      └────────┬────────┘   │
│         │                      │             │
│         └──────────┬───────────┘             │
│                    ▼                         │
│         ┌─────────────────────┐              │
│         │ McpCommandRegistry  │              │
│         │ (existing)          │              │
│         └──────────┬──────────┘              │
│                    ▼                         │
│    ┌──────────────────────────────┐          │
│    │ CLAP-derived commands        │          │
│    │ (McpReflection + McpExecutor)│          │
│    └──────────────────────────────┘          │
└─────────────────────────────────────────────┘
```

### 2. Tool Schemas

#### `b00t_search`

```json
{
  "name": "b00t_search",
  "description": "Search the b00t command registry for available commands. Returns command names, descriptions, categories, and JSON schemas.",
  "input_schema": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "Keyword to search across command names, descriptions, and tags"
      },
      "category": {
        "type": "string",
        "description": "Filter by category: mcp, cli, agent, grok, task, session, tutorial, ontology, acp"
      },
      "limit": {
        "type": "integer",
        "default": 10,
        "description": "Maximum results to return"
      }
    },
    "required": ["query"]
  }
}
```

Response is a ranked list:

```json
{
  "results": [
    {
      "name": "b00t_grok_ask",
      "description": "Ask questions and search the knowledgebase",
      "path": ["grok", "ask"],
      "category": "grok",
      "schema": { /* full JSON schema */ }
    }
  ],
  "total": 1
}
```

#### `b00t_execute`

```json
{
  "name": "b00t_execute",
  "description": "Execute any b00t command by name with parameters. Use b00t_search first to discover valid command names and schemas.",
  "input_schema": {
    "type": "object",
    "properties": {
      "command": {
        "type": "string",
        "description": "Exact command name from b00t_search (e.g., 'b00t_grok_ask')"
      },
      "params": {
        "type": "object",
        "description": "Command parameters as a JSON object matching the command's schema"
      }
    },
    "required": ["command", "params"]
  }
}
```

### 3. Execution Flow

1. LLM calls `b00t_search` with a natural-language query (e.g., `"how do I add a task?"`).
2. Server fuzzy-matches against registered tool metadata (name, description, `command_path()`).
3. LLM receives results, selects `b00t_task_add`.
4. LLM calls `b00t_execute` with:
   ```json
   {
     "command": "b00t_task_add",
     "params": { "title": "write tests", "priority": 2, "tags": "tdd,rust" }
   }
   ```
5. Server validates that `b00t_task_add` exists, then delegates to `registry.execute("b00t_task_add", params)`.
6. Existing `params_to_args()` and positional handling work unchanged.

### 4. Search Ranking

Search uses a simple weighted BM25-like heuristic (no external deps):

- Command name exact match: +100
- Command name prefix match: +50
- Path segment match: +30
- Description keyword match: +10
- Category exact match: +20

Results sorted descending; ties broken by declaration order in registry.

### 5. Backward Compatibility & Migration

**Phase 1 (opt-in)** — Add `--code-mode` CLI flag / `B00T_MCP_CODE_MODE=1` env var. When enabled, `create_mcp_registry()` returns a registry containing *only* `b00t_search` and `b00t_execute`. When disabled, current flat behavior remains.

**Phase 2 (default)** — Make Code Mode the default. Flat tools still registered but marked deprecated in descriptions. Update client configs (Claude Code, VS Code, Cursor) to prefer Code Mode.

**Phase 3 (legacy removal)** — After 2 release cycles, remove flat tool registration entirely. `create_mcp_registry()` always emits the 2-tool surface.

### 6. Error Contract

`b00t_execute` returns structured errors:

```json
{
  "success": false,
  "error": {
    "type": "unknown_command",
    "message": "Command 'b00t_foo_bar' not found. Did you mean 'b00t_grok_ask'?"
  },
  "suggestions": ["b00t_grok_ask", "b00t_cli_check"]
}
```

Error types:
- `unknown_command` — name not in registry; returns Levenshtein suggestions
- `invalid_params` — param fails JSON schema validation
- `acl_denied` — command blocked by ACL policy
- `execution_failed` — underlying `registry.execute()` returned Err

## Files to Touch

| File | Change |
|------|--------|
| `src/mcp_tools.rs` | Add `SearchCommand`, `ExecuteCommand`; add `create_code_mode_registry()` |
| `src/clap_reflection.rs` | Add `search_tools(query, category, limit) -> Vec<Tool>` helper on `McpCommandRegistry` |
| `src/mcp_server_rusty.rs` | Accept `--code-mode` flag; choose registry variant at startup |
| `src/main.rs` | Parse `--code-mode` / env var; pass to server constructor |
| `tests/mcp_server_test.rs` | Add tests for search ranking, execute dispatch, error suggestions |

## Non-Goals

- **Not** replacing CLAP with a dynamic schema system. CLAP remains the source of truth.
- **Not** changing the proxy MCP tools (`proxy_discover`, `proxy_execute`). Those serve external servers; this RFC serves internal b00t commands.
- **Not** adding natural-language-to-command translation (NL routing). The LLM still reasons about command names; we just compress the tool namespace.

## Open Questions

1. Should `b00t_search` support regex or only simple substring matching? **→ k0mmand3r/CLI-style first-token routing**: split query on whitespace, first token attempts exact match on command path prefix (`mcp`, `cli`, `agent`, `grok`, `task`, `session`, `tutorial`, `ontology`, `acp`), remaining tokens fuzzy-match name/description. No regex.
2. Should we expose a `b00t_categories` tool to list available categories, or embed categories in `search` results? **→ Embed in search**: each result includes `category` + `examples` array (top-3 example invocations). No separate tool.
3. Does the ACL layer apply at `execute` time only, or also filter `search` results? **→ ACL comes later**: no filtering in Phase 1. Search returns all commands. ACL enforcement remains at `execute` time only when integrated (post-roles/tag lookups in datums).

## Acceptance Criteria

- [ ] `b00t_search` returns ranked results for `"grok ask"` containing `b00t_grok_ask` in top-3
- [ ] `b00t_execute` successfully runs `b00t_task_add` with params object
- [ ] `b00t_execute` returns `unknown_command` with suggestions for `"b00t_foo"`
- [ ] `--code-mode` flag reduces `list_tools` length from ~40 to 2
- [ ] Flat mode continues to work when `--code-mode` is absent
- [ ] All existing tests pass in both modes

## References

- `proxy_mcp_tools.rs` — prior art for generic `execute` + `list` pattern on external MCP servers
- `clap_reflection.rs` — `McpCommandRegistry::get_tools()` provides the metadata we search over
- MCP protocol spec — tool enumeration and call semantics

<!-- b00t:map v1
summary: SDD-007 — Consolidate b00t-mcp flat tools into search+execute Code Mode
tags: b00t-mcp, mcp, code-mode, rfc, consolidation, registry
tier: frontier
cmds: b00t-mcp --code-mode, b00t_search, b00t_execute
complexity: 7
-->
