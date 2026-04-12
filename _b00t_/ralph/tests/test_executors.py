"""Unit tests for Ralph tool executors."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import Mock, patch

import pytest
from returns.result import Failure, Success

from ralph.config import RalphConfig
from ralph.executors import (
    AmpExecutor,
    ClaudeExecutor,
    CodexExecutor,
    ExecutorError,
    OpenCodeExecutor,
)


@pytest.fixture
def temp_prompt_file(tmp_path: Path) -> Path:
    """Create a temporary prompt file for testing."""
    prompt_file = tmp_path / "prompt.md"
    prompt_file.write_text("Test prompt content")
    return prompt_file


def test_amp_executor_success(temp_prompt_file: Path) -> None:
    """Test AmpExecutor with successful execution."""
    executor = AmpExecutor(prompt_path=temp_prompt_file, working_dir=temp_prompt_file.parent)

    with patch("ralph.executors._run_subprocess") as mock_subprocess:
        mock_subprocess.return_value = Success("Output from amp")
        result = executor.run()

    assert isinstance(result, Success)
    assert result.unwrap() == "Output from amp"
    mock_subprocess.assert_called_once()
    args = mock_subprocess.call_args
    assert args[0][0] == ("amp", "--dangerously-allow-all")
    assert args[1]["input_text"] == "Test prompt content"


def test_amp_executor_prompt_file_not_found(tmp_path: Path) -> None:
    """Test AmpExecutor with missing prompt file."""
    missing_file = tmp_path / "nonexistent.md"
    executor = AmpExecutor(prompt_path=missing_file, working_dir=tmp_path)

    result = executor.run()

    assert isinstance(result, Failure)
    error = result.failure()
    assert isinstance(error, ExecutorError)
    assert "Unable to read prompt file" in error.detail


def test_claude_executor_success(temp_prompt_file: Path) -> None:
    """Test ClaudeExecutor with successful execution."""
    executor = ClaudeExecutor(
        prompt_path=temp_prompt_file, working_dir=temp_prompt_file.parent
    )

    with patch("ralph.executors._run_subprocess") as mock_subprocess:
        mock_subprocess.return_value = Success("Output from claude")
        result = executor.run()

    assert isinstance(result, Success)
    assert result.unwrap() == "Output from claude"
    mock_subprocess.assert_called_once()
    args = mock_subprocess.call_args
    assert args[0][0] == (
        "claude",
        "--model",
        "sonnet",
        "--dangerously-skip-permissions",
        "--print",
    )


def test_codex_executor_success(tmp_path: Path) -> None:
    """Test CodexExecutor with successful execution."""
    config = RalphConfig.from_env(tool="codex")
    executor = CodexExecutor(config=config, working_dir=tmp_path)

    with patch("ralph.executors._run_subprocess") as mock_subprocess:
        mock_subprocess.return_value = Success("Output from codex")
        result = executor.run()

    assert isinstance(result, Success)
    assert result.unwrap() == "Output from codex"
    mock_subprocess.assert_called_once()
    args = mock_subprocess.call_args
    command = args[0][0]
    assert command[0] == "codex"
    assert command[1] == "exec"
    assert "-m" in command
    assert "gpt-5-codex" in command
    assert "--sandbox" in command
    assert "workspace-write" in command


def test_codex_executor_with_extra_args(tmp_path: Path) -> None:
    """Test CodexExecutor with CODEX_EXTRA_ARGS."""
    config = RalphConfig(
        tool="codex",
        use_mcp=False,
        taskmaster_url=None,
        codex_prompt_file=Path("CLAUDE.md"),
        codex_model="gpt-4o",
        codex_reasoning_effort="high",
        codex_sandbox="workspace-write",
        codex_full_auto=True,
        codex_extra_args="--verbose --debug",
        opencode_model="gpt-4",
        opencode_extra_args="",
    )
    executor = CodexExecutor(config=config, working_dir=tmp_path)

    with patch("ralph.executors._run_subprocess") as mock_subprocess:
        mock_subprocess.return_value = Success("Output")
        result = executor.run()

    assert isinstance(result, Success)
    args = mock_subprocess.call_args
    command = args[0][0]
    assert "--verbose" in command
    assert "--debug" in command


def test_opencode_executor_success(temp_prompt_file: Path) -> None:
    """Test OpenCodeExecutor with successful execution."""
    executor = OpenCodeExecutor(
        prompt_path=temp_prompt_file,
        working_dir=temp_prompt_file.parent,
        model="gpt-4",
    )

    with patch("ralph.executors._run_subprocess") as mock_subprocess:
        mock_subprocess.return_value = Success("Output from opencode")
        result = executor.run()

    assert isinstance(result, Success)
    assert result.unwrap() == "Output from opencode"
    mock_subprocess.assert_called_once()
    args = mock_subprocess.call_args
    assert args[0][0] == ("opencode", "--model", "gpt-4")
    assert args[1]["input_text"] == "Test prompt content"


def test_opencode_executor_with_extra_args(temp_prompt_file: Path) -> None:
    """Test OpenCodeExecutor with extra arguments."""
    executor = OpenCodeExecutor(
        prompt_path=temp_prompt_file,
        working_dir=temp_prompt_file.parent,
        model="gpt-4o",
        extra_args="--verbose --debug",
    )

    with patch("ralph.executors._run_subprocess") as mock_subprocess:
        mock_subprocess.return_value = Success("Output")
        result = executor.run()

    assert isinstance(result, Success)
    args = mock_subprocess.call_args
    command = args[0][0]
    assert command[0] == "opencode"
    assert "--model" in command
    assert "gpt-4o" in command
    assert "--verbose" in command
    assert "--debug" in command


def test_executor_error_str_representation() -> None:
    """Test ExecutorError string representation."""
    error = ExecutorError(
        detail="Command failed",
        command=("test", "command"),
        returncode=1,
        output="Error output",
    )

    error_str = str(error)
    assert "Command failed" in error_str
    assert "test command" in error_str
    assert "exit=1" in error_str
    assert "Error output" in error_str


def test_executor_error_minimal() -> None:
    """Test ExecutorError with minimal information."""
    error = ExecutorError(detail="Simple error")

    error_str = str(error)
    assert error_str == "Simple error"


def test_run_subprocess_success() -> None:
    """Test _run_subprocess with successful command."""
    from ralph.executors import _run_subprocess

    with patch("ralph.executors.subprocess.run") as mock_run:
        mock_completed = Mock()
        mock_completed.returncode = 0
        mock_run.return_value = mock_completed

        result = _run_subprocess(("echo", "test"))

    assert isinstance(result, Success)


def test_run_subprocess_failure() -> None:
    """Test _run_subprocess with failed command."""
    from ralph.executors import _run_subprocess

    with patch("ralph.executors.subprocess.run") as mock_run:
        mock_completed = Mock()
        mock_completed.returncode = 1
        mock_run.return_value = mock_completed

        result = _run_subprocess(("false",))

    assert isinstance(result, Failure)
    error = result.failure()
    assert isinstance(error, ExecutorError)
    assert error.returncode == 1


def test_run_subprocess_os_error() -> None:
    """Test _run_subprocess with OSError (command not found)."""
    from ralph.executors import _run_subprocess

    with patch("ralph.executors.subprocess.run", side_effect=OSError("Command not found")):
        result = _run_subprocess(("nonexistent-command",))

    assert isinstance(result, Failure)
    error = result.failure()
    assert isinstance(error, ExecutorError)
    assert "Failed to execute" in error.detail
