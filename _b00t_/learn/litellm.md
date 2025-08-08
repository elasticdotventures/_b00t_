# LiteLLM Integration - b00t Gospel

LiteLLM provides a unified proxy server for multiple AI providers through OpenAI-compatible API endpoints.

## References
- [LiteLLM Documentation](https://docs.litellm.ai/)
- [LiteLLM GitHub](https://github.com/BerriAI/litellm)
- [b00t Issue #36](https://github.com/elasticdotventures/dotfiles/issues/36)

## Architecture

### b00t AI Model Datums
AI model configurations are stored as TOML files using b00t's datum pattern:

```toml
# claude-3-5-sonnet.ai_model.toml
[b00t]
name = "claude-3-5-sonnet"
type = "ai_model"
hint = "Anthropic Claude 3.5 Sonnet - premium reasoning model"

[ai_model]
provider = "anthropic"
size = "large"
capabilities = ["chat", "vision", "reasoning"]
litellm_model = "anthropic/claude-3-5-sonnet-20241022"
api_key_env = "ANTHROPIC_API_KEY"
api_base = ""
rpm_limit = 60
enabled = true
```

### Configuration Generation
The `_scan-models` recipe automatically discovers all `*.ai_model.toml` files and generates LiteLLM-compatible YAML:

```yaml
model_list:
  - model_name: claude-3-5-sonnet
    litellm_params:
      model: anthropic/claude-3-5-sonnet-20241022
      api_key: "os.environ/ANTHROPIC_API_KEY"
      rpm: 60
```

## b00t Justfile Commands

### Core Operations
```bash
# Quick start with aliases
just --justfile litellm.just s          # Start proxy (alias)
just --justfile litellm.just start      # Start proxy (alias)
just --justfile litellm.just up         # Start proxy (alias)
just --justfile litellm.just run        # Start proxy (full command)

# Management
just --justfile litellm.just stop       # Stop proxy
just --justfile litellm.just restart    # Restart proxy
just --justfile litellm.just logs       # View logs
```

### Configuration & Discovery
```bash
# Setup and configuration
just --justfile litellm.just init       # Create .env template
just --justfile litellm.just config     # Generate config from datums
just --justfile litellm.just models     # List available models

# Status and testing
just --justfile litellm.just status     # Show proxy status
just --justfile litellm.just test       # Health check + completion test
```

### Maintenance
```bash
just --justfile litellm.just clean      # Clean up containers and files
```

## Environment Configuration

### Security Pattern - direnv + .envrc (b00t Standard)
🤓 **NEVER** hardcode API keys in justfiles! Use `.envrc` + `direnv` for secure environment variables:

```bash
# ~/.dotfiles/.envrc (gitignored, secured by direnv)
export LITELLM_MASTER_KEY="sk-b00t-$(uuidgen | head -c 8)"
export OPENROUTER_API_KEY="sk-or-v1-your-key-here"
export ANTHROPIC_API_KEY="sk-ant-your-key-here"
export OPENAI_API_KEY="sk-your-key-here"

# Allow direnv to load
# $ direnv allow ~/.dotfiles
```

### Container Configuration
```bash
# b00t secure pattern: environment variables passed through
podman run --replace --name litellm-proxy \
    --rm -d \
    -v {{justfile_directory()}}/_b00t_generated.yaml:/app/config.yaml \
    -p 4000:4000 \
    -e LITELLM_MASTER_KEY \
    -e OPENROUTER_API_KEY \
    -e ANTHROPIC_API_KEY \
    -e OPENAI_API_KEY \
    ghcr.io/berriai/litellm:main-latest \
    --config /app/config.yaml --detailed_debug
```

## Working Directory Patterns

### Just Recipe Execution
Recipes run in the justfile's directory by default, enabling simple path resolution:

```bash
# ✅ Correct: Uses justfile working directory
_scan-models:
    #!/usr/bin/env bash
    model_files=(*.ai_model.toml)  # Finds files in same directory
```

### Path Functions
```bash
# Available Just path functions
echo "Justfile: {{justfile_directory()}}"      # /path/to/_b00t_
echo "Parent: {{parent_directory(justfile_directory())}}"  # /path/to
echo "Invocation: {{invocation_directory()}}"  # Where just was called
```

## Integration Workflow

### 1. Model Registration
Create AI model datums in `_b00t_/` directory:

```bash
# Manual creation
cat > gpt-4o.ai_model.toml << EOF
[b00t]
name = "gpt-4o"
type = "ai_model"
hint = "OpenAI GPT-4o - multimodal flagship model"

[ai_model]
provider = "openai"
size = "large"
capabilities = ["chat", "vision", "function_calling"]
litellm_model = "openai/gpt-4o"
api_key_env = "OPENAI_API_KEY"
rpm_limit = 120
enabled = true
EOF
```

### 2. Configuration Generation
```bash
# Generate LiteLLM config from all datums
just --justfile litellm.just config
```

### 3. Environment Setup (b00t Standard)
```bash
# Add API keys to ~/.dotfiles/.envrc (one-time setup)
echo 'export OPENROUTER_API_KEY="sk-or-v1-your-key-here"' >> ~/.dotfiles/.envrc
echo 'export LITELLM_MASTER_KEY="sk-b00t-$(uuidgen | head -c 8)"' >> ~/.dotfiles/.envrc

# Allow direnv to load environment
direnv allow ~/.dotfiles

# Verify environment loaded
just --justfile litellm.just init
```

### 4. Proxy Startup
```bash
# Start proxy (auto-generates config from datums)
just --justfile litellm.just s
```

### 5. Testing & Validation
```bash
# Health check + completion test
just --justfile litellm.just test

# Manual API test with OpenRouter model
curl -X POST http://localhost:4000/chat/completions \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $LITELLM_MASTER_KEY" \
    -d '{"model": "gpt-4o-mini", "messages": [{"role": "user", "content": "Hello from b00t!"}], "max_tokens": 50}'

# Python SDK test
python3 demo_litellm_test.py
```

## b00t Patterns Applied

### 1. Datum Architecture
- **Abstract Schema**: `datum_ai_model.rs` defines structure
- **TOML Data**: Individual model configurations in files
- **No Recompilation**: Add/remove models without code changes

### 2. Security First
- **Environment Variables**: No hardcoded API keys
- **Template Generation**: Safe .env creation
- **Permissions**: Secure file permissions (`chmod 600`)

### 3. Tool Integration
- **Container Runtime**: Uses podman (b00t standard)
- **Configuration Management**: Just recipes for automation
- **Path Resolution**: Proper working directory handling

### 4. Operational Excellence
- **Health Checks**: Automated proxy testing
- **Log Access**: Easy log viewing with `logs` command
- **Clean Shutdown**: Proper container lifecycle management

## Common Patterns

### Model Size Classification
```toml
# sm0l models (fast, efficient)
[ai_model]
size = "small"
rpm_limit = 1000

# ch0nky models (powerful, expensive)  
[ai_model]
size = "large"
rpm_limit = 60
```

### Provider-Specific Configuration
```toml
# Azure OpenAI requires additional config
[ai_model]
provider = "azure"
litellm_model = "azure/gpt-4o"
api_key_env = "AZURE_API_KEY"
api_base = "https://your-endpoint.openai.azure.com/"
```

### Capability Tagging
```toml
[ai_model]
capabilities = ["chat", "vision", "function_calling", "reasoning"]
```

## Troubleshooting

### Path Resolution Issues
```bash
# ❌ Wrong: Trying to escape justfile directory
model_files=({{parent_directory(justfile_directory())}}/*.toml)

# ✅ Right: Use justfile's working directory
model_files=(*.ai_model.toml)
```

### Recipe Dependencies
```bash
# ❌ Wrong: Direct recipe calls fail
just _scan-models > config.yaml

# ✅ Right: Specify justfile explicitly  
just --justfile {{justfile()}} _scan-models > config.yaml
```

### Container Issues
```bash
# Check container status
podman ps --filter name=litellm-proxy

# View container logs
podman logs litellm-proxy

# Clean restart
just --justfile litellm.just clean
just --justfile litellm.just run
```

## Future Enhancements

### Provider Registration System
- Dynamic model discovery from provider APIs
- Automatic capability detection
- Cost optimization routing

### MCP Integration
- b00t-mcp server for model management
- Claude Code integration
- Agent-driven model selection

### Monitoring & Analytics
- Usage tracking per model
- Cost analysis
- Performance metrics

🤓 **Remember**: LiteLLM is a proxy, not a model host. You still need valid API keys for each provider you configure.