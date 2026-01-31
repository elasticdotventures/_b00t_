"""RQ job definitions for pydantic-ai agents.

Production-ready agent execution using pydantic-ai framework with
b00t datum system for provider discovery and validation.
"""

from typing import Dict, Any, List, Optional, Type
from rq import get_current_job
from pydantic import BaseModel
import asyncio

from .pydantic_ai_integration import (
    create_agent_from_datum,
    select_best_model,
    list_available_models,
)


def pydantic_agent_job(
    model_datum_name: str,
    task: str,
    system_prompt: Optional[str] = None,
    result_type_dict: Optional[Dict[str, Any]] = None,
    context: Optional[Dict[str, Any]] = None,
) -> Dict[str, Any]:
    """RQ job to execute pydantic-ai agent from datum.

    Args:
        model_datum_name: Name of model datum (e.g., "qwen-2.5-72b")
        task: Task description/prompt for the agent
        system_prompt: Optional system prompt
        result_type_dict: Optional Pydantic model schema for structured output
        context: Additional context for the agent

    Returns:
        Job result dictionary

    Example:
        >>> from b00t_j0b_py.rq_integration import get_queue
        >>> queue = get_queue()
        >>> job = queue.enqueue(
        ...     pydantic_agent_job,
        ...     model_datum_name="qwen-2.5-72b",
        ...     task="Research quantum computing trends",
        ...     system_prompt="You are a research assistant",
        ... )
    """
    job = get_current_job()
    job_id = job.id if job else "local"

    print(f"[{job_id}] Starting pydantic-ai agent: {model_datum_name}")
    print(f"[{job_id}] Task: {task}")

    try:
        # Create agent from datum (validates env vars)
        agent = create_agent_from_datum(
            model_datum_name,
            system_prompt=system_prompt,
            # TODO: Handle result_type_dict deserialization if needed
        )

        # Run agent async
        result = asyncio.run(agent.run(task))

        print(f"[{job_id}] Agent completed successfully")

        return {
            "status": "success",
            "model": model_datum_name,
            "task": task,
            "result": result.data if hasattr(result, 'data') else str(result),
            "job_id": job_id,
        }

    except EnvironmentError as e:
        print(f"[{job_id}] Environment error: {e}")
        return {
            "status": "error",
            "error_type": "environment",
            "error": str(e),
            "model": model_datum_name,
            "job_id": job_id,
        }

    except Exception as e:
        print(f"[{job_id}] Agent execution failed: {e}")
        return {
            "status": "error",
            "error_type": "execution",
            "error": str(e),
            "model": model_datum_name,
            "job_id": job_id,
        }


def auto_select_agent_job(
    task: str,
    capability: Optional[str] = None,
    prefer_local: bool = False,
    system_prompt: Optional[str] = None,
) -> Dict[str, Any]:
    """RQ job with automatic model selection based on task requirements.

    The agent self-selects the best model based on:
    - Required capability
    - Cost constraints
    - Local vs cloud preference
    - Available environment variables

    Args:
        task: Task description
        capability: Required capability (e.g., "reasoning", "code", "vision")
        prefer_local: Prefer local models (Ollama)
        system_prompt: Optional system prompt

    Returns:
        Job result dictionary

    Example:
        >>> job = queue.enqueue(
        ...     auto_select_agent_job,
        ...     task="Solve complex math problem",
        ...     capability="reasoning",
        ...     prefer_local=False,
        ... )
    """
    job = get_current_job()
    job_id = job.id if job else "local"

    print(f"[{job_id}] Auto-selecting model for task")
    print(f"[{job_id}] Capability: {capability}, Prefer local: {prefer_local}")

    try:
        # Agent selects best model
        model_name = select_best_model(
            capability=capability,
            prefer_local=prefer_local,
        )

        if not model_name:
            available = list_available_models()
            return {
                "status": "error",
                "error_type": "model_selection",
                "error": f"No suitable model found for capability '{capability}'",
                "available_models": available,
                "job_id": job_id,
            }

        print(f"[{job_id}] Selected model: {model_name}")

        # Execute with selected model
        return pydantic_agent_job(
            model_datum_name=model_name,
            task=task,
            system_prompt=system_prompt,
        )

    except Exception as e:
        print(f"[{job_id}] Auto-selection failed: {e}")
        return {
            "status": "error",
            "error_type": "selection",
            "error": str(e),
            "job_id": job_id,
        }


def multi_agent_pydantic_job(
    task: str,
    models: List[str],
    strategy: str = "sequential",
    system_prompt: Optional[str] = None,
) -> Dict[str, Any]:
    """Multi-agent coordination using pydantic-ai.

    Args:
        task: Task description
        models: List of model datum names
        strategy: "sequential" or "parallel"
        system_prompt: Optional system prompt for all agents

    Returns:
        Job result dictionary with all agent outputs

    Example:
        >>> job = queue.enqueue(
        ...     multi_agent_pydantic_job,
        ...     task="Research, analyze, and summarize AI safety",
        ...     models=["qwen-2.5-72b", "claude-3-5-sonnet"],
        ...     strategy="sequential",
        ... )
    """
    job = get_current_job()
    job_id = job.id if job else "local"

    print(f"[{job_id}] Multi-agent coordination ({strategy})")
    print(f"[{job_id}] Models: {models}")

    try:
        results = []

        if strategy == "sequential":
            # Execute agents one by one
            for model_name in models:
                print(f"[{job_id}] Running agent: {model_name}")

                agent_result = pydantic_agent_job(
                    model_datum_name=model_name,
                    task=task,
                    system_prompt=system_prompt,
                )

                results.append({
                    "model": model_name,
                    "result": agent_result,
                })

        elif strategy == "parallel":
            # Enqueue agents as separate jobs
            from .rq_integration import get_queue
            queue = get_queue()

            sub_jobs = []
            for model_name in models:
                sub_job = queue.enqueue(
                    pydantic_agent_job,
                    model_datum_name=model_name,
                    task=task,
                    system_prompt=system_prompt,
                )
                sub_jobs.append({
                    "model": model_name,
                    "job_id": sub_job.id,
                })

            results = sub_jobs

        else:
            raise ValueError(f"Unknown strategy: {strategy}")

        return {
            "status": "success",
            "strategy": strategy,
            "models": models,
            "results": results,
            "job_id": job_id,
        }

    except Exception as e:
        print(f"[{job_id}] Multi-agent coordination failed: {e}")
        return {
            "status": "error",
            "error": str(e),
            "job_id": job_id,
        }


__all__ = [
    "pydantic_agent_job",
    "auto_select_agent_job",
    "multi_agent_pydantic_job",
]
