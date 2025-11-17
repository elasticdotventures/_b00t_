"""Tests for k0mmand3r IPC listener."""

import asyncio
import json
import pytest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock

from b00t_langchain_agent.agent_service import AgentService
from b00t_langchain_agent.k0mmand3r import K0mmand3rListener
from b00t_langchain_agent.types import K0mmand3rMessage, AgentResult


@pytest.mark.asyncio
async def test_k0mmand3r_message_parsing():
    """Test parsing k0mmand3r messages."""
    message_data = json.dumps({
        "verb": "agent",
        "params": {
            "action": "run",
            "name": "researcher",
            "input": "Test input"
        },
        "timestamp": "2025-01-17T12:00:00Z",
        "agent_id": "test_agent"
    })

    message = K0mmand3rMessage(**json.loads(message_data))
    assert message.verb == "agent"
    assert message.params["action"] == "run"
    assert message.params["name"] == "researcher"


@pytest.mark.asyncio
async def test_k0mmand3r_listener_handle_agent_command():
    """Test handling agent commands."""
    # Create mock Redis and AgentService
    mock_redis = AsyncMock()
    mock_agent_service = MagicMock(spec=AgentService)
    mock_agent_service.redis_client = mock_redis
    mock_agent_service.run_agent = AsyncMock(
        return_value=AgentResult(
            success=True,
            agent_name="test_agent",
            output="Test output",
            message="Success"
        )
    )

    listener = K0mmand3rListener(
        redis_sub=mock_redis,
        agent_service=mock_agent_service,
        channel="test:channel",
    )

    # Create test message
    message = K0mmand3rMessage(
        verb="agent",
        params={
            "action": "run",
            "name": "test_agent",
            "input": "Test input"
        }
    )

    # Handle message
    await listener._handle_agent_command(message)

    # Verify run_agent was called
    mock_agent_service.run_agent.assert_called_once_with(
        agent_name="test_agent",
        input_text="Test input",
        model_override=None,
    )


@pytest.mark.asyncio
async def test_k0mmand3r_listener_handle_chain_command():
    """Test handling chain commands."""
    # Create mock Redis and AgentService
    mock_redis = AsyncMock()
    mock_agent_service = MagicMock(spec=AgentService)
    mock_agent_service.redis_client = mock_redis
    mock_agent_service.run_chain = AsyncMock(
        return_value=AgentResult(
            success=True,
            message="Chain completed",
            output="Chain output"
        )
    )

    listener = K0mmand3rListener(
        redis_sub=mock_redis,
        agent_service=mock_agent_service,
        channel="test:channel",
    )

    # Create test message
    message = K0mmand3rMessage(
        verb="chain",
        params={
            "action": "run",
            "name": "research-and-digest",
            "url": "https://example.com"
        }
    )

    # Handle message
    await listener._handle_chain_command(message)

    # Verify run_chain was called
    mock_agent_service.run_chain.assert_called_once_with(
        chain_name="research-and-digest",
        params={"url": "https://example.com"},
    )


@pytest.mark.asyncio
async def test_k0mmand3r_listener_broadcast():
    """Test broadcast functionality."""
    # Create mock Redis and AgentService
    mock_redis = AsyncMock()
    mock_agent_service = MagicMock(spec=AgentService)
    mock_agent_service.redis_client = mock_redis
    mock_agent_service.broadcast_to_agents = AsyncMock(
        return_value=[
            AgentResult(success=True, agent_name="agent1", output="Response 1", message="OK"),
            AgentResult(success=True, agent_name="agent2", output="Response 2", message="OK"),
        ]
    )

    listener = K0mmand3rListener(
        redis_sub=mock_redis,
        agent_service=mock_agent_service,
        channel="test:channel",
    )

    # Create broadcast message
    message = K0mmand3rMessage(
        verb="agent",
        params={
            "action": "broadcast",
            "message": "Status update?",
            "from": "coordinator"
        }
    )

    # Handle message
    await listener._handle_agent_command(message)

    # Verify broadcast was called
    mock_agent_service.broadcast_to_agents.assert_called_once_with(
        message="Status update?",
        from_agent="coordinator",
    )
