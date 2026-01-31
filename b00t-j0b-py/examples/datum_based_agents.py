"""Examples using datum-based agent configuration (DRY approach).

This demonstrates the DRY approach where all provider configuration
comes from b00t datums (TOML files) via Rust (PyO3).

No hardcoded provider logic in Python!
"""

from b00t_j0b_py import (
    DatumProvider,
    create_agent_from_datum,
    adk_agent_job,
    get_queue,
)


# ==========================================
# Example 1: Simple Agent from Datum
# ==========================================

def qwen_agent_from_datum():
    """Create Qwen agent directly from datum."""

    # All configuration comes from ~/.dotfiles/_b00t_/qwen-2.5-72b.ai_model.toml
    agent_config = create_agent_from_datum(
        "qwen-2.5-72b",
        task="Research quantum computing trends",
        tools=["web_search", "calculator"],
        temperature=0.7,
    )

    queue = get_queue()
    job = queue.enqueue(
        adk_agent_job,
        agent_config_dict=agent_config,
        task="Research quantum computing trends in 2025",
    )

    print(f"✅ Qwen agent job: {job.id}")
    return job


# ==========================================
# Example 2: List Available Models
# ==========================================

def list_available_models():
    """List all models from datums."""

    providers = DatumProvider.list_available_providers()
    print(f"\n📦 Available Providers: {providers}")

    models = DatumProvider.list_available_models()
    print(f"\n🤖 Available Models: {models}")

    # Check which providers have env vars set
    print("\n🔑 Provider Status:")
    for provider in providers:
        try:
            result = DatumProvider(f"{provider}-test").validate_env()
            is_valid, missing = result if isinstance(result, tuple) else (result["available"], result.get("missing_env_vars", []))

            if is_valid:
                print(f"  ✅ {provider}: ready")
            else:
                print(f"  ❌ {provider}: missing {missing}")
        except:
            # Provider doesn't have a test model, check directly
            import b00t_py
            try:
                status = b00t_py.check_provider_env(provider)
                if status["available"]:
                    print(f"  ✅ {provider}: ready")
                else:
                    print(f"  ❌ {provider}: missing {status['missing_env_vars']}")
            except:
                print(f"  ⚠️  {provider}: unable to check")


# ==========================================
# Example 3: Agent Self-Selection by Capability
# ==========================================

def agent_selects_best_model():
    """Let agent select best model for task based on capabilities."""

    # Agent wants reasoning capability, prefers low cost
    model = DatumProvider.select_model_by_capability(
        capability="reasoning",
        prefer_local=False,
        max_cost=1.0,  # Max $1 per 1K tokens
    )

    if model:
        print(f"\n🎯 Agent selected model: {model}")

        agent_config = create_agent_from_datum(
            model,
            task="Solve complex math problem",
            tools=["calculator", "code_interpreter"],
        )

        queue = get_queue()
        job = queue.enqueue(
            adk_agent_job,
            agent_config_dict=agent_config,
            task="Prove the Pythagorean theorem step by step",
        )

        print(f"✅ Job enqueued: {job.id}")
        return job
    else:
        print("❌ No suitable model found")


# ==========================================
# Example 4: Multi-Provider Ensemble from Datums
# ==========================================

def multi_provider_ensemble():
    """Create ensemble using different providers from datums."""
    from b00t_j0b_py import multi_agent_coordination_job

    # Coordinator
    coordinator_config = create_agent_from_datum(
        "claude-3-5-sonnet",  # From anthropic.ai.toml
        task="Coordinate research",
    )

    # Sub-agents from different providers
    sub_agents = [
        # Qwen from OpenRouter
        create_agent_from_datum(
            "qwen-2.5-72b",
            task="Research",
            tools=["web_search"],
        ),

        # Claude from Anthropic
        create_agent_from_datum(
            "claude-3-5-sonnet",
            task="Analysis",
            tools=["data_analyzer"],
        ),

        # Local Ollama model (if available)
        # create_agent_from_datum(
        #     "llama-3-8b-instruct",
        #     task="Summary",
        #     tools=["markdown_formatter"],
        # ),
    ]

    queue = get_queue()
    job = queue.enqueue(
        multi_agent_coordination_job,
        coordinator_config_dict=coordinator_config,
        sub_agent_configs=sub_agents,
        task="Research, analyze, and summarize AI safety developments",
        coordination_strategy="sequential",
    )

    print(f"✅ Ensemble job: {job.id}")
    return job


# ==========================================
# Example 5: Validate Environment
# ==========================================

def validate_environment():
    """Validate all required environment variables via datums."""

    models = DatumProvider.list_available_models()

    print("\n🔍 Environment Validation:\n")

    for model in models:
        try:
            provider = DatumProvider(model)
            is_valid, missing = provider.validate_env()

            if is_valid:
                print(f"  ✅ {model}: ready ({provider.provider})")
            else:
                print(f"  ❌ {model}: missing env vars: {missing}")
                print(f"     Set in .envrc: export {missing[0]}=...")

        except Exception as e:
            print(f"  ⚠️  {model}: {e}")

    print("\n💡 Tip: Use direnv and .envrc to manage environment variables")
    print("Example .envrc:")
    print("  export OPENROUTER_API_KEY=sk-or-v1-...")
    print("  export ANTHROPIC_API_KEY=sk-ant-...")
    print("  export HUGGINGFACE_TOKEN=hf_...")


# ==========================================
# Example 6: Dynamic Model Selection for Cost Optimization
# ==========================================

def cost_optimized_workflow():
    """Agent selects models based on cost and task complexity."""

    # Simple tasks -> cheap/local models
    simple_task_model = DatumProvider.select_model_by_capability(
        capability="chat",
        prefer_local=True,  # Try Ollama first
    )

    # Complex tasks -> powerful models
    complex_task_model = DatumProvider.select_model_by_capability(
        capability="reasoning",
        prefer_local=False,
    )

    print(f"\n💰 Cost-optimized model selection:")
    print(f"  Simple tasks: {simple_task_model or 'none available'}")
    print(f"  Complex tasks: {complex_task_model or 'none available'}")

    # Use appropriate model for each task
    if simple_task_model:
        config = create_agent_from_datum(
            simple_task_model,
            task="Simple task",
        )
        print(f"  ✅ Would use {simple_task_model} for simple tasks")

    if complex_task_model:
        config = create_agent_from_datum(
            complex_task_model,
            task="Complex task",
        )
        print(f"  ✅ Would use {complex_task_model} for complex tasks")


# ==========================================
# Main
# ==========================================

if __name__ == "__main__":
    import sys

    # Check if b00t_py is available
    try:
        import b00t_py
        print(f"✅ b00t_py available (version: {b00t_py.version()})")
    except ImportError:
        print("❌ b00t_py not available!")
        print("   Install: cd ../b00t-py && maturin develop")
        sys.exit(1)

    print("\n" + "="*60)
    print("Datum-Based Agent Examples (DRY Approach)")
    print("="*60)

    # Run examples
    print("\n📋 Example 1: List Available Models")
    list_available_models()

    print("\n🔍 Example 2: Validate Environment")
    validate_environment()

    print("\n💰 Example 3: Cost-Optimized Selection")
    cost_optimized_workflow()

    # Uncomment to actually run jobs (requires Redis and RQ worker)
    # print("\n🚀 Example 4: Create Qwen Agent from Datum")
    # qwen_agent_from_datum()

    # print("\n🎯 Example 5: Agent Self-Selection")
    # agent_selects_best_model()

    print("\n✅ Done! All configuration came from b00t datums (Rust-backed)")
    print("   No hardcoded provider logic in Python!")
