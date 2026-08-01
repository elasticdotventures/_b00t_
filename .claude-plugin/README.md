# b00t Claude Code Plugin

> Extreme programming agent framework plugin for Claude Code

## Overview

The **b00t plugin** extends Claude Code with skills and patterns from the b00t extreme programming agent framework. It teaches Claude about:

- **Datum System**: TOML-based AI model and provider configuration
- **direnv Pattern**: Environment variable management (direnv → .envrc → dotenv → .env)
- **DRY Philosophy**: Don't Repeat Yourself - use libraries, leverage Rust via PyO3
- **Polyglot Patterns**: Rust + Python integration via PyO3 bindings

## Features

### 🎯 Agent Skills

The plugin provides three production-ready Agent Skills:

#### 1. **datum-system**
Helps work with b00t's TOML-based configuration system for AI models and providers.

**Activates on:**
- "create a datum for [model/provider]"
- "add [model] to the datum system"
- "configure [provider] in b00t"
- "validate provider configuration"

**Capabilities:**
- Create provider and model datums
- Load datums via Rust/PyO3 (DRY approach)
- Validate environment variables
- Discover available models and providers

#### 2. **direnv-pattern**
Implements secure environment variable management using direnv.

**Activates on:**
- "setup environment variables"
- "configure direnv"
- "create .envrc file"
- "environment not loading"

**Capabilities:**
- Set up direnv + .envrc + .env configuration
- Follow b00t pattern: WHICH (datums) vs VALUES (.env)
- Support multiple environments (dev, staging, prod)
- Integrate with datum system validation

#### 3. **dry-philosophy**
Enforces Don't Repeat Yourself and Never Reinvent the Wheel principles.

**Activates on:**
- "implement [common functionality]"
- "create a [parser/validator/client]"
- Any task that might duplicate existing code

**Capabilities:**
- Find existing libraries before writing code
- Use Rust via PyO3 instead of duplicating in Python
- Contribute upstream rather than maintain forks
- Write lean, maintainable code

## Installation

```bash
# Add the b00t marketplace
/plugin marketplace add elasticdotventures/_b00t_

# Install plugins
/plugin install b00t@b00t-plugins
/plugin install next-task@b00t-plugins
/plugin install skill-document-understanding@b00t-plugins
```

> **Tip:** For local development, use `/plugin marketplace add /path/to/.b00t`
> 
> MCP bundle recipes are generated to `.claude-plugin/recipes/{skills,roles}/*.json`.

## Usage

### Automatic Skill Activation

Skills activate automatically when Claude detects relevant phrases:

```
User: "I need to add OpenRouter as a provider"
Claude: [datum-system skill activates]
        "I'll help you create an OpenRouter provider datum..."

User: "Setup environment variables for the project"
Claude: [direnv-pattern skill activates]
        "I'll set up the direnv pattern for you..."

User: "Create a JSON parser"
Claude: [dry-philosophy skill activates]
        "Before writing code, let's check for existing libraries..."
```

### Manual Skill Reference

You can also explicitly reference skills:

```
User: "Using the datum-system skill, create a new provider for Groq"
```

## Skills Reference

### datum-system

**Purpose**: Work with b00t's TOML-based AI model configuration system.

**Key Concepts:**
- Provider datums: `~/.dotfiles/_b00t_/*.ai.toml`
- Model datums: `~/.dotfiles/_b00t_/*.ai_model.toml`
- Rust validation via PyO3 (DRY approach)
- Datums specify WHICH env vars, .env contains VALUES

**Example:**
```toml
# ~/.dotfiles/_b00t_/openrouter.ai.toml
[b00t]
name = "openrouter"
type = "ai"

[models.qwen-2_5-72b-instruct]
capabilities = "text,chat,code"
cost_per_1k_input_tokens = 0.00035

[env]
required = ["OPENROUTER_API_KEY"]
defaults = { OPENROUTER_API_BASE = "https://openrouter.ai/api/v1" }
```

**Python Usage:**
```python
import b00t_pyverse

# Load datum (uses Rust via PyO3)
datum = b00t_pyverse.load_ai_model_datum("qwen-2.5-72b", "~/.dotfiles/_b00t_")

# Validate environment
validation = b00t_pyverse.check_provider_env("openrouter", "~/.dotfiles/_b00t_")
```

### direnv-pattern

**Purpose**: Implement b00t's environment variable management pattern.

**Pattern Flow:**
```
Developer enters dir → direnv loads .envrc → .envrc calls dotenv →
dotenv loads .env → Rust validates → ✅ App runs
```

**Setup:**
1. Create `.envrc`: `dotenv`
2. Create `.env`: `OPENROUTER_API_KEY=sk-or-...`
3. Allow: `direnv allow`

**Security:**
- `.env` is gitignored (contains secrets)
- `.env.example` is committed (template)
- Datums specify WHICH vars needed
- Rust validates before execution

### dry-philosophy

**Purpose**: Enforce DRY (Don't Repeat Yourself) and NRtW (Never Reinvent the Wheel).

**Principles:**
1. **Search first**: Check PyPI/crates.io before coding
2. **Use libraries**: Prefer established packages over custom code
3. **Leverage Rust**: Use PyO3 instead of duplicating in Python
4. **Contribute upstream**: Fork, fix, PR - don't maintain private copies

**Decision Tree:**
```
Need functionality?
  ↓
Exists in library? → YES → Use it (DRY)
  ↓
  NO
  ↓
Exists in b00t Rust? → YES → Use via PyO3 (DRY)
  ↓
  NO
  ↓
Truly novel? → YES → Implement (with tests)
  ↓
  NO → Search harder
```

## Architecture

### b00t Technology Stack

- **Rust**: Core libraries, datum parsing, system operations
- **Python**: Job orchestration, agent wrappers, API integration
- **PyO3**: Rust-to-Python bindings (DRY bridge)
- **Pydantic-AI**: Production agent framework (25+ providers)
- **RQ**: Redis-based job queue
- **direnv**: Environment variable management

### File Structure

```
_b00t_/
├── .claude-plugin/
│   ├── plugin.json          # Plugin manifest
│   └── README.md           # This file
├── skills/
│   ├── datum-system/
│   │   └── SKILL.md        # Datum system skill
│   ├── direnv-pattern/
│   │   └── SKILL.md        # Environment pattern skill
│   └── dry-philosophy/
│       └── SKILL.md        # DRY principles skill
├── b00t-c0re-lib/          # Rust core libraries
├── b00t-pyverse/                # PyO3 bindings
├── b00t-j0b-py/            # Python job system
└── _b00t_/                 # Datum repository
    ├── *.ai.toml           # Provider datums
    └── *.ai_model.toml     # Model datums
```

## Integration Points

### With b00t Datum System

```python
from b00t_j0b_py import create_pydantic_agent

# Create agent from datum (skill: datum-system)
agent = create_pydantic_agent(
    model_datum_name="qwen-2.5-72b",
    system_prompt="You are a helpful assistant"
)

result = await agent.run("What is the capital of France?")
```

### With direnv Pattern

```bash
# Setup (skill: direnv-pattern)
cp .envrc.example .envrc
cp .env.example .env
# Edit .env with actual keys
direnv allow

# Validate (skill: datum-system)
python3 -c "import b00t_pyverse; print(b00t_pyverse.check_provider_env('openrouter', '~/.dotfiles/_b00t_'))"
```

### With DRY Philosophy

```python
# Instead of writing custom code (skill: dry-philosophy)
# Use existing library
import httpx  # ✅ DRY

# Instead of duplicating Rust logic
# Use PyO3 bindings
import b00t_pyverse  # ✅ DRY
datum = b00t_pyverse.load_ai_model_datum("model", "path")
```

## Requirements

### System Dependencies

- **Rust**: 1.82+ (for building b00t-pyverse)
- **Python**: 3.12+
- **direnv**: Latest version
- **Git**: For version control

### Python Packages

```toml
# Core
pydantic-ai >= 0.0.14
pydantic >= 2.5.0
pydantic-settings >= 2.0.0

# Optional (for full b00t functionality)
rq >= 1.16.0          # Job queue
redis >= 5.0.0        # Cache
httpx >= 0.25.0       # HTTP client
```

### Rust Crates

```toml
# Core
pyo3 = "0.22"         # Python bindings
serde = "1.0"         # Serialization
toml = "0.8"          # TOML parsing
```

## Examples

### Example 1: Adding a New AI Provider

```bash
User: "Add Groq as a new provider with llama-3.1 model"

Claude (datum-system skill):
"I'll create a Groq provider datum at ~/.dotfiles/_b00t_/groq.ai.toml:

[b00t]
name = "groq"
type = "ai"
hint = "Groq - ultra-fast LLM inference"

[models.llama-3_1-70b-versatile]
capabilities = "text,chat,code"
context_length = 32768
cost_per_1k_input_tokens = 0.00059

[env]
required = ["GROQ_API_KEY"]
defaults = { GROQ_API_BASE = "https://api.groq.com/openai/v1" }

Now add GROQ_API_KEY to your .env file and run 'direnv allow'."
```

### Example 2: Setting Up Environment

```bash
User: "Setup environment variables for this project"

Claude (direnv-pattern skill):
"I'll set up the b00t direnv pattern:

1. Creating .envrc:
   dotenv

2. Creating .env with required keys:
   OPENROUTER_API_KEY=...
   ANTHROPIC_API_KEY=...

3. Run 'direnv allow' to load environment

4. Verify: echo $OPENROUTER_API_KEY"
```

### Example 3: Finding Existing Library

```bash
User: "Create a function to parse YAML files"

Claude (dry-philosophy skill):
"Before writing custom code, let's check for existing libraries:

pip search yaml

Found: PyYAML (most popular, 50M+ downloads/month)

Instead of custom implementation, use:

import yaml

with open('file.yaml') as f:
    data = yaml.safe_load(f)

This follows DRY - no need to reinvent YAML parsing."
```

## Troubleshooting

### Skills Not Activating

**Symptom**: Claude doesn't use b00t skills

**Solution**:
1. Verify plugin is installed: `/plugin list`
2. Check `.claude-plugin/plugin.json` exists
3. Restart Claude Code
4. Use explicit trigger phrases

### Datum Loading Fails

**Symptom**: `b00t_pyverse.load_ai_model_datum()` fails

**Solution**:
1. Check datum files exist in `~/.dotfiles/_b00t_/`
2. Verify TOML syntax: `tomlq . ~/.dotfiles/_b00t_/provider.ai.toml`
3. Ensure b00t-pyverse is installed: `pip install -e b00t-pyverse/`

### Environment Variables Not Loading

**Symptom**: `os.getenv('API_KEY')` returns None

**Solution**:
1. Check `.env` file has the key
2. Verify `.envrc` calls `dotenv`
3. Run `direnv allow`
4. Test: `direnv export bash | grep API_KEY`

## Contributing

Contributions welcome! To add new skills:

1. Create `skills/skill-name/SKILL.md`
2. Follow YAML frontmatter format
3. Include clear activation phrases
4. Add examples and workflows
5. Submit PR to elasticdotventures/_b00t_

## Documentation

- **CLAUDE.md**: b00t gospel - operating protocols
- **docs/ENVIRONMENT_SETUP.md**: Complete direnv pattern guide
- **docs/PYDANTIC_AI_ANALYSIS.md**: Pydantic-AI integration
- **b00t-j0b-py/README.md**: Python job system

## License

MIT License - see LICENSE file

## Support

- **Repository**: https://github.com/elasticdotventures/_b00t_
- **Issues**: https://github.com/elasticdotventures/_b00t_/issues
- **Organization**: PromptExecution (@promptexecution)

## Version History

- **0.1.0** (2025-11-13): Initial release
  - datum-system skill
  - direnv-pattern skill
  - dry-philosophy skill

---

**YEI** 🥾 _An extreme programming agent framework_
