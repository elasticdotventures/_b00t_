"""Examples using Google ADK with multiple LLM providers.

Demonstrates how to use ADK agents with:
- OpenRouter (Qwen, Kimi, DeepSeek, etc.)
- OpenAI GPT models
- Anthropic Claude
- Google Gemini (default)
"""

from b00t_j0b_py import AgentConfig, ModelProvider, adk_agent_job
from b00t_j0b_py.rq_integration import get_queue


# ==========================================
# Example 1: OpenRouter with Qwen 2.5
# ==========================================

def qwen_research_agent():
    """Use Qwen 2.5 via OpenRouter for research tasks."""

    agent = AgentConfig(
        name="qwen-researcher",
        description="Research agent powered by Qwen 2.5 72B",

        # OpenRouter configuration
        provider=ModelProvider.OPENROUTER,
        api_base="https://openrouter.ai/api/v1",
        model_name="qwen/qwen-2.5-72b-instruct",

        # Model parameters
        temperature=0.7,
        max_tokens=4096,

        # Tools
        tools=["web_search", "calculator"],
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent.to_dict(),
        task="Research the latest developments in quantum computing and summarize key breakthroughs",
    )

    print(f"Qwen research job: {job.id}")
    return job


# ==========================================
# Example 2: OpenRouter with Kimi K2
# ==========================================

def kimi_long_context_agent():
    """Use Kimi K2 via OpenRouter for long-context tasks."""

    agent = AgentConfig(
        name="kimi-analyzer",
        description="Long-context analysis with Kimi K2",

        # Kimi via OpenRouter
        provider=ModelProvider.OPENROUTER,
        api_base="https://openrouter.ai/api/v1",
        model_name="moonshot/kimi-k2",  # 200K+ context window

        temperature=0.5,
        max_tokens=8192,

        tools=["document_processor", "summarizer"],
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent.to_dict(),
        task="Analyze this codebase and identify architectural patterns",
        context={"codebase_url": "https://github.com/example/repo"},
    )

    print(f"Kimi analysis job: {job.id}")
    return job


# ==========================================
# Example 3: OpenRouter with DeepSeek R1
# ==========================================

def deepseek_reasoning_agent():
    """Use DeepSeek R1 via OpenRouter for complex reasoning."""

    agent = AgentConfig(
        name="deepseek-reasoner",
        description="Complex reasoning with DeepSeek R1",

        provider=ModelProvider.OPENROUTER,
        api_base="https://openrouter.ai/api/v1",
        model_name="deepseek/deepseek-r1",

        temperature=0.3,  # Lower temp for reasoning
        max_tokens=4096,

        tools=["code_interpreter", "math_solver"],
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent.to_dict(),
        task="Prove the following mathematical theorem step-by-step",
    )

    print(f"DeepSeek reasoning job: {job.id}")
    return job


# ==========================================
# Example 4: OpenAI GPT-4
# ==========================================

def openai_gpt4_agent():
    """Use OpenAI GPT-4 directly."""

    agent = AgentConfig(
        name="gpt4-assistant",
        description="General-purpose assistant with GPT-4",

        provider=ModelProvider.OPENAI,
        model_name="gpt-4-turbo-preview",

        temperature=0.7,
        max_tokens=4096,

        tools=["web_search", "code_interpreter", "file_browser"],
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent.to_dict(),
        task="Help me debug this Python code and suggest improvements",
    )

    print(f"GPT-4 job: {job.id}")
    return job


# ==========================================
# Example 5: Anthropic Claude
# ==========================================

def claude_sonnet_agent():
    """Use Anthropic Claude Sonnet."""

    agent = AgentConfig(
        name="claude-analyst",
        description="Data analysis with Claude Sonnet",

        provider=ModelProvider.ANTHROPIC,
        model_name="claude-3-5-sonnet-20241022",

        temperature=0.6,
        max_tokens=8192,

        tools=["data_analyzer", "visualizer"],
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent.to_dict(),
        task="Analyze this dataset and identify trends",
    )

    print(f"Claude job: {job.id}")
    return job


# ==========================================
# Example 6: Multi-Model Ensemble
# ==========================================

def multi_model_ensemble():
    """Use multiple models together for robust results."""
    from b00t_j0b_py import multi_agent_coordination_job

    # Coordinator (Gemini)
    coordinator = AgentConfig(
        name="ensemble-coordinator",
        description="Coordinates multiple model responses",
        provider=ModelProvider.GEMINI,
        model_name="gemini-2.0-flash-exp",
    )

    # Sub-agents with different models
    sub_agents = [
        # Qwen for research
        AgentConfig(
            name="qwen-researcher",
            description="Research specialist",
            provider=ModelProvider.OPENROUTER,
            api_base="https://openrouter.ai/api/v1",
            model_name="qwen/qwen-2.5-72b-instruct",
            tools=["web_search"],
        ).to_dict(),

        # DeepSeek for reasoning
        AgentConfig(
            name="deepseek-reasoner",
            description="Reasoning specialist",
            provider=ModelProvider.OPENROUTER,
            api_base="https://openrouter.ai/api/v1",
            model_name="deepseek/deepseek-r1",
            tools=["calculator"],
        ).to_dict(),

        # Claude for writing
        AgentConfig(
            name="claude-writer",
            description="Writing specialist",
            provider=ModelProvider.ANTHROPIC,
            model_name="claude-3-5-sonnet-20241022",
            tools=["markdown_formatter"],
        ).to_dict(),
    ]

    queue = get_queue()
    job = queue.enqueue(
        multi_agent_coordination_job,
        coordinator_config_dict=coordinator.to_dict(),
        sub_agent_configs=sub_agents,
        task="Research AI safety, reason about implications, and write a comprehensive report",
        coordination_strategy="sequential",
    )

    print(f"Ensemble job: {job.id}")
    return job


# ==========================================
# Example 7: Using Environment Variables
# ==========================================

def agent_with_env_vars():
    """Agent that gets API key from environment variable.

    Set environment variables:
        export OPENROUTER_API_KEY="sk-or-v1-..."
    """

    agent = AgentConfig(
        name="env-agent",
        description="Agent using env vars for API key",

        provider=ModelProvider.OPENROUTER,
        api_base="https://openrouter.ai/api/v1",
        model_name="qwen/qwen-2.5-72b-instruct",

        # api_key is None - will fall back to OPENROUTER_API_KEY env var
        api_key=None,

        temperature=0.7,
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent.to_dict(),
        task="Example task using env var for authentication",
    )

    return job


# ==========================================
# Example 8: Custom OpenAI-Compatible API
# ==========================================

def custom_api_agent():
    """Use any OpenAI-compatible API endpoint."""

    agent = AgentConfig(
        name="custom-api-agent",
        description="Agent using custom API",

        provider=ModelProvider.OPENAI_COMPATIBLE,
        api_base="https://your-custom-api.com/v1",  # Your API endpoint
        model_name="your-model-name",
        api_key="your-api-key",

        temperature=0.7,
        max_tokens=2048,
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent.to_dict(),
        task="Task for custom API",
    )

    return job


# ==========================================
# Main
# ==========================================

if __name__ == "__main__":
    import os

    # Check for required API keys
    if not os.getenv("OPENROUTER_API_KEY"):
        print("⚠️  Set OPENROUTER_API_KEY to run OpenRouter examples")

    if not os.getenv("OPENAI_API_KEY"):
        print("⚠️  Set OPENAI_API_KEY to run OpenAI examples")

    if not os.getenv("ANTHROPIC_API_KEY"):
        print("⚠️  Set ANTHROPIC_API_KEY to run Anthropic examples")

    # Run examples
    print("\n🚀 Launching multi-provider agents...\n")

    # Example 1: Qwen via OpenRouter
    qwen_research_agent()

    # Example 2: Kimi via OpenRouter
    kimi_long_context_agent()

    # Example 3: DeepSeek via OpenRouter
    deepseek_reasoning_agent()

    # Example 6: Multi-model ensemble
    multi_model_ensemble()

    print("\n✅ Jobs enqueued! Monitor with: rq info")
