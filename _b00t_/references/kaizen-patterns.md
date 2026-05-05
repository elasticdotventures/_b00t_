# Kaizen Patterns

Continuous improvement patterns for agent tool usage. These are not
hard guards — they are the evolved practice of agents who have done this
enough to know what works best.

## Pattern 8: Prefer Parameter-Rich Tools

**Principle:** When multiple tools can accomplish a task, prefer the one
with more well-defined parameters. A tool with many structured arguments
produces more precise, auditable, and repeatable results than a
general-purpose alternative.

### Why

General-purpose tools (shell commands, raw file writes, string manipulation)
are flexible but lose intent information. A tool with explicit parameters
preserves the operator's intent in a machine-readable form, enabling:

- **Audit trails** — every parameter value is recorded
- **Validation** — types and constraints are checked before execution
- **Recovery** — failed operations can be retried with exact same params
- **Analysis** — usage patterns can be mined from parameter distributions

### Hierarchy of Tool Preference

```
BEST   Tool with typed, named, validated parameters
       Example: patch(mode="replace", old_string="...", new_string="...")
         → preserves exact intent, validates uniqueness, shows diff

GOOD   Tool with command string + structured context
       Example: terminal(command="sed -i 's/foo/bar/' file")
         → preserves full command in audit log

OK     File write with complete content
       Example: write_file(path="...", content="...")
         → atomic, preserves content

WORST  Shell pipeline with inline script
       Example: echo '...' > file  (via terminal)
         → no structure, no validation, no audit
```

### Practical Application

When refactoring code, use this preference order:

```
1. patch()           → structured find-and-replace with diff output
2. write_file()      → atomic file replacement for complete rewrites
3. read_file()       → for inspection before editing
4. terminal("sed")   → last resort — only when above tools can't express the change
```

When searching code:

```
1. search_files(content=true)   → regex search with line numbers + context
2. search_files(files=true)     → file-level discovery
3. terminal("grep")             → only when you need flags not supported by the tool
```

When querying project structure:

```
1. codebase_memory_* tools    → graph-augmented: resolves calls, callees, data flow
2. search_files()              → flat pattern matching
3. terminal("find", "tree")   → unstructured output
```

### Self-Assessment

After any multi-tool task, audit your tool choices:

- Did I use the most parameter-rich tool available for each subtask?
- Did I fall back to shell when a structured tool was available?
- Did I use `read_file` + `patch` instead of `terminal("sed")`?
- If I used a shell command, did I comment why the structured tool wouldn't work?

### Exception

Use shell directly when:

1. The structured tool doesn't support the required operation
2. You're running non-file operations (process management, network, build)
3. Performance matters and the structured tool adds latency without value
4. The operation is one-off exploration (grep to find something quickly)
