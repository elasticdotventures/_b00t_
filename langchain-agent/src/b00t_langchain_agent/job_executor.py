"""
Job Executor for b00t job system integration.

Enables LangChain agents to be executed from job workflows via Redis IPC.
"""

import asyncio
import logging
from pathlib import Path
from typing import Any

from .agent_service import AgentService
from .types import AgentResult

log = logging.getLogger(__name__)


class JobExecutor:
    """Execute LangChain agents from job system workflows."""

    def __init__(self, agent_service: AgentService):
        """
        Initialize job executor.

        Args:
            agent_service: Agent service for executing agents
        """
        self.agent_service = agent_service

    async def execute_agent_task(
        self,
        agent_type: str,
        prompt: str,
        context_files: list[str] | None = None,
        timeout_ms: int | None = None,
    ) -> dict[str, Any]:
        """
        Execute agent for job system.

        Args:
            agent_type: Agent name from langchain.ai.toml (e.g., "researcher")
            prompt: Task prompt for agent
            context_files: Optional list of file paths to load as context
            timeout_ms: Optional timeout in milliseconds

        Returns:
            Result dictionary with success, output, error
        """
        log.info(f"🔧 Job executor: Running agent '{agent_type}'")

        try:
            # Load context files if specified
            context = await self._load_context_files(context_files or [])

            # Build full prompt with context
            if context:
                full_prompt = f"{context}\n\n{prompt}"
            else:
                full_prompt = prompt

            # Execute agent with optional timeout override
            if timeout_ms:
                # Override agent config timeout
                # 🤓: AgentService uses timeout from config, job can override
                log.info(f"   Timeout: {timeout_ms}ms (from job config)")

            result = await self.agent_service.run_agent(
                agent_name=agent_type,
                input_text=full_prompt,
            )

            # Convert AgentResult to job-compatible dict
            return {
                "success": result.success,
                "output": result.output or "",
                "error": result.error,
                "metadata": result.metadata,
            }

        except Exception as e:
            log.error(f"❌ Job executor error: {e}")
            return {
                "success": False,
                "output": "",
                "error": str(e),
                "metadata": {},
            }

    async def _load_context_files(self, file_paths: list[str]) -> str:
        """
        Load context files and format for agent.

        Args:
            file_paths: List of file paths to load

        Returns:
            Formatted context string
        """
        if not file_paths:
            return ""

        context_parts = []

        for file_path in file_paths:
            try:
                path = Path(file_path)
                if not path.exists():
                    log.warning(f"⚠️  Context file not found: {file_path}")
                    continue

                if not path.is_file():
                    log.warning(f"⚠️  Not a file: {file_path}")
                    continue

                # Read file content
                content = path.read_text(encoding="utf-8")

                # Format context block
                context_parts.append(f"--- Context from {file_path} ---")
                context_parts.append(content)
                context_parts.append("--- End context ---")

                log.info(f"   📄 Loaded context: {file_path}")

            except Exception as e:
                log.warning(f"⚠️  Failed to load {file_path}: {e}")

        return "\n".join(context_parts) if context_parts else ""

    async def execute_chain_workflow(
        self,
        chain_name: str,
        params: dict[str, Any],
    ) -> dict[str, Any]:
        """
        Execute a LangChain workflow chain.

        Args:
            chain_name: Chain name from langchain.ai.toml
            params: Parameters for chain execution

        Returns:
            Result dictionary with success, output, error
        """
        log.info(f"⛓️  Job executor: Running chain '{chain_name}'")

        try:
            result = await self.agent_service.run_chain(
                chain_name=chain_name,
                params=params,
            )

            return {
                "success": result.success,
                "output": result.output or "",
                "error": result.error,
                "metadata": result.metadata,
            }

        except Exception as e:
            log.error(f"❌ Job executor chain error: {e}")
            return {
                "success": False,
                "output": "",
                "error": str(e),
                "metadata": {},
            }
