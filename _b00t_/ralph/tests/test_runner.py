"""Unit tests for Ralph runner."""

from __future__ import annotations

from unittest.mock import Mock, patch

import pytest
from returns.result import Failure, Success

from ralph.config import RalphConfig
from ralph.executors import ExecutorError
from ralph.runner import run_ralph
from ralph.taskmaster_adapter import Task


def _make_task(id: str = "1", title: str = "test task", status: str = "pending") -> Task:
    from datetime import datetime
    return Task(
        id=id, title=title, description="", status=status,
        priority=1, acceptance_criteria=[], depends_on=[],
        blocked_by=[], notes=[], created_at=datetime.now().isoformat(),
        updated_at=datetime.now().isoformat(),
    )


@pytest.fixture
def mock_taskmaster():
    """Mock taskmaster client with one pending task."""
    tm = Mock()
    task = _make_task()
    tm.get_all_tasks.return_value = Success([task])
    tm.get_next_task.return_value = Success(task)
    tm.update_task_status.return_value = Success(None)
    tm.add_task_note.return_value = Success(None)
    return tm


@pytest.fixture
def mock_config() -> RalphConfig:
    """Create a mock configuration for testing."""
    return RalphConfig.from_env(tool="amp")


def test_run_ralph_completes_successfully(mock_config: RalphConfig, mock_taskmaster) -> None:
    """Test run_ralph() when tool completes with marker."""
    with (
        patch("ralph.runner.AmpExecutor") as mock_executor_class,
        patch("ralph.runner.create_client", return_value=mock_taskmaster),
    ):
        mock_executor = Mock()
        mock_executor.run.return_value = Success(
            "Some output\n<promise>COMPLETE</promise>\nMore output"
        )
        mock_executor_class.return_value = mock_executor
        # After marking done, return no pending tasks
        done_task = _make_task(status="done")
        mock_taskmaster.get_all_tasks.side_effect = [
            Success([_make_task()]),  # observe #1
            Success([done_task]),     # observe after complete
        ]

        exit_code = run_ralph(mock_config, max_iterations=5)

    assert exit_code == 0
    assert mock_executor.run.call_count == 1


def test_run_ralph_max_iterations_reached(mock_config: RalphConfig, mock_taskmaster) -> None:
    """Test run_ralph() when max iterations reached."""
    with (
        patch("ralph.runner.AmpExecutor") as mock_executor_class,
        patch("ralph.runner.create_client", return_value=mock_taskmaster),
        patch("ralph.runner.time.sleep"),
    ):
        mock_executor = Mock()
        mock_executor.run.return_value = Success("Regular output without marker")
        mock_executor_class.return_value = mock_executor
        # Always return one pending task
        mock_taskmaster.get_all_tasks.return_value = Success([_make_task()])

        exit_code = run_ralph(mock_config, max_iterations=3)

    assert exit_code == 1
    assert mock_executor.run.call_count == 3


def test_run_ralph_executor_failure(mock_config: RalphConfig, mock_taskmaster) -> None:
    """Test run_ralph() when executor fails."""
    with (
        patch("ralph.runner.AmpExecutor") as mock_executor_class,
        patch("ralph.runner.create_client", return_value=mock_taskmaster),
        patch("ralph.runner.time.sleep"),
    ):
        mock_executor = Mock()
        error = ExecutorError(detail="Command failed", returncode=1)
        mock_executor.run.return_value = Failure(error)
        mock_executor_class.return_value = mock_executor
        mock_taskmaster.get_all_tasks.return_value = Success([_make_task()])

        exit_code = run_ralph(mock_config, max_iterations=5)

    assert exit_code == 1


def test_run_ralph_with_claude_tool(mock_taskmaster) -> None:
    """Test run_ralph() with claude tool."""
    config = RalphConfig.from_env(tool="claude")

    with (
        patch("ralph.runner.ClaudeExecutor") as mock_executor_class,
        patch("ralph.runner.create_client", return_value=mock_taskmaster),
    ):
        mock_executor = Mock()
        mock_executor.run.return_value = Success("<promise>COMPLETE</promise>")
        mock_executor_class.return_value = mock_executor
        done_task = _make_task(status="done")
        mock_taskmaster.get_all_tasks.side_effect = [
            Success([_make_task()]),
            Success([done_task]),
        ]

        exit_code = run_ralph(config, max_iterations=1)

    assert exit_code == 0
    mock_executor_class.assert_called_once()


def test_run_ralph_with_codex_tool(mock_taskmaster) -> None:
    """Test run_ralph() with codex tool."""
    config = RalphConfig.from_env(tool="codex")

    with (
        patch("ralph.runner.CodexExecutor") as mock_executor_class,
        patch("ralph.runner.create_client", return_value=mock_taskmaster),
    ):
        mock_executor = Mock()
        mock_executor.run.return_value = Success("<promise>COMPLETE</promise>")
        mock_executor_class.return_value = mock_executor
        done_task = _make_task(status="done")
        mock_taskmaster.get_all_tasks.side_effect = [
            Success([_make_task()]),
            Success([done_task]),
        ]

        exit_code = run_ralph(config, max_iterations=1)

    assert exit_code == 0
    mock_executor_class.assert_called_once()


def test_run_ralph_sleeps_between_iterations(mock_config: RalphConfig, mock_taskmaster) -> None:
    """Test run_ralph() sleeps between iterations."""
    with (
        patch("ralph.runner.AmpExecutor") as mock_executor_class,
        patch("ralph.runner.create_client", return_value=mock_taskmaster),
        patch("ralph.runner.time.sleep") as mock_sleep,
    ):
        mock_executor = Mock()
        mock_executor.run.side_effect = [
            Success("Regular output"),
            Success("<promise>COMPLETE</promise>"),
        ]
        mock_executor_class.return_value = mock_executor
        done_task = _make_task(status="done")
        mock_taskmaster.get_all_tasks.side_effect = [
            Success([_make_task()]),
            Success([_make_task()]),
            Success([done_task]),
        ]

        exit_code = run_ralph(mock_config, max_iterations=5)

    assert exit_code == 0
    assert mock_sleep.called


def test_run_ralph_logs_configuration(mock_config: RalphConfig, mock_taskmaster) -> None:
    """Test run_ralph() logs startup message."""
    with (
        patch("ralph.runner.AmpExecutor") as mock_executor_class,
        patch("ralph.runner.create_client", return_value=mock_taskmaster),
        patch("ralph.runner.configure_logging") as mock_configure_logging,
        patch("ralph.runner.log_info") as mock_log_info,
    ):
        mock_logger = Mock()
        mock_configure_logging.return_value = mock_logger
        mock_executor = Mock()
        mock_executor.run.return_value = Success("<promise>COMPLETE</promise>")
        mock_executor_class.return_value = mock_executor
        done_task = _make_task(status="done")
        mock_taskmaster.get_all_tasks.side_effect = [
            Success([_make_task()]),
            Success([done_task]),
        ]

        run_ralph(mock_config, max_iterations=1)

    log_calls = [str(call) for call in mock_log_info.call_args_list]
    assert any("ralph OODA start" in c for c in log_calls)


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
