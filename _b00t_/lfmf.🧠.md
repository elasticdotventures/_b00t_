# LFMF (Learn From My Failures) - Syntax Therapist System 🧠

**Learn before you develop**: Master the art of tribal knowledge capture and contextual debugging assistance.

## Overview

LFMF is b00t's intelligent debugging companion that transforms failure into wisdom. It captures what goes wrong during development and provides contextual advice to prevent repeating the same mistakes.

### Two Systems Working Together

- **`b00t learn`**: Pre-development skill acquisition (broad guidance)
- **`b00t lfmf`**: Post-failure lesson capture (specific tribal knowledge)

## Core Commands

### Recording Lessons (`b00t lfmf`)

Record failures and their solutions using the format `"<topic>: <solution>"`:

```bash
# Rust compilation issues
b00t lfmf rust "cargo build conflict: Use unset CONDA_PREFIX before cargo build to avoid PyO3 linker errors"
b00t lfmf rust "clippy unnecessary mut: Remove mut keyword from variables that aren't reassigned"

# Justfile syntax traps  
b00t lfmf just "Template syntax conflict: Use grep/cut instead of Go template {{.Names}} to avoid Just variable interpolation conflicts"
b00t lfmf just "Duplicate recipe error: Use 6C pattern - comment old version, rename to name-legacy, clean later"

# Docker best practices
b00t lfmf docker "COPY vs ADD: Use COPY for local files, ADD only for URLs/archives with auto-extraction"

# Kubernetes gotchas
b00t lfmf k8s "ImagePullBackOff: Check registry auth, image name spelling, and network connectivity"
```

### Getting Advice (`b00t advice`)

#### List All Lessons
```bash
b00t advice rust list              # Show all Rust lessons
b00t advice just list --count 3    # Show first 3 justfile lessons  
b00t advice docker list            # Show all Docker lessons
```

#### Search for Specific Issues
```bash
# Direct error pattern matching
b00t advice rust "PyO3 linker"
b00t advice just "Unknown start of token '.'"
b00t advice k8s "ImagePullBackOff"

# Semantic search across lessons
b00t advice rust "search template"
b00t advice just "search recipe"
b00t advice docker "search copy"
```

## Advanced Features

### Configuration

Create `lfmf.toml` for custom settings:

```toml
[qdrant]
url = "http://localhost:6334"
# api_key = "your-api-key" 
# collection = "b00t_knowledge"

[filesystem]
learn_dir = "lfmf"  # Separate from learn system
```

Override with environment variables:
```bash
export QDRANT_URL=http://localhost:6334
export QDRANT_API_KEY=your-key
export B00T_LEARN_DIR=~/.b00t/failures
```

### Dual Storage Architecture

LFMF uses intelligent dual storage:

1. **Vector Database** (Qdrant + Ollama): Semantic search with embeddings
2. **Filesystem Fallback**: Reliable pattern matching when vector DB unavailable

```bash
# When vector DB works - semantic search with AI
b00t advice rust "linking issues"
# → Finds: "cargo build conflict: Use unset CONDA_PREFIX..."

# When vector DB unavailable - filesystem pattern matching  
b00t advice rust "CONDA_PREFIX"
# → Still finds relevant lessons through text matching
```

### MCP Integration

LFMF is available as MCP tools for AI development environments:

- `b00t_lfmf(tool: str, lesson: str)` - Record lessons
- `b00t_advice(tool: str, query: str, count?: number)` - Get advice

## Patterns & Best Practices

### Recording Effective Lessons

✅ **Good Format**: `"<specific-topic>: <actionable-solution>"`
```bash
b00t lfmf rust "cargo build CONDA_PREFIX: unset CONDA_PREFIX before cargo build"
```

❌ **Poor Format**: Generic or vague descriptions  
```bash
b00t lfmf rust "build failed"  # Too vague
b00t lfmf rust "don't use conda" # Negative framing
```

### Affirmative Style

LFMF encourages positive, actionable guidance:

✅ **Affirmative**: "Use X for Y benefit"  
❌ **Negative**: "Don't use X" or "Never do Y"

### Token Limits

- **Topic**: < 25 tokens (OpenAI tiktoken)
- **Body**: < 250 tokens  
- Focus on concise, actionable advice

## Real-World Examples

### Debugging Session Flow

1. **Hit an error**: `error: Unknown start of token '.'`
2. **Ask for advice**: `b00t advice just "Unknown start of token '.'"`
3. **Get contextual help**: System finds template conflict lessons
4. **Apply solution**: Use grep/cut instead of Go templates
5. **Record lesson**: `b00t lfmf just "Template syntax conflict: ..."`

### Cross-Tool Learning

LFMF captures patterns across different tools:

```bash
# Python environment conflicts
b00t lfmf python "conda pip conflict: Use pip install --user in conda environments"

# Rust environment conflicts  
b00t lfmf rust "CONDA_PREFIX PyO3: unset CONDA_PREFIX before cargo build"

# Later search finds both
b00t advice python "conda conflict"  # Finds pip solution
b00t advice rust "conda"             # Finds CONDA_PREFIX solution
```

## Architecture Notes

### Shared Library Design

LFMF uses `b00t-c0re-lib::LfmfSystem` for:
- **Code reuse**: Same implementation for CLI and MCP
- **Type safety**: Rust compile-time guarantees
- **Configuration**: External TOML/env setup
- **Resilience**: Graceful fallbacks

### Melvin 🤓 Build Intelligence

The system includes `build.rs` melvin scripts that detect common environment conflicts:

```rust
// Detects CONDA_PREFIX + PyO3 conflicts during build
if env::var("CONDA_PREFIX").is_ok() {
    println!("cargo:warning=✅ SOLUTION: unset CONDA_PREFIX && cargo build");
}
```

## Integration Examples

### VS Code + Claude Code
```bash
# MCP tools automatically available
# Record lessons through chat interface
# Get contextual advice during debugging
```

### Terminal Workflow
```bash
# Standard debugging flow
cargo build  # fails with linker error
b00t advice rust "PyO3 linker"  # get instant help
unset CONDA_PREFIX && cargo build  # apply solution  
b00t lfmf rust "CONDA_PREFIX conflict: unset before build"  # record lesson
```

### CI/CD Integration
```bash
# In build scripts
if ! cargo build; then
    b00t advice rust "$(cargo build 2>&1 | head -1)"
    exit 1
fi
```

## Success Metrics

LFMF tracks effectiveness through:
- **Reduced debugging time**: Faster error → solution cycles
- **Pattern recognition**: Prevention of repeated issues  
- **Hive knowledge growth**: Collective learning across projects
- **Context preservation**: Tribal knowledge survives team changes

## Future Enhancements

- **Conversational interface**: Chat-based syntax therapist
- **Cross-session tracking**: Maintain debugging conversations
- **Team synchronization**: Share lessons across agent instances
- **Automated recording**: Capture lessons from successful fixes

---

**Remember**: LFMF transforms debugging from "search docs + trial/error" to "chat with experienced syntax therapist who remembers similar problems."

The goal is not just to solve problems, but to ensure they never happen again. 🧠✨