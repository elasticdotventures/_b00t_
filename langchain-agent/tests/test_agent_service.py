"""Tests for Agent Service."""

import pytest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

from b00t_langchain_agent.agent_service import AgentService
from b00t_langchain_agent.types import AgentConfig


@pytest.mark.asyncio
async def test_agent_service_initialization():
    """Test agent service initialization."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"
    if not datum_path.exists():
        pytest.skip("_b00t_ directory not found")

    service = AgentService(
        redis_client=None,
        mcp_tools=[],
        datum_path=datum_path,
    )

    await service.initialize()

    # Should load some agent configs
    # Note: This depends on langchain.ai.toml existing
    if (datum_path / "langchain.ai.toml").exists():
        assert len(service.agent_configs) > 0


@pytest.mark.asyncio
async def test_agent_service_get_tools_for_agent():
    """Test getting tools for an agent."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"

    # Create mock tool
    mock_tool = MagicMock()
    mock_tool.name = "test_tool"

    service = AgentService(
        redis_client=None,
        mcp_tools=[mock_tool],
        datum_path=datum_path,
    )

    # Create test agent config
    config = AgentConfig(
        name="test_agent",
        model="anthropic/claude-sonnet-4",
        tools=["test_tool"],
        system_prompt="Test prompt",
    )

    tools = service._get_tools_for_agent(config)
    assert len(tools) == 1
    assert tools[0].name == "test_tool"


@pytest.mark.asyncio
async def test_agent_service_create_agent():
    """Test agent creation."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"

    service = AgentService(
        redis_client=None,
        mcp_tools=[],
        datum_path=datum_path,
    )

    # Add test agent config
    service.agent_configs["test_agent"] = AgentConfig(
        name="test_agent",
        model="anthropic/claude-sonnet-4",
        tools=[],
        system_prompt="You are a test agent",
    )

    # Create agent (requires ANTHROPIC_API_KEY)
    try:
        agent = await service.create_agent("test_agent")
        assert agent is not None
    except Exception as e:
        # Expected if ANTHROPIC_API_KEY not set
        if "ANTHROPIC_API_KEY" in str(e) or "API key" in str(e):
            pytest.skip("ANTHROPIC_API_KEY not configured")
        else:
            raise


@pytest.mark.asyncio
async def test_agent_service_run_agent():
    """Test running an agent."""
    datum_path = Path.home() / ".dotfiles" / "_b00t_"

    service = AgentService(
        redis_client=None,
        mcp_tools=[],
        datum_path=datum_path,
    )

    # Add test agent config
    service.agent_configs["test_agent"] = AgentConfig(
        name="test_agent",
        model="anthropic/claude-sonnet-4",
        tools=[],
        system_prompt="You are a test agent. Always respond with 'Test response'.",
        max_iterations=1,
        timeout_seconds=30,
    )

    # Run agent (requires ANTHROPIC_API_KEY)
    try:
        result = await service.run_agent(
            agent_name="test_agent",
            input_text="Hello",
        )
        assert result.success or "API key" in (result.error or "")
    except Exception as e:
        # Expected if ANTHROPIC_API_KEY not set
        if "ANTHROPIC_API_KEY" in str(e) or "API key" in str(e):
            pytest.skip("ANTHROPIC_API_KEY not configured")
        else:
            raise
