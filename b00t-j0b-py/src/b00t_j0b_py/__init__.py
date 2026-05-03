"""b00t-j0b-py: Web crawler job system for b00t ecosystem using Redis RQ.

Provides:
- Web crawling jobs with Redis-backed tracking
- Pydantic-AI agent integration for production AI agents (RECOMMENDED)
- Google ADK agent integration (DEPRECATED - use pydantic-ai)
- Cleanup and maintenance jobs for Redis data
- Multi-agent coordination via RQ
- Datum-based provider configuration (DRY - uses Rust via PyO3)
"""

__version__ = "0.1.0"
__author__ = "elasticdotventures"

# Core job functions
from .jobs import (
    crawl_url_job,
    digest_url_job,
    process_binary_content_job,
    cleanup_old_data_job,
)

# Pydantic-AI integration (RECOMMENDED - production-ready)
from .pydantic_ai_integration import (
    create_agent_from_datum as create_pydantic_agent,
    get_model_string_from_datum,
    select_best_model,
    list_available_models,
    list_available_providers,
)

from .pydantic_ai_jobs import (
    pydantic_agent_job,
    auto_select_agent_job,
    multi_agent_pydantic_job,
)

# ADK integration (DEPRECATED - use pydantic-ai)
from .adk_integration import (
    ADKAgentRunner,
    AgentConfig,
    AgentExecutionContext,
    AgentStatus,
    ModelProvider,
    adk_agent_job,
    multi_agent_coordination_job,
)

# Datum provider (DRY - uses Rust via PyO3)
from .datum_provider import (
    DatumProvider,
    create_agent_from_datum as create_adk_agent_from_datum,
)

# RQ integration utilities
from .rq_integration import (
    get_queue,
    get_all_queues,
    start_worker,
    get_job_status,
    get_queue_info,
    clear_all_queues,
)

__all__ = [
    # Jobs
    "crawl_url_job",
    "digest_url_job",
    "process_binary_content_job",
    "cleanup_old_data_job",
    # Pydantic-AI Integration (RECOMMENDED)
    "create_pydantic_agent",
    "get_model_string_from_datum",
    "select_best_model",
    "list_available_models",
    "list_available_providers",
    "pydantic_agent_job",
    "auto_select_agent_job",
    "multi_agent_pydantic_job",
    # ADK Integration (DEPRECATED)
    "ADKAgentRunner",
    "AgentConfig",
    "AgentExecutionContext",
    "AgentStatus",
    "ModelProvider",
    "adk_agent_job",
    "multi_agent_coordination_job",
    # Datum Provider (DRY)
    "DatumProvider",
    "create_adk_agent_from_datum",
    # RQ Utils
    "get_queue",
    "get_all_queues",
    "start_worker",
    "get_job_status",
    "get_queue_info",
    "clear_all_queues",
]
