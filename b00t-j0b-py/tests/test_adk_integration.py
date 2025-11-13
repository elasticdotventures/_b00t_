"""Tests for Google ADK integration."""

import pytest
from datetime import datetime
from unittest.mock import Mock, patch, MagicMock
import json

from b00t_j0b_py.adk_integration import (
    ADKAgentRunner,
    AgentConfig,
    AgentExecutionContext,
    AgentStatus,
    adk_agent_job,
    multi_agent_coordination_job,
)


@pytest.fixture
def mock_redis():
    """Mock Redis connection."""
    redis_mock = MagicMock()
    redis_mock.get.return_value = None
    redis_mock.setex.return_value = True
    redis_mock.publish.return_value = 1
    redis_mock.brpop.return_value = None
    return redis_mock


@pytest.fixture
def mock_tracker(mock_redis):
    """Mock RedisTracker."""
    tracker = Mock()
    tracker.redis = mock_redis
    return tracker


@pytest.fixture
def simple_agent_config():
    """Simple agent configuration for testing."""
    return AgentConfig(
        name="test-agent",
        description="Test agent for unit tests",
        model_name="gemini-2.0-flash-exp",
        temperature=0.7,
        tools=["search", "calculator"],
    )


@pytest.fixture
def hierarchical_agent_config():
    """Hierarchical agent configuration with sub-agents."""
    sub_agent_1 = AgentConfig(
        name="researcher",
        description="Research sub-agent",
        tools=["search"],
    )
    sub_agent_2 = AgentConfig(
        name="analyzer",
        description="Analysis sub-agent",
        tools=["calculator"],
    )

    return AgentConfig(
        name="coordinator",
        description="Coordinator agent",
        sub_agents=[sub_agent_1, sub_agent_2],
    )


class TestAgentConfig:
    """Tests for AgentConfig dataclass."""

    def test_agent_config_creation(self, simple_agent_config):
        """Test basic agent configuration creation."""
        assert simple_agent_config.name == "test-agent"
        assert simple_agent_config.model_name == "gemini-2.0-flash-exp"
        assert len(simple_agent_config.tools) == 2

    def test_agent_config_serialization(self, simple_agent_config):
        """Test agent configuration serialization."""
        config_dict = simple_agent_config.to_dict()

        assert config_dict["name"] == "test-agent"
        assert config_dict["tools"] == ["search", "calculator"]
        assert config_dict["temperature"] == 0.7

    def test_agent_config_with_sub_agents(self, hierarchical_agent_config):
        """Test agent configuration with sub-agents."""
        assert len(hierarchical_agent_config.sub_agents) == 2
        assert hierarchical_agent_config.sub_agents[0].name == "researcher"

        config_dict = hierarchical_agent_config.to_dict()
        assert len(config_dict["sub_agents"]) == 2


class TestAgentExecutionContext:
    """Tests for AgentExecutionContext."""

    def test_context_creation(self):
        """Test execution context creation."""
        ctx = AgentExecutionContext(
            agent_id="agent-123",
            job_id="job-456",
        )

        assert ctx.agent_id == "agent-123"
        assert ctx.job_id == "job-456"
        assert ctx.status == AgentStatus.INITIALIZING
        assert ctx.end_time is None

    def test_context_serialization(self):
        """Test execution context serialization."""
        ctx = AgentExecutionContext(
            agent_id="agent-123",
            job_id="job-456",
            session_id="session-789",
            metadata={"key": "value"},
        )
        ctx.status = AgentStatus.RUNNING

        ctx_dict = ctx.to_dict()

        assert ctx_dict["agent_id"] == "agent-123"
        assert ctx_dict["status"] == "running"
        assert ctx_dict["metadata"]["key"] == "value"


class TestADKAgentRunner:
    """Tests for ADKAgentRunner."""

    def test_runner_initialization(self, mock_tracker):
        """Test runner initialization."""
        runner = ADKAgentRunner(tracker=mock_tracker)
        assert runner.tracker == mock_tracker

    def test_persist_context(self, mock_tracker, mock_redis):
        """Test context persistence to Redis."""
        runner = ADKAgentRunner(tracker=mock_tracker)

        ctx = AgentExecutionContext(
            agent_id="test-agent",
            job_id="test-job",
        )

        result = runner._persist_context(ctx)

        assert result.is_success
        mock_redis.setex.assert_called_once()

        # Verify correct key format
        call_args = mock_redis.setex.call_args
        assert call_args[0][0] == "adk:agent:test-agent:context"
        assert call_args[0][1] == 3600  # 1 hour TTL

    def test_get_context(self, mock_tracker, mock_redis):
        """Test context retrieval from Redis."""
        runner = ADKAgentRunner(tracker=mock_tracker)

        # Mock stored context
        ctx = AgentExecutionContext(
            agent_id="test-agent",
            job_id="test-job",
        )
        mock_redis.get.return_value = json.dumps(ctx.to_dict())

        retrieved_ctx = runner._get_context("test-agent")

        assert retrieved_ctx is not None
        assert retrieved_ctx.agent_id == "test-agent"
        assert retrieved_ctx.job_id == "test-job"

    def test_get_context_not_found(self, mock_tracker, mock_redis):
        """Test context retrieval when context doesn't exist."""
        runner = ADKAgentRunner(tracker=mock_tracker)
        mock_redis.get.return_value = None

        retrieved_ctx = runner._get_context("nonexistent-agent")
        assert retrieved_ctx is None

    def test_wait_for_approval_timeout(self, mock_tracker, mock_redis):
        """Test HITL approval with timeout."""
        runner = ADKAgentRunner(tracker=mock_tracker)

        ctx = AgentExecutionContext(
            agent_id="test-agent",
            job_id="test-job",
        )

        # Mock timeout (no response)
        mock_redis.brpop.return_value = None

        result = runner._wait_for_approval(ctx, "Test action", timeout=5)

        assert result.is_failure
        assert "timeout" in str(result.failure())

    def test_wait_for_approval_approved(self, mock_tracker, mock_redis):
        """Test HITL approval when user approves."""
        runner = ADKAgentRunner(tracker=mock_tracker)

        ctx = AgentExecutionContext(
            agent_id="test-agent",
            job_id="test-job",
        )

        # Mock approval response
        approval_response = json.dumps({"approved": True})
        mock_redis.brpop.return_value = ("response_key", approval_response)

        result = runner._wait_for_approval(ctx, "Test action", timeout=5)

        assert result.is_success

    def test_wait_for_approval_denied(self, mock_tracker, mock_redis):
        """Test HITL approval when user denies."""
        runner = ADKAgentRunner(tracker=mock_tracker)

        ctx = AgentExecutionContext(
            agent_id="test-agent",
            job_id="test-job",
        )

        # Mock denial response
        denial_response = json.dumps({"approved": False, "reason": "Not safe"})
        mock_redis.brpop.return_value = ("response_key", denial_response)

        result = runner._wait_for_approval(ctx, "Test action", timeout=5)

        assert result.is_failure
        assert "Not safe" in str(result.failure())

    @patch("b00t_j0b_py.adk_integration.get_current_job")
    def test_execute_agent_basic(self, mock_get_job, mock_tracker, simple_agent_config):
        """Test basic agent execution (without actual ADK)."""
        mock_job = Mock()
        mock_job.id = "test-job-123"
        mock_get_job.return_value = mock_job

        runner = ADKAgentRunner(tracker=mock_tracker)

        result = runner.execute_agent(
            agent_config=simple_agent_config,
            task="Test task",
        )

        assert result["status"] == "success"
        assert result["agent_name"] == "test-agent"
        assert result["job_id"] == "test-job-123"


class TestADKJobWrappers:
    """Tests for RQ job wrapper functions."""

    @patch("b00t_j0b_py.adk_integration.get_current_job")
    @patch("b00t_j0b_py.adk_integration.RedisTracker")
    def test_adk_agent_job(self, mock_tracker_class, mock_get_job):
        """Test ADK agent job wrapper."""
        mock_job = Mock()
        mock_job.id = "job-123"
        mock_get_job.return_value = mock_job

        mock_tracker = Mock()
        mock_redis = MagicMock()
        mock_tracker.redis = mock_redis
        mock_tracker_class.return_value = mock_tracker

        agent_config_dict = {
            "name": "test-agent",
            "description": "Test agent",
            "tools": ["search"],
        }

        result = adk_agent_job(agent_config_dict, "Test task")

        assert result["status"] == "success"
        assert result["agent_name"] == "test-agent"

    @patch("b00t_j0b_py.adk_integration.get_current_job")
    @patch("b00t_j0b_py.adk_integration.RedisTracker")
    def test_multi_agent_sequential(self, mock_tracker_class, mock_get_job):
        """Test multi-agent coordination with sequential strategy."""
        mock_job = Mock()
        mock_job.id = "coord-job-123"
        mock_get_job.return_value = mock_job

        mock_tracker = Mock()
        mock_redis = MagicMock()
        mock_tracker.redis = mock_redis
        mock_tracker_class.return_value = mock_tracker

        coordinator_config = {
            "name": "coordinator",
            "description": "Coordinator agent",
        }

        sub_agent_configs = [
            {"name": "sub-agent-1", "description": "First sub-agent"},
            {"name": "sub-agent-2", "description": "Second sub-agent"},
        ]

        result = multi_agent_coordination_job(
            coordinator_config,
            sub_agent_configs,
            "Coordination task",
            coordination_strategy="sequential",
        )

        assert result["status"] == "success"
        assert result["strategy"] == "sequential"
        assert len(result["sub_agents"]) == 2

    @patch("b00t_j0b_py.adk_integration.get_current_job")
    @patch("b00t_j0b_py.adk_integration.get_queue")
    @patch("b00t_j0b_py.adk_integration.RedisTracker")
    def test_multi_agent_parallel(self, mock_tracker_class, mock_get_queue, mock_get_job):
        """Test multi-agent coordination with parallel strategy."""
        mock_job = Mock()
        mock_job.id = "coord-job-456"
        mock_get_job.return_value = mock_job

        mock_queue = Mock()
        mock_enqueued_job = Mock()
        mock_enqueued_job.id = "sub-job-123"
        mock_queue.enqueue.return_value = mock_enqueued_job
        mock_get_queue.return_value = mock_queue

        coordinator_config = {
            "name": "coordinator",
            "description": "Coordinator agent",
        }

        sub_agent_configs = [
            {"name": "parallel-1", "description": "Parallel agent 1"},
            {"name": "parallel-2", "description": "Parallel agent 2"},
        ]

        result = multi_agent_coordination_job(
            coordinator_config,
            sub_agent_configs,
            "Parallel task",
            coordination_strategy="parallel",
        )

        assert result["status"] == "success"
        assert result["strategy"] == "parallel"
        assert len(result["sub_agents"]) == 2
        assert all(agent["status"] == "enqueued" for agent in result["sub_agents"])

        # Verify jobs were enqueued
        assert mock_queue.enqueue.call_count == 2

    @patch("b00t_j0b_py.adk_integration.get_current_job")
    @patch("b00t_j0b_py.adk_integration.RedisTracker")
    def test_multi_agent_hierarchical(self, mock_tracker_class, mock_get_job):
        """Test multi-agent coordination with hierarchical strategy."""
        mock_job = Mock()
        mock_job.id = "coord-job-789"
        mock_get_job.return_value = mock_job

        mock_tracker = Mock()
        mock_redis = MagicMock()
        mock_tracker.redis = mock_redis
        mock_tracker_class.return_value = mock_tracker

        coordinator_config = {
            "name": "coordinator",
            "description": "Coordinator agent",
        }

        sub_agent_configs = [
            {"name": "hierarchical-1", "description": "Sub-agent 1"},
        ]

        result = multi_agent_coordination_job(
            coordinator_config,
            sub_agent_configs,
            "Hierarchical task",
            coordination_strategy="hierarchical",
        )

        assert result["status"] == "success"
        assert result["strategy"] == "hierarchical"
        assert "coordinator_result" in result

    @patch("b00t_j0b_py.adk_integration.get_current_job")
    def test_multi_agent_invalid_strategy(self, mock_get_job):
        """Test multi-agent coordination with invalid strategy."""
        mock_job = Mock()
        mock_job.id = "job-invalid"
        mock_get_job.return_value = mock_job

        result = multi_agent_coordination_job(
            {"name": "coord", "description": "Coordinator"},
            [],
            "Task",
            coordination_strategy="invalid",
        )

        assert result["status"] == "error"
        assert "Unknown coordination strategy" in result["error"]


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
