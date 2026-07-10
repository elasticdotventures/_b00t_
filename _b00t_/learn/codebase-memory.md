# Codebase Memory MCP - Code Knowledge Graph

Index, search, and analyze codebases using a knowledge graph powered by Model Context Protocol.

## Overview

codebase-memory-mcp builds a semantic knowledge graph of your codebase — functions, classes, routes, HTTP calls, async channels — enabling multi-hop queries, impact analysis, and cross-service tracing that grep/glob cannot do.

**Binary path:** `/home/brianh/.b00t/vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp`

## MCP Server Configuration

### b00t-mcp.json (Hermes Agent)

```json
{
  "mcpServers": {
    "codebase-memory": {
      "command": "/home/brianh/.b00t/vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp",
      "args": []
    }
  }
}
```

### b00t TOML Datum

The `_b00t_/codebase-memory.mcp.toml` file registers the server for b00t-mcp tool discovery.

## Available Tools

### index_repository
Index a repository into the knowledge graph.

**Modes:**
- `full` — all passes (structure + semantic)
- `moderate` — fast + semantic
- `fast` — structure only (no embeddings)
- `cross-repo-intelligence` — skip extraction, match Routes/Channels across projects

**Required:** `repo_path`
**Optional:** `mode`, `target_projects` (for cross-repo), `persistence` (compress graph to `.codebase-memory/graph.db.zst`)

```
index_repository(repo_path="/path/to/project", mode="moderate")
```

### search_graph
Search the knowledge graph for functions, classes, routes, variables. Use INSTEAD OF grep/glob for code definition discovery.

**Three independent search modes (can be combined):**
- `query="update settings"` — BM25 full-text with camelCase splitting and structural boosting
- `name_pattern=".*regex.*"` — exact regex pattern matching on qualified names
- `semantic_query=["send","pubsub","publish"]` — vector cosine search (ARRAY of keywords, not a string)

**Required:** `project`
**Optional:** `label`, `name_pattern`, `qn_pattern`, `file_pattern`, `relationship`, `min_degree`, `max_degree`, `exclude_entry_points`, `include_connected`, `limit`, `offset`

```
search_graph(project="my-project", query="update user settings")
search_graph(project="my-project", semantic_query=["send","pubsub","publish"])
```

### query_graph
Execute a Cypher query against the knowledge graph for complex multi-hop patterns.

**Required:** `query` (Cypher), `project`
**Optional:** `max_rows` (default: unlimited, ceiling 100k)

```
query_graph(project="my-project", query="MATCH (f:Function)-[:CALLS]->(t:Function) RETURN f.name, t.name LIMIT 20")
```

### trace_path
Trace paths through the code graph — callers, callees, data flow, cross-service.

**Modes:**
- `calls` — follow CALLS edges (default)
- `data_flow` — follow CALLS + DATA_FLOWS with argument expressions at each hop
- `cross_service` — follow HTTP_CALLS + ASYNC_CALLS + DATA_FLOWS through Routes

**Required:** `function_name`, `project`
**Optional:** `direction` (inbound/outbound/both, default both), `depth` (default 3), `parameter_name` (data_flow mode), `edge_types`, `risk_labels` (true/false, default false), `include_tests` (default false)

```
trace_path(function_name="handle_request", project="my-project", direction="inbound", depth=3)
trace_path(function_name="publish_event", project="my-project", mode="data_flow", parameter_name="payload")
```

### get_code_snippet
Read source code for a function/class/symbol by qualified name.

**Required:** `qualified_name` (from search_graph), `project`
**Optional:** `include_neighbors` (default false)

```
search_graph(project="my-project", query="handle_request")
get_code_snippet(qualified_name="src/handlers.rs::handle_request", project="my-project")
```

### get_graph_schema
Get the knowledge graph schema — node labels and edge types.

**Required:** `project`

### get_architecture
High-level architecture overview — packages, services, dependencies.

**Required:** `project`
**Optional:** `aspects`

### search_code
Graph-augmented code search. Greps for text patterns, then enriches with knowledge graph ranking.

**Required:** `pattern`, `project`
**Optional:** `file_pattern` (glob), `path_filter` (regex), `mode` (compact/full/files), `context` (lines), `regex` (bool), `limit` (default 10)

### list_projects
List all indexed projects. No parameters required.

### delete_project
Delete a project from the index.

**Required:** `project`

### index_status
Get indexing status of a project.

**Required:** `project`

### detect_changes
Detect code changes and their impact.

**Required:** `project`
**Optional:** `scope`, `depth` (default 2), `base_branch` (default "main"), `since` (git ref or date)

### manage_adr
Create or update Architecture Decision Records.

**Required:** `project`
**Optional:** `mode` (get/update/sections), `content`, `sections`

### ingest_traces
Ingest runtime traces to enhance the knowledge graph.

**Required:** `traces` (array), `project`

## Quick Start

```bash
# 1. Index a project
index_repository(repo_path="/home/brianh/projects/my-app", mode="moderate")

# 2. List indexed projects
list_projects()

# 3. Search for functions
search_graph(project="my-app", query="database connection pool")

# 4. Trace callers
trace_path(function_name="connect_db", project="my-app", direction="inbound")

# 5. Read source code
get_code_snippet(qualified_name="src/db.rs::connect_db", project="my-app")
```

## Verification

Test the MCP server handshake:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}' \
  | /home/brianh/.b00t/vendor/codebase-memory-mcp-b00t-ir0n-ledg3rr/build/c/codebase-memory-mcp
```

Expected response:
```json
{"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","serverInfo":{"name":"codebase-memory-mcp","version":"0.10.0"},...}}
```

## Integration with Hermes Agent

codebase-memory-mcp is registered as a proxy MCP tool in b00t-mcp. Hermes Agent will automatically discover and use the codebase-memory tools when analyzing or modifying code.

## Best Practices

1. **Always index first** — run `index_repository` before searching
2. **Use `search_graph` over `grep`** — it returns structured results with relationships
3. **Use `trace_path` for impact analysis** — find what calls a function and what a function calls
4. **Use `detect_changes` for PR review** — see the blast radius of code changes
5. **Use `cross-repo-intelligence` mode** — link HTTP calls and async channels across services
6. **Combine search modes** — BM25 + semantic in one `search_graph` call for best results

## LFMF Integration

Record lessons about codebase-memory usage:

```bash
b00t lfmf codebase-memory "index_repository modes: full (all passes), moderate (fast+semantic), fast (structure), cross-repo-intelligence (match Routes/Channels across projects)"
b00t lfmf codebase-memory "search_graph semantic_query MUST be an ARRAY of strings, not a single string — e.g. [\"send\",\"pubsub\"]"
b00t lfmf codebase-memory "search_graph query uses BM25 with camelCase splitting and structural boosting — best for natural language discovery"
b00t lfmf codebase-memory "trace_path data_flow: follow arg expressions through CALLS+DATA_FLOWS edges"
b00t lfmf codebase-memory "get_code_snippet: ALWAYS run search_graph first to get the exact qualified_name, then pass it to get_code_snippet"
b00t lfmf codebase-memory "index_repository persistence: writes compressed .codebase-memory/graph.db.zst for team sharing"
b00t lfmf codebase-memory "search_code: graph-augmented grep with structural ranking — definitions first, tests last"
```

---
mcp-timeout: CLI mode bypasses 5s MCP timeout. Edge type filtering broken in Cypher parser — type(r) returns nothing. Full-graph scans work but label/type-scoped queries are unreliable. 67K nodes indexed fine.
