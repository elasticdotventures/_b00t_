# Multi-Provider LLM Support for Google ADK

Google ADK integration in b00t-j0b-py supports **multiple LLM providers**, enabling you to use agents powered by different models beyond Gemini.

## Supported Providers

| Provider | Models | Use Case |
|----------|--------|----------|
| **Gemini** (default) | gemini-2.0-flash-exp, gemini-2.5-pro | General purpose, fast responses |
| **OpenRouter** | Qwen, Kimi, DeepSeek, Claude, GPT, etc. | Multi-model gateway with 200+ models |
| **OpenAI** | GPT-4, GPT-4 Turbo, GPT-3.5 | General purpose, code generation |
| **Anthropic** | Claude 3.5 Sonnet, Claude 3 Opus | Long context, analysis, writing |
| **OpenAI-Compatible** | Any custom API | Self-hosted or proprietary models |

## Quick Start

### 1. OpenRouter with Qwen 2.5

**Qwen 2.5** - Alibaba's flagship model, excellent for reasoning and multilingual tasks.

```python
from b00t_j0b_py import AgentConfig, ModelProvider, adk_agent_job, get_queue

agent = AgentConfig(
    name="qwen-researcher",
    description="Research agent powered by Qwen 2.5",

    # OpenRouter configuration
    provider=ModelProvider.OPENROUTER,
    api_base="https://openrouter.ai/api/v1",
    model_name="qwen/qwen-2.5-72b-instruct",

    temperature=0.7,
    max_tokens=4096,
    tools=["web_search", "calculator"],
)

queue = get_queue()
job = queue.enqueue(
    adk_agent_job,
    agent_config_dict=agent.to_dict(),
    task="Research quantum computing developments in 2025",
)
```

### 2. OpenRouter with Kimi K2

**Kimi K2** - Moonshot AI's model with massive context window (200K+ tokens).

```python
agent = AgentConfig(
    name="kimi-analyzer",
    description="Long-context analysis with Kimi K2",

    provider=ModelProvider.OPENROUTER,
    api_base="https://openrouter.ai/api/v1",
    model_name="moonshot/kimi-k2",

    temperature=0.5,
    max_tokens=8192,
    tools=["document_processor", "code_analyzer"],
)

job = queue.enqueue(
    adk_agent_job,
    agent_config_dict=agent.to_dict(),
    task="Analyze this entire codebase and document architecture",
    context={"repo": "https://github.com/example/large-repo"},
)
```

### 3. OpenRouter with DeepSeek R1

**DeepSeek R1** - Specialized in complex reasoning and mathematical tasks.

```python
agent = AgentConfig(
    name="deepseek-reasoner",
    description="Complex reasoning with DeepSeek R1",

    provider=ModelProvider.OPENROUTER,
    api_base="https://openrouter.ai/api/v1",
    model_name="deepseek/deepseek-r1",

    temperature=0.3,  # Lower for precise reasoning
    max_tokens=4096,
    tools=["code_interpreter", "math_solver"],
)

job = queue.enqueue(
    adk_agent_job,
    agent_config_dict=agent.to_dict(),
    task="Prove the Riemann hypothesis step-by-step",
)
```

### 4. Anthropic Claude

**Claude 3.5 Sonnet** - Excellent for analysis, writing, and long documents.

```python
agent = AgentConfig(
    name="claude-analyst",
    description="Data analysis with Claude Sonnet",

    provider=ModelProvider.ANTHROPIC,
    model_name="claude-3-5-sonnet-20241022",

    temperature=0.6,
    max_tokens=8192,
    tools=["data_analyzer", "visualizer"],
)

job = queue.enqueue(
    adk_agent_job,
    agent_config_dict=agent.to_dict(),
    task="Analyze sales data and identify trends",
)
```

### 5. OpenAI GPT-4

**GPT-4 Turbo** - General purpose with strong coding abilities.

```python
agent = AgentConfig(
    name="gpt4-developer",
    description="Code assistant with GPT-4",

    provider=ModelProvider.OPENAI,
    model_name="gpt-4-turbo-preview",

    temperature=0.7,
    max_tokens=4096,
    tools=["code_interpreter", "file_browser"],
)

job = queue.enqueue(
    adk_agent_job,
    agent_config_dict=agent.to_dict(),
    task="Refactor this code to improve performance",
)
```

## Environment Variables

Set API keys via environment variables (recommended for production):

```bash
# OpenRouter (supports 200+ models)
export OPENROUTER_API_KEY="sk-or-v1-..."

# OpenAI
export OPENAI_API_KEY="sk-..."

# Anthropic
export ANTHROPIC_API_KEY="sk-ant-..."

# Google (Gemini)
export GEMINI_API_KEY="..."
```

When `api_key=None` in config, the integration automatically checks for `{PROVIDER}_API_KEY` environment variable.

## Multi-Model Ensemble

Combine multiple models for robust results:

```python
from b00t_j0b_py import multi_agent_coordination_job

# Coordinator
coordinator = AgentConfig(
    name="ensemble-coordinator",
    provider=ModelProvider.GEMINI,
    model_name="gemini-2.0-flash-exp",
)

# Specialized sub-agents
sub_agents = [
    # Qwen for research
    AgentConfig(
        name="qwen-researcher",
        provider=ModelProvider.OPENROUTER,
        api_base="https://openrouter.ai/api/v1",
        model_name="qwen/qwen-2.5-72b-instruct",
        tools=["web_search"],
    ).to_dict(),

    # DeepSeek for reasoning
    AgentConfig(
        name="deepseek-reasoner",
        provider=ModelProvider.OPENROUTER,
        api_base="https://openrouter.ai/api/v1",
        model_name="deepseek/deepseek-r1",
        tools=["calculator"],
    ).to_dict(),

    # Claude for writing
    AgentConfig(
        name="claude-writer",
        provider=ModelProvider.ANTHROPIC,
        model_name="claude-3-5-sonnet-20241022",
        tools=["markdown_formatter"],
    ).to_dict(),
]

job = queue.enqueue(
    multi_agent_coordination_job,
    coordinator_config_dict=coordinator.to_dict(),
    sub_agent_configs=sub_agents,
    task="Research, analyze, and write comprehensive report on AI safety",
    coordination_strategy="sequential",
)
```

## Popular OpenRouter Models

OpenRouter provides access to 200+ models through a single API:

### Reasoning & Analysis
```python
# DeepSeek R1 - Advanced reasoning
model_name="deepseek/deepseek-r1"

# Qwen 2.5 - Strong reasoning & multilingual
model_name="qwen/qwen-2.5-72b-instruct"

# Claude 3.5 Sonnet via OpenRouter
model_name="anthropic/claude-3.5-sonnet"
```

### Long Context
```python
# Kimi K2 - 200K+ context window
model_name="moonshot/kimi-k2"

# Gemini 1.5 Pro via OpenRouter - 1M context
model_name="google/gemini-pro-1.5"

# Claude 3 Opus - 200K context
model_name="anthropic/claude-3-opus"
```

### Coding Specialists
```python
# DeepSeek Coder
model_name="deepseek/deepseek-coder-33b-instruct"

# Qwen Coder
model_name="qwen/qwencoder-32b"

# CodeLlama
model_name="codellama/codellama-70b-instruct"
```

### Fast & Cost-Effective
```python
# Gemini Flash - Very fast
model_name="google/gemini-flash-1.5"

# GPT-3.5 Turbo - Cost effective
model_name="openai/gpt-3.5-turbo"

# Mistral 7B - Open source, fast
model_name="mistralai/mistral-7b-instruct"
```

## Configuration Patterns

### Pattern 1: Hardcoded API Key (Development)

```python
agent = AgentConfig(
    provider=ModelProvider.OPENROUTER,
    api_base="https://openrouter.ai/api/v1",
    api_key="sk-or-v1-...",  # ⚠️ Not recommended for production
    model_name="qwen/qwen-2.5-72b-instruct",
)
```

### Pattern 2: Environment Variable (Production)

```python
# Reads from OPENROUTER_API_KEY env var
agent = AgentConfig(
    provider=ModelProvider.OPENROUTER,
    api_base="https://openrouter.ai/api/v1",
    api_key=None,  # Falls back to env var
    model_name="qwen/qwen-2.5-72b-instruct",
)
```

### Pattern 3: Dynamic Configuration

```python
import os

def create_agent(provider_name: str, model: str):
    """Dynamically create agent from config."""

    provider_map = {
        "openrouter": ModelProvider.OPENROUTER,
        "openai": ModelProvider.OPENAI,
        "anthropic": ModelProvider.ANTHROPIC,
        "gemini": ModelProvider.GEMINI,
    }

    api_base_map = {
        "openrouter": "https://openrouter.ai/api/v1",
        "openai": None,  # Uses default
        "anthropic": None,
        "gemini": None,
    }

    return AgentConfig(
        name=f"{provider_name}-agent",
        description=f"Agent powered by {model}",
        provider=provider_map[provider_name],
        api_base=api_base_map[provider_name],
        model_name=model,
        api_key=None,  # From env
    )

# Usage
agent = create_agent("openrouter", "qwen/qwen-2.5-72b-instruct")
```

## Custom OpenAI-Compatible APIs

Use any OpenAI-compatible API (self-hosted, LM Studio, vLLM, etc.):

```python
agent = AgentConfig(
    name="custom-model",
    provider=ModelProvider.OPENAI_COMPATIBLE,
    api_base="http://localhost:8000/v1",  # Your local API
    api_key="not-needed",  # Optional for local
    model_name="llama-3-70b",
    temperature=0.7,
)
```

### Examples of OpenAI-Compatible APIs:
- **LM Studio** - `http://localhost:1234/v1`
- **vLLM** - `http://localhost:8000/v1`
- **Ollama** (with adapter) - `http://localhost:11434/v1`
- **Text Generation WebUI** - `http://localhost:5000/v1`
- **LocalAI** - `http://localhost:8080/v1`

## Model Selection Guide

### When to Use Qwen 2.5
- ✅ Multilingual tasks (Chinese, English, etc.)
- ✅ Complex reasoning
- ✅ Code generation
- ✅ Cost-effective for large workloads

### When to Use Kimi K2
- ✅ Extremely long documents (200K+ tokens)
- ✅ Codebase analysis
- ✅ Book summarization
- ✅ Long conversation history

### When to Use DeepSeek R1
- ✅ Mathematical proofs
- ✅ Complex logical reasoning
- ✅ Step-by-step problem solving
- ✅ Code optimization

### When to Use Claude 3.5
- ✅ Writing & editing
- ✅ Data analysis
- ✅ Long-form content
- ✅ Nuanced conversation

### When to Use GPT-4
- ✅ General purpose tasks
- ✅ Strong coding abilities
- ✅ Creative writing
- ✅ API availability & ecosystem

### When to Use Gemini
- ✅ Multimodal tasks (image + text)
- ✅ Fast responses
- ✅ Free tier available
- ✅ Google ecosystem integration

## Pricing Comparison (via OpenRouter)

| Model | Input ($/1M tokens) | Output ($/1M tokens) | Context Window |
|-------|---------------------|----------------------|----------------|
| Qwen 2.5 72B | $0.35 | $0.40 | 32K |
| Kimi K2 | $0.30 | $0.90 | 200K+ |
| DeepSeek R1 | $0.55 | $2.19 | 64K |
| Claude 3.5 Sonnet | $3.00 | $15.00 | 200K |
| GPT-4 Turbo | $10.00 | $30.00 | 128K |
| Gemini Flash | $0.075 | $0.30 | 1M |

*Prices subject to change. Check OpenRouter for current pricing.*

## Advanced: Provider-Specific Features

### OpenRouter Features

```python
# Include site URL for attribution
agent = AgentConfig(
    provider=ModelProvider.OPENROUTER,
    api_base="https://openrouter.ai/api/v1",
    model_name="qwen/qwen-2.5-72b-instruct",
)

# Add OpenRouter-specific headers via context
context = {
    "openrouter_headers": {
        "HTTP-Referer": "https://yourapp.com",
        "X-Title": "YourApp Name",
    }
}
```

### Model Fallbacks

```python
# Try Qwen first, fallback to GPT-4
primary_agent = AgentConfig(
    provider=ModelProvider.OPENROUTER,
    model_name="qwen/qwen-2.5-72b-instruct",
)

fallback_agent = AgentConfig(
    provider=ModelProvider.OPENAI,
    model_name="gpt-4-turbo-preview",
)

# Implement retry logic in job
def robust_agent_job(task):
    try:
        return adk_agent_job(primary_agent.to_dict(), task)
    except Exception as e:
        print(f"Primary failed: {e}, using fallback")
        return adk_agent_job(fallback_agent.to_dict(), task)
```

## Troubleshooting

### Issue: "Invalid API key"
```bash
# Check environment variable
echo $OPENROUTER_API_KEY

# Set if missing
export OPENROUTER_API_KEY="sk-or-v1-..."
```

### Issue: "Model not found"
```python
# Verify model name format
# Correct: "qwen/qwen-2.5-72b-instruct"
# Wrong: "qwen-2.5-72b-instruct"

# Check available models at:
# https://openrouter.ai/models
```

### Issue: Rate limiting
```python
# Add retry logic or use lower-cost model
agent = AgentConfig(
    provider=ModelProvider.OPENROUTER,
    model_name="qwen/qwen-2.5-7b-instruct",  # Smaller, faster
    timeout=600,  # Increase timeout
)
```

## Examples

See `examples/multi_provider_agents.py` for complete working examples of:
- OpenRouter with Qwen, Kimi, DeepSeek
- OpenAI GPT-4
- Anthropic Claude
- Multi-model ensembles
- Custom API configuration

## References

- [OpenRouter Models](https://openrouter.ai/models)
- [OpenRouter Pricing](https://openrouter.ai/pricing)
- [Qwen Documentation](https://qwenlm.github.io/)
- [Kimi K2 Info](https://www.moonshot.cn/)
- [DeepSeek Models](https://www.deepseek.com/)
- [Google ADK Documentation](https://github.com/google/adk-python)

---

**🚀 Ready to use multiple LLM providers with Google ADK!**

Install dependencies:
```bash
pip install b00t-j0b-py[adk]
pip install openai anthropic  # For non-Gemini providers
```

Get API keys:
- OpenRouter: https://openrouter.ai/keys
- OpenAI: https://platform.openai.com/api-keys
- Anthropic: https://console.anthropic.com/
