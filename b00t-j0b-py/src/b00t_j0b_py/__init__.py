"""b00t-j0b-py: Web crawler job system for b00t ecosystem using Redis RQ.

Provides:
- Web crawling jobs with Redis-backed tracking
- Google ADK agent integration for AI agents as jobs
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

# ADK integration
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
    create_agent_from_datum,
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
    # ADK Integration
    "ADKAgentRunner",
    "AgentConfig",
    "AgentExecutionContext",
    "AgentStatus",
    "ModelProvider",
    "adk_agent_job",
    "multi_agent_coordination_job",
    # Datum Provider (DRY)
    "DatumProvider",
    "create_agent_from_datum",
    # RQ Utils
    "get_queue",
    "get_all_queues",
    "start_worker",
    "get_job_status",
    "get_queue_info",
    "clear_all_queues",
]
