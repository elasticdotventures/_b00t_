# LFMF (Learn From My Failures) - Syntax Therapist System

Master the art of tribal knowledge capture and contextual debugging assistance.

## Overview

LFMF is b00t's intelligent debugging companion that transforms failure into wisdom. It captures what goes wrong during development and provides contextual advice to prevent repeating the same mistakes.

### Two Systems Working Together

- **`b00t learn`**: Pre-development skill acquisition (broad guidance)
- **`b00t lfmf`**: Post-failure lesson capture (specific tribal knowledge)

## Core Commands

### Recording Lessons

Record failures and their solutions using the format `"<topic>: <solution>"`:

```bash
# Record specific failure lessons
b00t lfmf rust "cargo build conflict: Use unset CONDA_PREFIX before cargo build to avoid PyO3 linker errors"
b00t lfmf just "Template syntax conflict: Use grep/cut instead of Go template {{.Names}} to avoid Just variable interpolation conflicts"
b00t lfmf docker "COPY vs ADD: Use COPY for local files, ADD only for URLs/archives with auto-extraction"
```

### Getting Advice

```bash
# List all lessons for a tool
b00t advice rust list
b00t advice just list --count 3

# Search for specific error patterns
b00t advice rust "PyO3 linker"
b00t advice just "Unknown start of token '.'"

# Semantic search across lessons
b00t advice rust "search template"
```

## Key Features

- **Tribal Knowledge Capture**: Record what went wrong and how it was fixed
- **Semantic Search**: Find relevant solutions using error patterns and keywords  
- **Contextual Advice**: Get specific suggestions rather than generic documentation
- **Vector Database Integration**: Advanced semantic matching with filesystem fallback
- **MCP Integration**: Available to AI development environments

## Best Practices

### Recording Format
✅ **Good**: `"<specific-topic>: <actionable-solution>"`
❌ **Poor**: Generic or vague descriptions

### Affirmative Style
✅ **Affirmative**: "Use X for Y benefit"  
❌ **Negative**: "Don't use X"

### Real-World Flow
1. Hit an error → 2. Ask for advice → 3. Apply solution → 4. Record lesson

## Configuration

Create `lfmf.toml` for custom settings:

```toml
[qdrant]
url = "http://localhost:6334"

[filesystem]
learn_dir = "lfmf"  # Separate from learn system
```

## Architecture

- **Dual Storage**: Vector database + filesystem fallback
- **Shared Library**: `b00t-c0re-lib::LfmfSystem`
- **External Config**: TOML/environment variables
- **Type Safety**: Rust compile-time guarantees

LFMF transforms debugging from "search docs + trial/error" to "chat with experienced syntax therapist who remembers similar problems."

See the full guide at `_b00t_/lfmf.🧠.md` for complete documentation.