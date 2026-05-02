"""
Agent Service for LangChain v1.0 agents.

Manages agent lifecycle, execution, and cross-agent communication.
"""

import asyncio
import logging
import tomllib
from pathlib import Path
from typing import Any

from langchain.agents import create_agent
from langchain_anthropic import ChatAnthropic
from langchain_core.messages import HumanMessage
from langchain_core.runnables import RunnableConfig
from langchain_core.tools import BaseTool
from redis.asyncio import Redis

from .b00t_tools import B00tToolset
from .decision_tree import DecisionTree
from .types import AgentConfig, AgentResult, ChainConfig

log = logging.getLogger(__name__)


class AgentService:
    """Manages LangChain agent lifecycle and execution."""

    def __init__(
        self,
        redis_client: Redis | None,
        mcp_tools: list[BaseTool],
        datum_path: Path,
    ) -> None:
        """
        Initialize Agent Service.

        Args:
            redis_client: Redis client for status publishing (optional)
            mcp_tools: List of MCP tools discovered
            datum_path: Path to _b00t_ directory
        """
        self.redis_client = redis_client
        self.mcp_tools = mcp_tools
        self.datum_path = datum_path

        # Loaded configurations
        self.agent_configs: dict[str, AgentConfig] = {}
        self.chain_configs: dict[str, ChainConfig] = {}

        # Active agents (cached)
        self.agents: dict[str, Any] = {}
        self.native_toolset = B00tToolset(datum_path)

    async def initialize(self) -> None:
        """Load agent and chain configurations from datum."""
        log.info("🤖 Initializing Agent Service...")

        # Load langchain.ai.toml
        langchain_datum = self.datum_path / "langchain.ai.toml"
        if not langchain_datum.exists():
            log.warning(f"⚠️  {langchain_datum} not found, using defaults")
            return

        with open(langchain_datum, "rb") as f:
            data = tomllib.load(f)

        # Parse agent presets
        agents = data.get("langchain", {}).get("agents", {})
        for agent_name, agent_data in agents.items():
            try:
                config = AgentConfig(
                    name=agent_name,
                    description=agent_data.get("description"),
                    model=agent_data.get("model", "anthropic/claude-sonnet-4"),
                    tools=agent_data.get("tools", []),
                    middleware=agent_data.get("middleware", []),
                    system_prompt=agent_data.get("system_prompt", ""),
                    max_iterations=agent_data.get("max_iterations", 10),
                    timeout_seconds=agent_data.get("timeout_seconds", 300),
                    peer_agents=agent_data.get("peer_agents", []),
                    decision_tree=agent_data.get("decision_tree"),
                    bootstrap_script=agent_data.get("bootstrap_script"),
                )
                self.agent_configs[agent_name] = config
                log.info(f"  ✅ Loaded agent config: {agent_name}")
            except Exception as e:
                log.warning(f"  ⚠️  Failed to load agent {agent_name}: {e}")

        # Parse chain presets
        chains = data.get("langchain", {}).get("chains", {})
        for chain_name, chain_data in chains.items():
            try:
                config = ChainConfig(
                    name=chain_name,
                    description=chain_data.get("description"),
                    steps=chain_data.get("steps", []),
                )
                self.chain_configs[chain_name] = config
                log.info(f"  ✅ Loaded chain config: {chain_name}")
            except Exception as e:
                log.warning(f"  ⚠️  Failed to load chain {chain_name}: {e}")

        log.info(
            f"Agent Service initialized: {len(self.agent_configs)} agents, "
            f"{len(self.chain_configs)} chains"
        )

    async def create_agent(
        self,
        agent_name: str,
        model_override: str | None = None,
    ) -> Any:
        """
        Create or retrieve cached agent.

        Args:
            agent_name: Name of agent from configuration
            model_override: Optional model override

        Returns:
            LangGraph agent executor
        """
        # Check cache
        cache_key = f"{agent_name}:{model_override or 'default'}"
        if cache_key in self.agents:
            return self.agents[cache_key]

        # Get configuration
        config = self.agent_configs.get(agent_name)
        if not config:
            raise ValueError(f"Agent '{agent_name}' not found in configuration")

        # Select model
        model_name = model_override or config.model

        # Create LLM
        if model_name.startswith("anthropic/"):
            llm = ChatAnthropic(
                model=model_name.replace("anthropic/", ""),
                temperature=0.0,
            )
        else:
            # 🤓: Add support for other providers (OpenAI, etc.)
            raise ValueError(f"Unsupported model provider: {model_name}")

        # Get tools for this agent
        agent_tools = self._get_tools_for_agent(config)

        # De-duplicate by tool name; some LangChain constructors reject duplicate names.
        agent_tools = list({t.name: t for t in agent_tools}.values())

        # LangChain v1 uses `create_agent`; keep the service on the non-deprecated path.
        agent = create_agent(
            model=llm,
            tools=agent_tools,
            system_prompt=self._build_system_prompt(config),
        )

        # Cache agent
        self.agents[cache_key] = agent

        log.info(f"✅ Created agent: {agent_name} with {len(agent_tools)} tools")
        return agent

    def _get_tools_for_agent(self, config: AgentConfig) -> list[BaseTool]:
        """
        Get tools for an agent based on configuration.

        Args:
            config: Agent configuration

        Returns:
            List of BaseTool instances
        """
        tools: list[BaseTool] = []

        for tool_name in config.tools:
            # Find matching tools from MCP discovery
            matching = [t for t in self.mcp_tools if tool_name in t.name]
            tools.extend(matching)

        if not tools:
            log.warning(f"⚠️  No tools found for agent {config.name}")

        return tools

    def _build_system_prompt(self, config: AgentConfig) -> str:
        """Compose prompt with operator-specific routing context."""
        sections = [config.system_prompt.strip()]

        if config.decision_tree:
            tree_path = self.datum_path / config.decision_tree
            if tree_path.exists():
                tree = DecisionTree.from_file(tree_path)
                sections.append(tree.summary())

        if config.bootstrap_script:
            sections.append(
                "Internal operator script is available via `b00t_operator_script` "
                f"and resolves to `{config.bootstrap_script}`."
            )

        return "\n\n".join(section for section in sections if section)

    async def run_agent(
        self,
        agent_name: str,
        input_text: str,
        model_override: str | None = None,
    ) -> AgentResult:
        """
        Run an agent with given input.

        Args:
            agent_name: Name of agent to run
            input_text: Input prompt for agent
            model_override: Optional model override

        Returns:
            AgentResult with output or error
        """
        try:
            # Create/get agent
            agent = await self.create_agent(agent_name, model_override)

            # Get configuration for timeout
            config = self.agent_configs[agent_name]

            # Run agent with timeout
            log.info(f"🚀 Running agent: {agent_name}")
            log.info(f"   Input: {input_text[:100]}...")

            # Create input
            inputs = {"messages": [HumanMessage(content=input_text)]}

            # Run with timeout
            result = await asyncio.wait_for(
                agent.ainvoke(
                    inputs,
                    config=RunnableConfig(recursion_limit=config.max_iterations),
                ),
                timeout=config.timeout_seconds,
            )

            # Extract output
            # 🤓: LangGraph result is a dict with 'messages' key
            messages = result.get("messages", [])
            if messages:
                output = messages[-1].content
            else:
                output = "No output generated"

            log.info(f"✅ Agent {agent_name} completed")

            return AgentResult(
                success=True,
                agent_name=agent_name,
                output=output,
                message=f"Agent {agent_name} executed successfully",
                metadata={"model": config.model, "iterations": len(messages)},
            )

        except asyncio.TimeoutError:
            log.error(f"❌ Agent {agent_name} timed out")
            return AgentResult(
                success=False,
                agent_name=agent_name,
                error="Agent execution timed out",
                message=f"Agent {agent_name} timed out after {config.timeout_seconds}s",
            )
        except Exception as e:
            log.error(f"❌ Agent {agent_name} failed: {e}")
            return AgentResult(
                success=False,
                agent_name=agent_name,
                error=str(e),
                message=f"Agent {agent_name} execution failed",
            )

    async def broadcast_to_agents(
        self,
        message: str,
        from_agent: str | None = None,
    ) -> list[AgentResult]:
        """
        Broadcast a message to all agents.

        Args:
            message: Message to broadcast
            from_agent: Agent sending the broadcast

        Returns:
            List of responses from all agents
        """
        log.info(f"📢 Broadcasting from {from_agent or 'system'}: {message[:50]}...")

        results: list[AgentResult] = []

        # Run all agents concurrently
        tasks = [
            self.run_agent(agent_name, message)
            for agent_name in self.agent_configs.keys()
            if agent_name != from_agent  # Don't broadcast to self
        ]

        results = await asyncio.gather(*tasks, return_exceptions=True)

        # Convert exceptions to AgentResult
        final_results: list[AgentResult] = []
        for idx, result in enumerate(results):
            if isinstance(result, Exception):
                final_results.append(
                    AgentResult(
                        success=False,
                        error=str(result),
                        message="Agent execution failed",
                    )
                )
            else:
                final_results.append(result)

        return final_results

    async def run_chain(
        self,
        chain_name: str,
        params: dict[str, Any],
    ) -> AgentResult:
        """
        Run a predefined chain workflow.

        Args:
            chain_name: Name of chain from configuration
            params: Parameters for chain execution

        Returns:
            AgentResult with chain output
        """
        config = self.chain_configs.get(chain_name)
        if not config:
            return AgentResult(
                success=False,
                error=f"Chain '{chain_name}' not found",
                message="Chain not found",
            )

        log.info(f"⛓️  Running chain: {chain_name}")

        try:
            # Execute each step in sequence
            context = params.copy()

            for step_idx, step in enumerate(config.steps):
                log.info(f"  Step {step_idx + 1}/{len(config.steps)}: {step}")

                # 🤓: Implement chain step execution
                # For now, placeholder
                if "agent" in step:
                    agent_name = step["agent"]
                    task = step.get("task", "")
                    result = await self.run_agent(agent_name, task)
                    context[f"step_{step_idx}_result"] = result.output

            return AgentResult(
                success=True,
                message=f"Chain {chain_name} completed",
                output=str(context),
                metadata={"chain": chain_name, "steps": len(config.steps)},
            )

        except Exception as e:
            log.error(f"❌ Chain {chain_name} failed: {e}")
            return AgentResult(
                success=False,
                error=str(e),
                message=f"Chain {chain_name} execution failed",
            )

    async def shutdown(self) -> None:
        """Gracefully shutdown agent service."""
        log.info("🛑 Shutting down Agent Service...")

        # Clear cached agents
        self.agents.clear()

        log.info("✅ Agent Service stopped")
