"""Unit tests for Ralph CLI entrypoint."""

from __future__ import annotations

import sys
from pathlib import Path
from unittest.mock import patch

import pytest

from ralph.ralph_cli import main


def test_main_run_default_arguments() -> None:
    """Test main() with 'run' subcommand and default arguments."""
    with (
        patch.object(sys, "argv", ["ralph", "run"]),
        patch("ralph.runner.run_ralph", return_value=0) as mock_run,
    ):
        exit_code = main()

    assert exit_code == 0
    mock_run.assert_called_once()
    args = mock_run.call_args
    config = args[0][0]
    max_iterations = args[0][1]
    assert config.tool == "amp"
    assert max_iterations == 10


def test_main_run_with_tool_argument() -> None:
    """Test main() with 'run' subcommand and --tool argument."""
    with (
        patch.object(sys, "argv", ["ralph", "run", "--tool", "codex"]),
        patch("ralph.runner.run_ralph", return_value=0) as mock_run,
    ):
        exit_code = main()

    assert exit_code == 0
    args = mock_run.call_args
    config = args[0][0]
    assert config.tool == "codex"


def test_main_run_with_max_iterations() -> None:
    """Test main() with 'run' subcommand and --max-iterations argument."""
    with (
        patch.object(sys, "argv", ["ralph", "run", "--max-iterations", "5"]),
        patch("ralph.runner.run_ralph", return_value=0) as mock_run,
    ):
        exit_code = main()

    assert exit_code == 0
    args = mock_run.call_args
    max_iterations = args[0][1]
    assert max_iterations == 5


def test_main_run_with_tool_and_iterations() -> None:
    """Test main() with 'run' subcommand, --tool and --max-iterations."""
    with (
        patch.object(
            sys,
            "argv",
            ["ralph", "run", "--tool", "claude", "--max-iterations", "15"],
        ),
        patch("ralph.runner.run_ralph", return_value=0) as mock_run,
    ):
        exit_code = main()

    assert exit_code == 0
    args = mock_run.call_args
    config = args[0][0]
    max_iterations = args[0][1]
    assert config.tool == "claude"
    assert max_iterations == 15


def test_main_run_dry_run() -> None:
    """Test main() with 'run' subcommand and --dry-run flag."""
    with patch.object(sys, "argv", ["ralph", "run", "--dry-run"]):
        exit_code = main()

    assert exit_code == 0


def test_main_status_subcommand() -> None:
    """Test main() with 'status' subcommand."""
    with (
        patch.object(sys, "argv", ["ralph", "status"]),
        patch("ralph.taskmaster_adapter.FileTaskMasterClient.get_all_tasks") as mock_get,
        patch("builtins.print"),
    ):
        from returns.result import Success

        from ralph.taskmaster_adapter import Task

        mock_get.return_value = Success([
            Task(
                id="task-001",
                title="Test Task",
                description="Test",
                status="pending",
                priority=1,
                acceptance_criteria=[],
                depends_on=[],
                blocked_by=[],
                notes=[],
                created_at="2026-01-01T00:00:00Z",
                updated_at="2026-01-01T00:00:00Z",
            )
        ])
        exit_code = main()

    assert exit_code == 0


def test_main_list_tasks_subcommand() -> None:
    """Test main() with 'list-tasks' subcommand."""
    with (
        patch.object(sys, "argv", ["ralph", "list-tasks", "--filter", "pending"]),
        patch("ralph.taskmaster_adapter.FileTaskMasterClient.get_all_tasks") as mock_get,
        patch("builtins.print"),
    ):
        from returns.result import Success

        from ralph.taskmaster_adapter import Task

        mock_get.return_value = Success([
            Task(
                id="task-001",
                title="Test Task",
                description="Test",
                status="pending",
                priority=1,
                acceptance_criteria=[],
                depends_on=[],
                blocked_by=[],
                notes=[],
                created_at="2026-01-01T00:00:00Z",
                updated_at="2026-01-01T00:00:00Z",
            )
        ])
        exit_code = main()

    assert exit_code == 0


def test_main_version_flag() -> None:
    """Test main() with --version flag."""
    with (
        patch.object(sys, "argv", ["ralph", "--version"]),
        pytest.raises(SystemExit) as exc_info,
    ):
        main()

    assert exc_info.value.code == 0


def test_main_invalid_tool() -> None:
    """Test main() with invalid tool choice."""
    with (
        patch.object(sys, "argv", ["ralph", "run", "--tool", "invalid"]),
        pytest.raises(SystemExit) as exc_info,
    ):
        main()

    assert exc_info.value.code == 2


def test_main_returns_nonzero_on_failure() -> None:
    """Test main() returns non-zero exit code on failure."""
    with (
        patch.object(sys, "argv", ["ralph", "run"]),
        patch("ralph.runner.run_ralph", return_value=1),
    ):
        exit_code = main()

    assert exit_code == 1


def test_main_runs_with_agent(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    """Test main() runs with subcommand syntax."""
    monkeypatch.chdir(tmp_path)
    (tmp_path / "progress.txt").touch()
    (tmp_path / ".taskmaster" / "tasks").mkdir(parents=True)
    (tmp_path / ".taskmaster" / "tasks" / "tasks.json").write_text(
        '{"tasks":[{"id":"task-001","title":"t","description":"d","status":"pending","priority":1}],"metadata":{"project":"t","branchName":"b","taskMasterVersion":"1.0"}}'
    )

    # Mock runner.run_ralph to return success without actual execution
    with patch("ralph.runner.run_ralph", return_value=0) as mock_run:
        exit_code = main(["run", "--tool", "amp", "--max-iterations", "1"])

    assert exit_code == 0
    assert (tmp_path / "progress.txt").exists()
    mock_run.assert_called_once()


def test_main_defaults_to_sys_argv(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> None:
    """Test main() defaults to reading from sys.argv with subcommand syntax."""
    monkeypatch.chdir(tmp_path)
    (tmp_path / "progress.txt").touch()
    (tmp_path / ".taskmaster" / "tasks").mkdir(parents=True)
    (tmp_path / ".taskmaster" / "tasks" / "tasks.json").write_text(
        '{"tasks":[{"id":"task-001","title":"t","description":"d","status":"pending","priority":1}],"metadata":{"project":"t","branchName":"b","taskMasterVersion":"1.0"}}'
    )

    # Mock runner.run_ralph to return success without actual execution
    with (
        patch("ralph.runner.run_ralph", return_value=0) as mock_run,
        patch.object(sys, "argv", ["ralph", "run", "--tool", "amp", "--max-iterations", "1"]),
    ):
        exit_code = main()

    assert exit_code == 0
    mock_run.assert_called_once()
