# Unified Datum Schema Example: just

This demonstrates the complete integration of learn, usage, references, and LFMF categories in the enhanced datum schema.

## Example: Recording a Just lesson

```bash
# Record a lesson about just modules
b00t lfmf just "module organization: Use mod declarations for clean separation; recipes in module run in module's directory, not invocation dir"

# Record a lesson about heredocs
b00t lfmf just "shebang heredocs: Use #!/usr/bin/env bash with heredocs to avoid justfile parsing conflicts with template syntax"

# Record a lesson about environment variables
b00t lfmf just "dotenv pattern: Always use 'set dotenv-load' at top of justfile; access with $VAR not {{var}}"
```

## Example: Getting advice

```bash
# List all just lessons
b00t advice just list

# Search for specific pattern
b00t advice just "heredoc"

# Get advice on modules
b00t advice just search "module"
```

## Example: View comprehensive datum info

```bash
# Show complete just datum information
b00t datum show just

# Output includes:
# - Basic info (type, hint, category)
# - Learn content from _b00t_/learn/just.md
# - Usage examples (4 examples)
# - References (elasticdotventures fork, official docs, upstream)
# - LFMF integration instructions
```

## Example: How datums in same category work together

Both `just.cli.toml` and `just-mcp.mcp.toml` have:
```toml
lfmf_category = "just"
```

This means:
- ✅ `b00t lfmf just "<lesson>"` - works for both just CLI and just-mcp
- ✅ `b00t advice just list` - shows lessons for all just-related tools
- ✅ Unified knowledge base across the just ecosystem
- ✅ `b00t datum show just` - shows CLI datum
- ✅ `b00t datum show just-mcp` - shows MCP server datum

## LFMF Lesson Format

```bash
# Good: Affirmative, actionable, tool-focused
b00t lfmf just "variable escaping: Use \$\${VAR} to pass literal \${VAR} to shell; use {{var}} for just variables"

# Good: Under 25 tokens topic, under 250 tokens body
b00t lfmf just "conditional recipes: Use [attribute] annotations like [no-cd] or [working-directory: '/tmp'] to control recipe execution context"

# Bad: Negative phrasing
b00t lfmf just "cd usage: Don't use cd in recipes"  # ❌ Use affirmative instead

# Bad: Repo-specific
b00t lfmf just "my-project: Fixed bug in my deploy script"  # ❌ Not tool wisdom
```

## Auto-digest Feature

The `just.cli.toml` includes:
```toml
[b00t.learn]
topic = "just"
auto_digest = true
```

This means:
- When you update `_b00t_/learn/just.md`, it can auto-feed to grok
- Grok can then provide semantic search across just documentation
- Combines structured lessons (LFMF) with comprehensive docs (learn)

## Complete Workflow Example

```bash
# 1. Learn about just (read documentation)
b00t learn just

# 2. Use just (see usage examples)
b00t datum show just
just -l

# 3. Encounter a problem (e.g., heredoc not working)
just init  # Fails with template syntax error

# 4. Record the lesson
b00t lfmf just "heredoc fix: Use shebang recipes #!/usr/bin/env bash for heredocs; prevents justfile from parsing content"

# 5. Later, get advice when facing similar issue
b00t advice just "heredoc"
# Returns: Match: 1.00 - [heredoc fix] Use shebang recipes...

# 6. See all available references
b00t datum show just | grep -A10 "References"
# Shows: elasticdotventures fork, official docs, upstream repo
```

## Benefits of Unified Schema

1. **Discoverability**: `b00t datum show just` shows everything about just
2. **Learning**: Topic links to comprehensive docs in `_b00t_/learn/just.md`
3. **Quick Reference**: Usage examples for common patterns
4. **Tribal Knowledge**: LFMF lessons capture what we learned from failures
5. **Community**: References link to elasticdotventures fork and upstream
6. **Integration**: Grok can auto-digest learn content for semantic search

## Category-based Organization

Tools sharing `lfmf_category = "just"`:
- just.cli.toml (command runner)
- just-mcp.mcp.toml (MCP server)

Future additions could include:
- just-lsp.toml (language server)
- justfile-syntax.vscode.toml (VS Code extension)

All sharing the same lesson base under `b00t lfmf just "<lesson>"`!
