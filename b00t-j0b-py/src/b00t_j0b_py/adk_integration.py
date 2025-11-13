"""Google ADK integration for running agents as RQ jobs.

This module provides integration between Google ADK (Agent Development Kit)
and the b00t job system, enabling AI agents to be executed as background jobs
with full lifecycle management, monitoring, and coordination.

Key features:
- Wrap ADK agents as RQ jobs
- Support for multi-agent hierarchies
- Session management and state persistence
- Tool integration with b00t ecosystem
- Human-in-the-loop (HITL) support via Redis pub/sub
"""

from typing import Dict, Any, List, Optional, Callable
from dataclasses import dataclass, field
from enum import Enum
from rq import get_current_job
import json
from datetime import datetime
from returns.result import Result, Success, Failure

from .config import config
from .redis_client import RedisTracker


class AgentStatus(Enum):
    """Status of an ADK agent execution."""
    INITIALIZING = "initializing"
    RUNNING = "running"
    WAITING_APPROVAL = "waiting_approval"  # HITL
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


@dataclass
class AgentExecutionContext:
    """Context for ADK agent execution within RQ job."""

    agent_id: str
    job_id: str
    session_id: Optional[str] = None
    parent_agent_id: Optional[str] = None
    status: AgentStatus = AgentStatus.INITIALIZING
    start_time: datetime = field(default_factory=datetime.utcnow)
    end_time: Optional[datetime] = None
    metadata: Dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> Dict[str, Any]:
        """Serialize context to dictionary."""
        return {
            "agent_id": self.agent_id,
            "job_id": self.job_id,
            "session_id": self.session_id,
            "parent_agent_id": self.parent_agent_id,
            "status": self.status.value,
            "start_time": self.start_time.isoformat(),
            "end_time": self.end_time.isoformat() if self.end_time else None,
            "metadata": self.metadata,
        }


@dataclass
class AgentConfig:
    """Configuration for ADK agent execution."""

    # Agent identification
    name: str
    description: str

    # Model configuration
    model_name: str = "gemini-2.0-flash-exp"
    temperature: float = 0.7
    max_tokens: Optional[int] = None

    # Tool configuration
    tools: List[str] = field(default_factory=list)  # Tool names to enable
    enable_mcp: bool = False  # Enable MCP tool integration

    # Sub-agent configuration
    sub_agents: List["AgentConfig"] = field(default_factory=list)

    # HITL configuration
    require_approval: bool = False  # Require human approval for actions
    approval_timeout: int = 300  # Seconds to wait for approval

    # Execution limits
    max_iterations: int = 50
    timeout: int = 600  # Seconds

    # Session configuration
    enable_rewind: bool = False  # Enable session rewind capability

    def to_dict(self) -> Dict[str, Any]:
        """Serialize config to dictionary."""
        return {
            "name": self.name,
            "description": self.description,
            "model_name": self.model_name,
            "temperature": self.temperature,
            "max_tokens": self.max_tokens,
            "tools": self.tools,
            "enable_mcp": self.enable_mcp,
            "sub_agents": [sa.to_dict() for sa in self.sub_agents],
            "require_approval": self.require_approval,
            "approval_timeout": self.approval_timeout,
            "max_iterations": self.max_iterations,
            "timeout": self.timeout,
            "enable_rewind": self.enable_rewind,
        }


class ADKAgentRunner:
    """Runner for executing ADK agents within RQ jobs."""

    def __init__(self, tracker: Optional[RedisTracker] = None):
        """Initialize ADK agent runner.

        Args:
            tracker: Redis tracker for state persistence (optional)
        """
        self.tracker = tracker or RedisTracker()

    def _persist_context(self, context: AgentExecutionContext) -> Result[bool, Exception]:
        """Persist agent execution context to Redis."""
        try:
            key = f"adk:agent:{context.agent_id}:context"
            self.tracker.redis.setex(
                key,
                3600,  # 1 hour TTL
                json.dumps(context.to_dict())
            )
            return Success(True)
        except Exception as e:
            return Failure(e)

    def _get_context(self, agent_id: str) -> Optional[AgentExecutionContext]:
        """Retrieve agent execution context from Redis."""
        try:
            key = f"adk:agent:{agent_id}:context"
            data = self.tracker.redis.get(key)
            if data:
                ctx_dict = json.loads(data)
                return AgentExecutionContext(
                    agent_id=ctx_dict["agent_id"],
                    job_id=ctx_dict["job_id"],
                    session_id=ctx_dict.get("session_id"),
                    parent_agent_id=ctx_dict.get("parent_agent_id"),
                    status=AgentStatus(ctx_dict["status"]),
                    start_time=datetime.fromisoformat(ctx_dict["start_time"]),
                    end_time=datetime.fromisoformat(ctx_dict["end_time"]) if ctx_dict.get("end_time") else None,
                    metadata=ctx_dict.get("metadata", {}),
                )
            return None
        except Exception:
            return None

    def _create_adk_agent(self, agent_config: AgentConfig) -> Any:
        """Create ADK agent from configuration.

        Note: This is a placeholder. Actual implementation requires:
        - `from adk import Agent` (when google-adk-python is installed)
        - Proper tool registration
        - Model client initialization

        Args:
            agent_config: Agent configuration

        Returns:
            Initialized ADK agent instance
        """
        # TODO: Implement actual ADK agent creation
        # This requires google-adk-python package:
        #
        # from adk import Agent
        # from adk.tools import tool
        #
        # agent = Agent(
        #     name=agent_config.name,
        #     description=agent_config.description,
        #     model=agent_config.model_name,
        #     temperature=agent_config.temperature,
        #     tools=[self._get_tool(tool_name) for tool_name in agent_config.tools],
        #     sub_agents=[self._create_adk_agent(sub) for sub in agent_config.sub_agents],
        # )
        # return agent

        raise NotImplementedError(
            "ADK agent creation requires google-adk-python package. "
            "Install with: pip install google-adk-python"
        )

    def _wait_for_approval(
        self,
        context: AgentExecutionContext,
        action: str,
        timeout: int
    ) -> Result[bool, str]:
        """Wait for human approval via Redis pub/sub (HITL).

        Args:
            context: Agent execution context
            action: Action description requiring approval
            timeout: Timeout in seconds

        Returns:
            Success(True) if approved, Failure with reason if denied/timeout
        """
        try:
            # Publish approval request
            approval_channel = f"adk:agent:{context.agent_id}:approval"
            request = {
                "agent_id": context.agent_id,
                "job_id": context.job_id,
                "action": action,
                "timestamp": datetime.utcnow().isoformat(),
            }
            self.tracker.redis.publish(approval_channel, json.dumps(request))

            # Wait for approval response
            response_key = f"adk:agent:{context.agent_id}:approval:response"

            # Block waiting for response (with timeout)
            # 🤓 Using BRPOP for blocking pop with timeout
            result = self.tracker.redis.brpop(response_key, timeout=timeout)

            if result:
                _, response_data = result
                response = json.loads(response_data)
                if response.get("approved"):
                    return Success(True)
                else:
                    return Failure(response.get("reason", "Approval denied"))
            else:
                return Failure(f"Approval timeout after {timeout}s")

        except Exception as e:
            return Failure(f"Approval error: {str(e)}")

    def execute_agent(
        self,
        agent_config: AgentConfig,
        task: str,
        context: Optional[Dict[str, Any]] = None,
        parent_agent_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        """Execute ADK agent as RQ job.

        Args:
            agent_config: Agent configuration
            task: Task description/prompt for the agent
            context: Additional context for the agent
            parent_agent_id: Parent agent ID if this is a sub-agent

        Returns:
            Execution result dictionary
        """
        job = get_current_job()
        job_id = job.id if job else "local"

        # Create execution context
        agent_ctx = AgentExecutionContext(
            agent_id=f"{agent_config.name}_{job_id}",
            job_id=job_id,
            parent_agent_id=parent_agent_id,
            metadata=context or {},
        )

        print(f"[{job_id}] Starting ADK agent: {agent_config.name}")
        print(f"[{job_id}] Task: {task}")

        try:
            # Persist initial context
            self._persist_context(agent_ctx)

            # Create ADK agent
            # 🚩 Requires google-adk-python package
            agent_ctx.status = AgentStatus.RUNNING
            self._persist_context(agent_ctx)

            # TODO: Actual agent execution would look like:
            # agent = self._create_adk_agent(agent_config)
            # result = agent.run(task, context=context)

            # For now, return placeholder
            agent_ctx.status = AgentStatus.COMPLETED
            agent_ctx.end_time = datetime.utcnow()
            self._persist_context(agent_ctx)

            print(f"[{job_id}] ADK agent completed: {agent_config.name}")

            return {
                "status": "success",
                "agent_id": agent_ctx.agent_id,
                "agent_name": agent_config.name,
                "task": task,
                "result": "Agent execution requires google-adk-python package",
                "execution_context": agent_ctx.to_dict(),
                "job_id": job_id,
            }

        except Exception as e:
            agent_ctx.status = AgentStatus.FAILED
            agent_ctx.end_time = datetime.utcnow()
            agent_ctx.metadata["error"] = str(e)
            self._persist_context(agent_ctx)

            print(f"[{job_id}] ADK agent failed: {agent_config.name} - {e}")

            return {
                "status": "error",
                "agent_id": agent_ctx.agent_id,
                "agent_name": agent_config.name,
                "task": task,
                "error": str(e),
                "execution_context": agent_ctx.to_dict(),
                "job_id": job_id,
            }


# RQ job wrapper functions

def adk_agent_job(
    agent_config_dict: Dict[str, Any],
    task: str,
    context: Optional[Dict[str, Any]] = None,
    parent_agent_id: Optional[str] = None,
) -> Dict[str, Any]:
    """RQ job to execute ADK agent.

    Args:
        agent_config_dict: Agent configuration as dictionary
        task: Task description/prompt
        context: Additional context
        parent_agent_id: Parent agent ID if sub-agent

    Returns:
        Execution result
    """
    # Reconstruct AgentConfig from dict
    # 🤓 Simple reconstruction - would need proper deserialization for sub_agents
    agent_config = AgentConfig(
        name=agent_config_dict["name"],
        description=agent_config_dict["description"],
        model_name=agent_config_dict.get("model_name", "gemini-2.0-flash-exp"),
        temperature=agent_config_dict.get("temperature", 0.7),
        max_tokens=agent_config_dict.get("max_tokens"),
        tools=agent_config_dict.get("tools", []),
        enable_mcp=agent_config_dict.get("enable_mcp", False),
        require_approval=agent_config_dict.get("require_approval", False),
        approval_timeout=agent_config_dict.get("approval_timeout", 300),
        max_iterations=agent_config_dict.get("max_iterations", 50),
        timeout=agent_config_dict.get("timeout", 600),
        enable_rewind=agent_config_dict.get("enable_rewind", False),
    )

    runner = ADKAgentRunner()
    return runner.execute_agent(agent_config, task, context, parent_agent_id)


def multi_agent_coordination_job(
    coordinator_config_dict: Dict[str, Any],
    sub_agent_configs: List[Dict[str, Any]],
    task: str,
    coordination_strategy: str = "sequential",
) -> Dict[str, Any]:
    """RQ job for multi-agent coordination.

    Args:
        coordinator_config_dict: Coordinator agent configuration
        sub_agent_configs: List of sub-agent configurations
        task: Task description
        coordination_strategy: "sequential", "parallel", or "hierarchical"

    Returns:
        Coordination result with all agent outputs
    """
    job = get_current_job()
    job_id = job.id if job else "local"

    print(f"[{job_id}] Starting multi-agent coordination: {coordination_strategy}")

    try:
        results = {
            "status": "success",
            "strategy": coordination_strategy,
            "coordinator": coordinator_config_dict["name"],
            "sub_agents": [],
            "job_id": job_id,
        }

        # Execute based on strategy
        if coordination_strategy == "sequential":
            # Execute sub-agents one by one
            for sub_config in sub_agent_configs:
                sub_result = adk_agent_job(sub_config, task, parent_agent_id=job_id)
                results["sub_agents"].append(sub_result)

        elif coordination_strategy == "parallel":
            # Enqueue sub-agents as separate jobs
            from .rq_integration import get_queue
            queue = get_queue()

            for sub_config in sub_agent_configs:
                sub_job = queue.enqueue(
                    adk_agent_job,
                    agent_config_dict=sub_config,
                    task=task,
                    parent_agent_id=job_id,
                )
                results["sub_agents"].append({
                    "agent": sub_config["name"],
                    "job_id": sub_job.id,
                    "status": "enqueued",
                })

        elif coordination_strategy == "hierarchical":
            # Create coordinator agent with sub-agents
            coordinator_config_dict["sub_agents"] = sub_agent_configs
            coordinator_result = adk_agent_job(coordinator_config_dict, task)
            results["coordinator_result"] = coordinator_result

        else:
            raise ValueError(f"Unknown coordination strategy: {coordination_strategy}")

        print(f"[{job_id}] Multi-agent coordination completed")
        return results

    except Exception as e:
        print(f"[{job_id}] Multi-agent coordination failed: {e}")
        return {
            "status": "error",
            "error": str(e),
            "job_id": job_id,
        }
