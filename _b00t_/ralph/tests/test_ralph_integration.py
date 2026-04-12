"""Integration tests for Ralph CLI behaviors."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from collections.abc import Mapping, Sequence
from pathlib import Path

import pytest
from returns.result import Success

from ralph.file_manager import append_to_progress, initialize_progress_file
from ralph.runner import _check_for_completion

PROJECT_ROOT = Path(__file__).resolve().parent.parent
RUN_REAL_TOOL_TESTS = os.environ.get("RALPH_RUN_REAL_TOOL_TESTS", "0") == "1"


def has_tool(name: str) -> bool:
    """Return True when the external tool is available in PATH."""

    return shutil.which(name) is not None


def _should_run_real_tool(tool: str) -> bool:
    """Determine whether integration tests should run against the real tool."""

    return RUN_REAL_TOOL_TESTS and has_tool(tool)


def _prepare_workspace(workdir: Path) -> None:
    """Create the minimal files Ralph expects (prompt files and .taskmaster structure)."""

    (workdir / "prompt.md").write_text("# Test\n\nSay hello and exit.\n", encoding="utf-8")
    (workdir / "CLAUDE.md").write_text(
        "# Test\n\nPrint '<promise>COMPLETE</promise>' immediately.\n",
        encoding="utf-8",
    )

    # Create TaskMaster structure (replaces prd.json)
    taskmaster_dir = workdir / ".taskmaster"
    taskmaster_dir.mkdir(exist_ok=True)
    (taskmaster_dir / "tasks").mkdir(exist_ok=True)

    # Create minimal tasks.json
    (taskmaster_dir / "tasks" / "tasks.json").write_text(
        '{"tasks": [{"id":"task-001","title":"Test task","description":"d","status":"pending","priority":1}], "metadata": {"project": "Test", "branchName": "test-branch", "taskMasterVersion": "1.0"}}',
        encoding="utf-8",
    )

    # Create config.json
    (taskmaster_dir / "config.json").write_text(
        '{"version": "1.0", "model": "claude-sonnet-4-5"}',
        encoding="utf-8",
    )


def _build_env(overrides: Mapping[str, str] | None = None) -> dict[str, str]:
    """Construct an environment with PYTHONPATH pointing at the project root."""

    env = os.environ.copy()
    python_path = env.get("PYTHONPATH")
    entries = [str(PROJECT_ROOT)]
    if python_path:
        entries.append(python_path)
    env["PYTHONPATH"] = os.pathsep.join(entries)
    if overrides:
        env.update(overrides)
    return env


def _run_cli(
    workdir: Path,
    args: Sequence[str],
    *,
    env_overrides: Mapping[str, str] | None = None,
    timeout: float = 5.0,
) -> subprocess.CompletedProcess[str]:
    """Invoke the Ralph CLI in a subprocess for black-box verification."""

    command = [sys.executable, "-m", "ralph", *args]
    env = _build_env(env_overrides)
    return subprocess.run(
        command,
        cwd=workdir,
        capture_output=True,
        text=True,
        timeout=timeout,
        env=env,
    )


def _install_fake_amp(workdir: Path, command_name: str = "amp") -> dict[str, str]:
    """Install a lightweight fake tool that finishes immediately."""

    # Use existing tool stubs from tests/tool_stubs/
    stubs_dir = PROJECT_ROOT / "tests" / "tool_stubs"
    return {"PATH": f"{stubs_dir}{os.pathsep}{os.environ.get('PATH', '')}"}


def _progress_path(workdir: Path) -> Path:
    return workdir / "progress.txt"


@pytest.mark.skipif(
    not _should_run_real_tool("amp"),
    reason="amp not installed or RUN_REAL_TOOL_TESTS!=1",
)
def test_ralph_with_amp_can_start(tmp_path: Path) -> None:
    """Run Ralph with the real amp tool when available."""

    _prepare_workspace(tmp_path)
    result = _run_cli(tmp_path, ["run", "--tool", "amp", "--max-iterations", "1"], timeout=5)
    assert result.returncode in (0, 1)


@pytest.mark.skipif(
    not _should_run_real_tool("claude"),
    reason="claude not installed or RUN_REAL_TOOL_TESTS!=1",
)
def test_ralph_with_claude_can_start(tmp_path: Path) -> None:
    """Run Ralph with the real claude tool when available."""

    _prepare_workspace(tmp_path)
    result = _run_cli(tmp_path, ["run", "--tool", "claude", "--max-iterations", "1"], timeout=5)
    assert result.returncode in (0, 1)


@pytest.mark.skipif(
    not _should_run_real_tool("codex"),
    reason="codex not installed or RUN_REAL_TOOL_TESTS!=1",
)
def test_ralph_with_codex_can_start(tmp_path: Path) -> None:
    """Run Ralph with the real codex tool when available."""

    _prepare_workspace(tmp_path)
    env = {"CODEX_PROMPT_FILE": str(tmp_path / "CLAUDE.md")}
    result = _run_cli(tmp_path, ["run", "--tool", "codex", "--max-iterations", "1"], env_overrides=env, timeout=5)
    assert result.returncode in (0, 1)


def test_ralph_logs_iterations_and_creates_progress_file(tmp_path: Path) -> None:
    """Verify Ralph output contains iteration logs and initializes progress.txt."""

    _prepare_workspace(tmp_path)
    env = _install_fake_amp(tmp_path)
    result = _run_cli(tmp_path, ["run", "--tool", "amp", "--max-iterations", "1"], env_overrides=env, timeout=10)

    assert result.returncode == 0
    stderr = result.stderr
    assert "Ralph Iteration 1 of 1" in stderr
    assert "Ralph completed all tasks" in stderr

    progress_path = _progress_path(tmp_path)
    assert progress_path.exists()
    content = progress_path.read_text(encoding="utf-8")
    assert "# Ralph Progress Log" in content
    assert "Started:" in content


def test_progress_file_can_be_updated_after_cli_run(tmp_path: Path) -> None:
    """Ensure progress.txt supports appends after Ralph runs."""

    _prepare_workspace(tmp_path)
    env = _install_fake_amp(tmp_path)
    _run_cli(tmp_path, ["run", "--tool", "amp", "--max-iterations", "1"], env_overrides=env, timeout=10)

    progress_path = _progress_path(tmp_path)
    result = append_to_progress("## Integration entry", progress_path)
    assert isinstance(result, Success)

    content = progress_path.read_text(encoding="utf-8")
    assert "## Integration entry" in content


def test_completion_signal_detection() -> None:
    """Verify <promise>COMPLETE</promise> detection remains stable."""

    assert _check_for_completion("Output\n<promise>COMPLETE</promise>\nMore") is True
    assert _check_for_completion("No signal here") is False
    assert _check_for_completion("") is False
    assert _check_for_completion("<promise>complete</promise>") is False


def test_help_flag() -> None:
    """Verify `ralph --help` exits successfully and shows subcommands."""

    result = subprocess.run(
        [sys.executable, "-m", "ralph", "--help"],
        capture_output=True,
        text=True,
        timeout=10,
        cwd=PROJECT_ROOT,
    )

    assert result.returncode == 0
    assert "usage:" in result.stdout.lower()
    # Check for subcommand structure
    assert "run" in result.stdout
    assert "status" in result.stdout
    assert "list-tasks" in result.stdout


def test_version_flag() -> None:
    """Verify `ralph --version` reports the CLI version."""

    result = subprocess.run(
        [sys.executable, "-m", "ralph", "--version"],
        capture_output=True,
        text=True,
        timeout=10,
        cwd=PROJECT_ROOT,
    )

    assert result.returncode == 0
    assert "0.1.0" in result.stdout or "ralph" in result.stdout.lower()


def test_progress_file_initialization(tmp_path: Path) -> None:
    """Verify progress.txt initialization creates a well-formed file."""

    progress_path = tmp_path / "progress.txt"
    result = initialize_progress_file(progress_path)

    assert isinstance(result, Success)
    assert progress_path.exists()

    content = progress_path.read_text(encoding="utf-8")
    assert "# Ralph Progress Log" in content
    assert "Started:" in content
    assert "---" in content


def test_progress_file_append(tmp_path: Path) -> None:
    """Verify appending to progress.txt works correctly."""

    progress_path = tmp_path / "progress.txt"
    initialize_progress_file(progress_path)

    result = append_to_progress("## Test message", progress_path)
    assert isinstance(result, Success)

    content = progress_path.read_text(encoding="utf-8")
    assert "## Test message" in content
