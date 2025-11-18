# b00t Learn + Grok RAG Workflow Guide

Complete guide to using b00t's knowledge management and RAG capabilities.

## Table of Contents

1. [Overview](#overview)
2. [System Components](#system-components)
3. [Quick Start](#quick-start)
4. [Common Workflows](#common-workflows)
5. [Command Reference](#command-reference)
6. [Troubleshooting](#troubleshooting)
7. [Advanced Usage](#advanced-usage)

## Overview

b00t provides two complementary knowledge systems:

### LFMF (Learn From My Failures)
- **Purpose**: Capture tribal knowledge and specific failure solutions
- **Storage**: Filesystem + Vector DB (dual storage)
- **Use Case**: "I just fixed this error, how do I remember the solution?"

### Grok RAG (Retrieval-Augmented Generation)
- **Purpose**: Store and search large amounts of documentation
- **Storage**: Vector database (Qdrant)
- **Use Case**: "I need to search across all my documentation"

### Unified Interface: `b00t learn`

The `b00t learn` command unifies both systems:
- Displays curated documentation
- Records LFMF lessons
- Searches lessons
- Digests content to RAG
- Queries RAG knowledgebase

## System Components

### Dependencies

**Required for LFMF (filesystem mode)**:
- None (pure filesystem storage)

**Required for LFMF (vector DB mode)**:
- Qdrant vector database
- Embedding model (OpenAI or Ollama)

**Required for Grok RAG**:
- Qdrant vector database
- b00t-grok-py MCP server
- Python with uv
- Embedding model (OpenAI or Ollama)

### Data Storage Locations

```
~/.b00t/
├── _b00t_/
│   ├── learn/                  # LFMF lessons (markdown)
│   │   ├── rust.md
│   │   ├── git.md
│   │   └── docker.md
│   └── grok-guru.mcp.toml     # Grok MCP config
├── qdrant/                     # Vector DB storage
└── raglight/                   # RAGLight storage
    └── uploads/
```

## Quick Start

### 1. Learn About a Topic

```bash
# Display curated documentation
b00t learn rust
b00t learn git
b00t learn docker

# Show table of contents only
b00t learn rust --toc

# Jump to specific section
b00t learn rust --section 3

# Concise output (token-optimized)
b00t learn rust --concise
```

### 2. Record a Lesson (LFMF)

When you fix an error or learn something important:

```bash
# Format: "<topic>: <solution>"
b00t learn rust --record "cargo build conflict: Unset CONDA_PREFIX to avoid PyO3 errors"

b00t learn git --record "merge conflict: Use git mergetool to resolve conflicts interactively"

b00t learn docker --record "port binding: Use --publish to expose container ports to host"
```

**Best Practices for Recording**:
- ✅ Be specific and actionable
- ✅ Use affirmative language ("Use X for Y")
- ✅ Keep topics under 25 tokens
- ✅ Keep body under 250 tokens
- ❌ Avoid generic statements
- ❌ Avoid negative language ("Don't use X")

### 3. Search for Lessons

```bash
# List all lessons for a topic
b00t learn rust --search list

# Search for specific error pattern
b00t learn rust --search "linker error"

b00t learn git --search "merge"

# Limit results
b00t learn docker --search "port" --limit 3
```

### 4. Digest Content to RAG

When you have documentation to index:

```bash
# Digest inline content
b00t learn rust --digest "Rust ensures memory safety through ownership and borrowing"

# Or use grok directly
b00t grok digest -t rust "Rust has zero-cost abstractions"

# Learn from URLs (via crawler)
b00t grok learn "https://doc.rust-lang.org/book/" -t rust

# Learn from files
b00t grok learn -s "notes.md" "$(cat notes.md)" -t rust
```

### 5. Query RAG Knowledgebase

```bash
# Query via learn command
b00t learn rust --ask "How does ownership work?"

# Or use grok directly
b00t grok ask "memory safety" -t rust

# Limit results
b00t grok ask "borrowing rules" -t rust --limit 5
```

## Common Workflows

### Workflow 1: Daily Development Debugging

```bash
# 1. Hit an error
cargo build
# Error: linking with `cc` failed: PyO3 conflict

# 2. Search for existing solutions
b00t learn rust --search "PyO3"

# 3. If found, apply solution
# If not found, solve it yourself

# 4. Record the solution for next time
b00t learn rust --record "PyO3 linker: Unset CONDA_PREFIX before building"
```

### Workflow 2: Learning a New Technology

```bash
# 1. Read curated documentation
b00t learn kubernetes --toc

# 2. Digest important articles
b00t grok learn "https://kubernetes.io/docs/concepts/" -t k8s

# 3. As you learn, record gotchas
b00t learn k8s --record "pod networking: Each pod gets its own IP address"

# 4. Later, search when you forget
b00t learn k8s --search "networking"
b00t learn k8s --ask "How do pods communicate?"
```

### Workflow 3: Building Team Knowledge

```bash
# Team member encounters issue and records solution
b00t learn docker --record "layer caching: Order COPY commands from least to most frequently changed"

# Commit to shared repo
git add _b00t_/learn/docker.md
git commit -m "docs: Add Docker layer caching best practice"
git push

# Other team members pull and search
git pull
b00t learn docker --search "caching"
```

### Workflow 4: Comprehensive Documentation

```bash
# Index project documentation
b00t grok learn -s "docs/architecture.md" "$(cat docs/architecture.md)" -t project

# Index external resources
b00t grok learn "https://github.com/project/wiki" -t project

# Later query across everything
b00t grok ask "How does the authentication flow work?" -t project

# Get specific count
b00t grok ask "database schema" -t project --limit 10
```

## Command Reference

### b00t learn

```bash
b00t learn <topic> [OPTIONS]

OPTIONS:
  --record <LESSON>    Record lesson in "topic: solution" format
  --search <QUERY>     Search lessons ("list" for all)
  --digest <CONTENT>   Digest content to RAG
  --ask <QUERY>        Query RAG knowledgebase
  --limit <N>          Max results (default: 5)
  --global             Record globally (default: repo)
  --toc                Show table of contents only
  --section <N>        Jump to specific section
  --concise            Concise token-optimized output
  --man                Force display man page
```

### b00t grok

```bash
b00t grok <SUBCOMMAND> [OPTIONS]

SUBCOMMANDS:
  digest    Digest content into chunks
  ask       Ask questions and search
  learn     Learn from URLs or files

DIGEST:
  b00t grok digest -t <TOPIC> <CONTENT> [--rag <BACKEND>]

ASK:
  b00t grok ask <QUERY> [-t <TOPIC>] [--limit <N>] [--rag <BACKEND>]

LEARN:
  b00t grok learn [-s <SOURCE>] <CONTENT> -t <TOPIC> [--rag <BACKEND>]

OPTIONS:
  --rag <BACKEND>    Use RAG backend (raglight or default)
```

### b00t advice (LFMF)

```bash
b00t advice <TOOL> <QUERY|list> [OPTIONS]

OPTIONS:
  --count <N>    Max results (default: 5)

EXAMPLES:
  b00t advice rust list
  b00t advice rust "linker error"
  b00t advice git "search merge" --count 3
```

### b00t lfmf

```bash
b00t lfmf <TOOL> "<TOPIC>: <BODY>" [--global]

OPTIONS:
  --global    Record globally (default: repo)

EXAMPLES:
  b00t lfmf rust "ownership: Use Rc<RefCell<T>> for shared mutable state"
```

## Troubleshooting

### Issue: "GrokClient not initialized"

**Cause**: b00t-grok-py MCP server not available

**Solutions**:
```bash
# Check if Qdrant is running
docker ps | grep qdrant

# Start Qdrant if needed
b00t start qdrant.docker

# Verify environment variables
echo $QDRANT_URL

# Set if missing
export QDRANT_URL="http://localhost:6333"
```

### Issue: "Vector database unavailable"

**Cause**: Qdrant not running or unreachable

**Solutions**:
```bash
# Use filesystem fallback (LFMF only)
# This works automatically - lessons are still recorded to files

# Or start Qdrant
b00t start qdrant.docker

# Check Qdrant health
curl http://localhost:6333/health
```

### Issue: "No knowledge found for topic"

**Cause**: Topic not in learn.toml and no man page exists

**Solutions**:
```bash
# Create a learn file manually
mkdir -p _b00t_/learn
echo "# My Custom Topic" > _b00t_/learn/mytopic.md

# Or add to learn.toml
echo 'mytopic = "_b00t_/learn/mytopic.md"' >> learn.toml

# Or check if man page exists
man mytopic
```

### Issue: "Failed to digest content"

**Causes & Solutions**:

1. **Qdrant not running**:
   ```bash
   b00t start qdrant.docker
   ```

2. **Network issues**:
   ```bash
   # Check connectivity
   curl http://localhost:6333/health

   # Check DNS
   ping qdrant
   ```

3. **Content too large**:
   ```bash
   # Grok handles chunking automatically
   # But if issues persist, chunk manually:
   b00t grok digest -t topic "First chunk"
   b00t grok digest -t topic "Second chunk"
   ```

### Issue: "Lesson exceeds token limit"

**Cause**: Lesson topic or body too long

**Solution**:
```bash
# Topic must be <25 tokens, body <250 tokens
# Be more concise:

# ❌ Too long:
b00t learn rust --record "Understanding the comprehensive details of Rust's advanced memory management system including ownership, borrowing, and lifetimes: The Rust compiler enforces strict rules..."

# ✅ Good:
b00t learn rust --record "memory safety: Rust enforces ownership rules at compile time to prevent data races"
```

## Advanced Usage

### Custom LFMF Configuration

Create `lfmf.toml`:

```toml
[qdrant]
url = "http://custom-qdrant:6334"
collection_name = "custom_lfmf"

[filesystem]
learn_dir = "lfmf"  # Separate from learn/
```

### Using RAGLight Backend

```bash
# Digest with RAGLight
b00t grok digest -t rust "Content" --rag raglight

# Query with RAGLight
b00t grok ask "query" -t rust --rag raglight
```

### Programmatic Access (Rust)

```rust
use b00t_c0re_lib::{GrokClient, LfmfSystem};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Use GrokClient
    let mut client = GrokClient::new();
    client.initialize().await?;

    let result = client.digest("rust", "Rust content").await?;
    println!("Digested: {}", result.chunk_id);

    let ask_result = client.ask("memory safety", Some("rust"), Some(5)).await?;
    println!("Found {} results", ask_result.total_found);

    // Use LFMF
    let config = LfmfSystem::load_config(".")?;
    let mut lfmf = LfmfSystem::new(config);
    lfmf.initialize().await?;

    lfmf.record_lesson("rust", "topic: body").await?;
    let lessons = lfmf.get_advice("rust", "query", Some(5)).await?;

    Ok(())
}
```

### Environment Variables

```bash
# Qdrant configuration
export QDRANT_URL="http://localhost:6333"
export QDRANT_API_KEY="your-api-key"  # Optional

# Embedding provider
export EMBEDDING_PROVIDER="openai"    # or "ollama"
export OPENAI_API_KEY="sk-..."        # if using OpenAI
export OLLAMA_API_URL="http://localhost:11434"  # if using Ollama

# b00t paths
export _B00T_Path="$HOME/.b00t/_b00t_"

# Testing
export TEST_WITH_QDRANT=1  # Enable Qdrant tests
export B00T_DEBUG=1        # Verbose output
```

### Integration with MCP

The grok system is available as MCP tools:

```python
# In Claude Desktop or other MCP clients
# Tools available:
# - grok_digest
# - grok_ask
# - grok_learn
# - grok_status
```

### Batch Operations

```bash
# Digest multiple files
for file in docs/*.md; do
    b00t grok learn -s "$file" "$(cat "$file")" -t docs
done

# Record multiple lessons
b00t learn rust --record "pattern 1: solution 1"
b00t learn rust --record "pattern 2: solution 2"
b00t learn rust --record "pattern 3: solution 3"

# Query and process results
b00t grok ask "search query" -t topic --limit 100 | jq '.results[]'
```

## Best Practices

### Recording Lessons

1. **Record immediately** - Don't wait until you forget
2. **Be specific** - Include error messages or symptoms
3. **Include context** - What were you trying to do?
4. **Use keywords** - Make it searchable
5. **Affirmative style** - Say what TO do, not what NOT to do

### Organizing Knowledge

1. **Use consistent topics** - Stick to tool/language names
2. **Create topic hierarchy** - Use subtopics for organization
3. **Tag appropriately** - Use metadata for cross-referencing
4. **Regular review** - Update outdated lessons
5. **Share with team** - Commit LFMF files to repo

### Searching Effectively

1. **Start broad** - Use general terms first
2. **Refine gradually** - Add specific keywords
3. **Try variations** - Different phrasings may match better
4. **Use list mode** - See all lessons when unsure
5. **Combine systems** - Try both LFMF and RAG

### RAG Content Strategy

1. **Index strategically** - Don't index everything
2. **Organize by topic** - Use meaningful topic names
3. **Update regularly** - Re-index when docs change
4. **Verify accuracy** - Check RAG results before trusting
5. **Chunk appropriately** - Let grok handle automatic chunking

## Next Steps

- **Explore**: Try `b00t learn --help` for all options
- **Practice**: Record lessons as you work
- **Integrate**: Add to your daily workflow
- **Customize**: Configure for your team's needs
- **Contribute**: Share improvements back to b00t

## Resources

- **LFMF Guide**: `_b00t_/lfmf.🧠.md`
- **LFMF Quick Ref**: `_b00t_/learn/lfmf.md`
- **Architecture**: `GROK_ARCHITECTURE_MAP.md`
- **Issue #85 Analysis**: `ISSUE_85_ANALYSIS.md`
- **Tests**: `b00t-cli/tests/learn_rag_integration_test.rs`
