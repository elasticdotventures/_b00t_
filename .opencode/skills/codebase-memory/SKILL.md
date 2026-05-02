---
name: codebase-knowledge-graph
description: |
  Query the codebase knowledge graph for structural code understanding.
  Provides functions, classes, routes, callers, and dependency analysis.
version: 1.0.0
allowed-tools: All with MCP
---

## What This Skill Does

Query the codebase knowledge graph instead of grep/glob:

- Find functions, classes, routes by pattern
- Trace call chains (who calls what)
- Get code snippets for specific symbols
- Run Cypher queries for complex patterns
- Get architecture overview

## When It Activates

- "explore the codebase"
- "understand the architecture"
- "what functions exist"
- "show me the structure"
- "who calls this function"
- "trace the call chain"
- "find callers of"
- "show dependencies"
- "impact analysis"

## MCP Tools (codebase-memory-mcp)

```javascript
// Search by pattern
codebase-memory-mcp_search_graph({
  query: "function pattern",
  project: "current-project"
})

// Trace callers/callees  
codebase-memory-mcp_trace_path({
  function_name: "handleAuth",
  project: "current-project", 
  direction: "inbound"
})

// Get source code
codebase-memory-mcp_get_code_snippet({
  qualified_name: "handlers.auth.handleAuth",
  project: "current-project"
})

// Architecture overview
codebase-memory-mcp_get_architecture({
  project: "current-project"
})
```

## Fallback to grep/glob

Use grep/glob when:
- Searching for string literals, error messages
- Searching config files, Dockerfiles
- MCP returns insufficient results

## Version

1.0.0