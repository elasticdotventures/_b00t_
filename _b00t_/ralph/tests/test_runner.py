"""Unit tests for Ralph runner."""

from __future__ import annotations

from unittest.mock import Mock, patch

import pytest
from returns.result import Failure, Success

from ralph.config import RalphConfig
from ralph.executors import ExecutorError
from ralph.runner import run_ralph


@pytest.fixture
def mock_config() -> RalphConfig:
    """Create a mock configuration for testing."""
    return RalphConfig.from_env(tool="amp")


def test_run_ralph_completes_successfully(mock_config: RalphConfig) -> None:
    """Test run_ralph() when tool completes with marker."""
    with patch("ralph.runner.AmpExecutor") as mock_executor_class:
        mock_executor = Mock()
        mock_executor.run.return_value = Success(
            "Some output\n<promise>COMPLETE</promise>\nMore output"
        )
        mock_executor_class.return_value = mock_executor

        exit_code = run_ralph(mock_config, max_iterations=5)

    assert exit_code == 0
    # Should only run once since completion marker was found
    assert mock_executor.run.call_count == 1


def test_run_ralph_max_iterations_reached(mock_config: RalphConfig) -> None:
    """Test run_ralph() when max iterations reached."""
    with patch("ralph.runner.AmpExecutor") as mock_executor_class:
        mock_executor = Mock()
        mock_executor.run.return_value = Success("Regular output without marker")
        mock_executor_class.return_value = mock_executor

        exit_code = run_ralph(mock_config, max_iterations=3)

    assert exit_code == 1
    # Should run exactly max_iterations times
    assert mock_executor.run.call_count == 3


def test_run_ralph_executor_failure(mock_config: RalphConfig) -> None:
    """Test run_ralph() when executor fails."""
    with patch("ralph.runner.AmpExecutor") as mock_executor_class:
        mock_executor = Mock()
        error = ExecutorError(detail="Command failed", returncode=1)
        mock_executor.run.return_value = Failure(error)
        mock_executor_class.return_value = mock_executor

        exit_code = run_ralph(mock_config, max_iterations=5)

    assert exit_code == 1
    # Should stop on first failure
    assert mock_executor.run.call_count == 1


def test_run_ralph_with_claude_tool() -> None:
    """Test run_ralph() with claude tool."""
    config = RalphConfig.from_env(tool="claude")

    with patch("ralph.runner.ClaudeExecutor") as mock_executor_class:
        mock_executor = Mock()
        mock_executor.run.return_value = Success("<promise>COMPLETE</promise>")
        mock_executor_class.return_value = mock_executor

        exit_code = run_ralph(config, max_iterations=1)

    assert exit_code == 0
    mock_executor_class.assert_called_once()


def test_run_ralph_with_codex_tool() -> None:
    """Test run_ralph() with codex tool."""
    config = RalphConfig.from_env(tool="codex")

    with patch("ralph.runner.CodexExecutor") as mock_executor_class:
        mock_executor = Mock()
        mock_executor.run.return_value = Success("<promise>COMPLETE</promise>")
        mock_executor_class.return_value = mock_executor

        exit_code = run_ralph(config, max_iterations=1)

    assert exit_code == 0
    mock_executor_class.assert_called_once()


def test_run_ralph_sleeps_between_iterations(mock_config: RalphConfig) -> None:
    """Test run_ralph() sleeps between iterations."""
    with (
        patch("ralph.runner.AmpExecutor") as mock_executor_class,
        patch("ralph.runner.time.sleep") as mock_sleep,
    ):
        mock_executor = Mock()
        # First call returns regular output, second call completes
        mock_executor.run.side_effect = [
            Success("Regular output"),
            Success("<promise>COMPLETE</promise>"),
        ]
        mock_executor_class.return_value = mock_executor

        exit_code = run_ralph(mock_config, max_iterations=5)

    assert exit_code == 0
    # Should sleep once between iterations
    mock_sleep.assert_called_once_with(2)


def test_run_ralph_logs_configuration(mock_config: RalphConfig) -> None:
    """Test run_ralph() logs configuration at startup."""
    with (
        patch("ralph.runner.AmpExecutor") as mock_executor_class,
        patch("ralph.runner.configure_logging") as mock_configure_logging,
        patch("ralph.runner.log_info") as mock_log_info,
    ):
        mock_logger = Mock()
        mock_configure_logging.return_value = mock_logger
        mock_executor = Mock()
        mock_executor.run.return_value = Success("<promise>COMPLETE</promise>")
        mock_executor_class.return_value = mock_executor

        run_ralph(mock_config, max_iterations=1)

    # Check that configuration was logged
    log_calls = [str(call) for call in mock_log_info.call_args_list]
    config_logged = any("Configuration loaded" in str(call) for call in log_calls)
    assert config_logged


def test_run_ralph_unsupported_tool() -> None:
    """Test run_ralph() with unsupported tool."""
    # This requires creating a config with an invalid tool, which the dataclass doesn't allow
    # So we'll test the _build_executor function directly
    from ralph.runner import _build_executor

    config = RalphConfig.from_env(tool="amp")
    # Manually override the tool field (breaking immutability for test)
    with pytest.raises(ValueError, match="Unsupported tool"):
        # Use object.__setattr__ to bypass frozen dataclass
        object.__setattr__(config, "tool", "invalid-tool")
        _build_executor(config.tool, config)
