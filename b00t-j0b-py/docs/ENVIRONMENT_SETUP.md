# Environment Setup - The b00t Pattern

This document explains the b00t environment variable pattern using **direnv → .envrc → dotenv → .env**.

## Overview

The b00t ecosystem follows a DRY (Don't Repeat Yourself) philosophy for environment variable management:

1. **Datums specify WHICH** environment variables are required
2. **`.env` file contains** the actual secret values
3. **`direnv` automatically loads** environment variables when you enter the directory
4. **Rust validates** that required variables are present via PyO3 bindings

This pattern ensures:
- ✅ Secrets stay in `.env` (gitignored)
- ✅ Configuration is declarative (datums)
- ✅ Validation happens at runtime
- ✅ No duplicate logic between Rust and Python

## The Pattern Flow

```
┌─────────────────────────────────────────────────────────────┐
│  Developer enters project directory                         │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  direnv detects .envrc file                                 │
│  (automatically loads environment)                          │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  .envrc calls: dotenv                                       │
│  (loads variables from .env file)                           │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  .env contains actual API keys:                             │
│  OPENROUTER_API_KEY=sk-or-...                              │
│  ANTHROPIC_API_KEY=sk-ant-...                              │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  Python code runs                                           │
│  import b00t_py                                            │
│  b00t_py.check_provider_env("openrouter")                  │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  Rust validates against datum:                             │
│  ~/.dotfiles/_b00t_/openrouter.ai.toml                     │
│  [env]                                                      │
│  required = ["OPENROUTER_API_KEY"]                         │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│  ✅ Environment validated, agent runs                       │
└─────────────────────────────────────────────────────────────┘
```

## Setup Instructions

### 1. Install direnv

```bash
# macOS
brew install direnv

# Ubuntu/Debian
sudo apt-get install direnv

# Add to your shell (choose one):
# For bash:
echo 'eval "$(direnv hook bash)"' >> ~/.bashrc

# For zsh:
echo 'eval "$(direnv hook zsh)"' >> ~/.zshrc

# For fish:
echo 'direnv hook fish | source' >> ~/.config/fish/config.fish
```

### 2. Create .envrc

```bash
cd /path/to/b00t-j0b-py
cp .envrc.example .envrc
```

The `.envrc` file contains:

```bash
# Load project .env file (contains API keys)
dotenv

# Optionally load home directory .env for global keys
# dotenv ~/.env

# Optionally load additional environment-specific configs
# dotenv .env.local
```

### 3. Create .env with Your API Keys

```bash
cp .env.example .env
# Edit .env with your actual API keys
nano .env  # or vim, code, etc.
```

Example `.env`:

```bash
# OpenRouter (200+ models)
OPENROUTER_API_KEY=sk-or-v1-abc123...

# Anthropic (Claude)
ANTHROPIC_API_KEY=sk-ant-api03-xyz789...

# OpenAI
OPENAI_API_KEY=sk-proj-def456...

# HuggingFace
HF_TOKEN=hf_ghi789...

# Groq (ultra-fast inference)
GROQ_API_KEY=gsk_jkl012...
```

### 4. Allow direnv

```bash
direnv allow
```

This tells direnv to trust and load your `.envrc` file.

### 5. Verify Setup

```bash
# Check that environment variables are loaded
echo $OPENROUTER_API_KEY

# Test with Python
python3 -c "import os; print('✅ OPENROUTER_API_KEY loaded' if os.getenv('OPENROUTER_API_KEY') else '❌ Not loaded')"
```

## How Datums Work

### Provider Datums (in `~/.dotfiles/_b00t_/`)

Provider datums specify **WHICH** environment variables are required:

**`~/.dotfiles/_b00t_/openrouter.ai.toml`**:

```toml
[b00t]
name = "openrouter"
type = "ai"
hint = "OpenRouter multi-model gateway"

# Environment variables
[env]
# Required: Must be present in .env file
required = ["OPENROUTER_API_KEY"]

# Optional: Default values for non-secret configuration
defaults = { OPENROUTER_API_BASE = "https://openrouter.ai/api/v1" }

[models.qwen-2_5-72b-instruct]
capabilities = "text,chat,code,reasoning"
cost_per_1k_input_tokens = 0.00035
```

### Model Datums

Model datums reference the provider and specify which env var to use:

**`~/.dotfiles/_b00t_/qwen-2.5-72b.ai_model.toml`**:

```toml
[ai_model]
provider = "openrouter"
litellm_model = "openrouter/qwen/qwen-2.5-72b-instruct"
api_key_env = "OPENROUTER_API_KEY"  # ← References env var NAME, not value
```

## Usage in Python Code

### Using Pydantic-AI with Datums (Recommended)

```python
from b00t_j0b_py import create_pydantic_agent
import os

# Ensure direnv loaded the environment
assert os.getenv('OPENROUTER_API_KEY'), "Run 'direnv allow' first!"

# Create agent from datum (validates env vars automatically)
agent = create_pydantic_agent(
    model_datum_name="qwen-2.5-72b",
    system_prompt="You are a helpful assistant"
)

# Run agent
result = await agent.run("What is the capital of France?")
print(result.data)
```

### Manual Validation

```python
import b00t_py
import os

# Check if provider environment is valid
validation = b00t_py.check_provider_env("openrouter", "~/.dotfiles/_b00t_")

if validation["available"]:
    print("✅ OpenRouter environment ready")
else:
    print(f"❌ Missing environment variables: {validation['missing_env_vars']}")
    print("Add them to your .env file and run 'direnv allow'")
```

## File Structure

```
b00t-j0b-py/
├── .envrc              # ← Loaded by direnv (calls dotenv)
├── .env                # ← Contains actual API keys (GITIGNORED!)
├── .envrc.example      # ← Template for .envrc
├── .env.example        # ← Template showing required keys
└── src/
    └── b00t_j0b_py/
        └── pydantic_ai_integration.py  # ← Uses b00t_py to validate

~/.dotfiles/_b00t_/
├── openrouter.ai.toml       # ← Specifies required env vars
├── huggingface.ai.toml      # ← Specifies required env vars
├── qwen-2.5-72b.ai_model.toml  # ← References OPENROUTER_API_KEY
└── kimi-k2.ai_model.toml       # ← References OPENROUTER_API_KEY
```

## Security Best Practices

### ✅ DO

- ✅ Add `.env` to `.gitignore` (already done)
- ✅ Use `.env.example` as a template (committed to git)
- ✅ Store API keys only in `.env` files
- ✅ Use `direnv allow` to load environment per-project
- ✅ Validate environment variables before use

### ❌ DON'T

- ❌ Commit `.env` files to git
- ❌ Hard-code API keys in source code
- ❌ Store secrets in datum TOML files
- ❌ Share `.env` files via chat/email
- ❌ Use production keys in examples

## Multiple Environment Support

### Project + Home Directory

Load both project-specific and global keys:

**`.envrc`**:

```bash
# Load global keys from home directory
dotenv ~/.env

# Load project-specific keys (can override global)
dotenv
```

### Development vs Production

Use different `.env` files:

**`.envrc`**:

```bash
# Load base environment
dotenv

# Load environment-specific overrides
if [ "$ENVIRONMENT" = "production" ]; then
    dotenv .env.production
elif [ "$ENVIRONMENT" = "staging" ]; then
    dotenv .env.staging
else
    dotenv .env.development
fi
```

## Troubleshooting

### Variables Not Loading

```bash
# Check if direnv is hooked
direnv status

# Re-allow .envrc
direnv allow

# Check what direnv loaded
direnv export bash | grep API_KEY
```

### Missing Required Variables

```python
import b00t_py

# Check which providers are available
providers = b00t_py.list_ai_providers("~/.dotfiles/_b00t_")
print(f"Available providers: {providers}")

# Check specific provider
validation = b00t_py.check_provider_env("openrouter", "~/.dotfiles/_b00t_")
if not validation["available"]:
    print(f"Missing: {validation['missing_env_vars']}")
```

### Permission Denied

```bash
# Make .envrc executable (not required, but can help)
chmod +x .envrc

# Re-allow
direnv allow
```

## Advanced: Custom Validation

You can add custom validation logic in `.envrc`:

```bash
#!/usr/bin/env bash

# Load environment
dotenv

# Validate required keys are present
required_vars=("OPENROUTER_API_KEY" "ANTHROPIC_API_KEY")
for var in "${required_vars[@]}"; do
    if [ -z "${!var}" ]; then
        echo "❌ Missing required environment variable: $var"
        echo "   Add it to your .env file"
        return 1
    fi
done

echo "✅ All required environment variables loaded"
```

## Integration with CI/CD

For CI/CD environments where `direnv` isn't available:

```bash
# In GitHub Actions, GitLab CI, etc.
# Set secrets as environment variables directly
# The datum validation will still work

# Example .gitlab-ci.yml
variables:
  OPENROUTER_API_KEY: ${CI_OPENROUTER_API_KEY}

test:
  script:
    - python3 -m pytest
```

## Summary

The b00t pattern provides:

1. **DRY**: Single source of truth for environment requirements (datums)
2. **Secure**: Secrets in `.env` (gitignored), not in code
3. **Automatic**: `direnv` loads environment when you `cd` into directory
4. **Validated**: Rust checks required vars are present before execution
5. **Flexible**: Supports multiple `.env` files, local/global keys

For questions, see the b00t documentation or CLAUDE.md gospel.
