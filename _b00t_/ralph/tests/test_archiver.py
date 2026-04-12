"""Unit tests for Ralph branch archiving."""

from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import patch

import pytest
from returns.maybe import Nothing, Some
from returns.result import Success

from ralph.archiver import (
    archive_previous_run,
    check_branch_change,
)


@pytest.fixture
def mock_project_root(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    """Mock the project root for archiver tests."""
    prd_file = tmp_path / "prd.json"
    progress_file = tmp_path / "progress.txt"
    last_branch_file = tmp_path / ".last-branch"

    # Create initial files
    prd_file.write_text(json.dumps({"branchName": "main"}))
    progress_file.write_text("Initial progress\n")

    # Patch module-level constants
    monkeypatch.setattr("ralph.archiver.PACKAGE_DIR", tmp_path)
    monkeypatch.setattr("ralph.archiver.ARCHIVE_DIR", tmp_path / "archive")
    monkeypatch.setattr("ralph.archiver.LAST_BRANCH_PATH", last_branch_file)
    monkeypatch.setattr("ralph.archiver.PRD_PATH", prd_file)
    monkeypatch.setattr("ralph.archiver.PROGRESS_PATH", progress_file)

    return tmp_path


def test_archive_previous_run_success(mock_project_root: Path) -> None:
    """Test archive_previous_run() creates archive directory."""
    result = archive_previous_run("old/branch", "new/branch")

    assert isinstance(result, Success)
    archive_dir = result.unwrap()
    assert archive_dir.exists()
    assert "old-branch" in archive_dir.name

    # Check archived files
    archived_prd = list(archive_dir.glob("prd.json"))
    archived_progress = list(archive_dir.glob("progress.txt"))
    assert len(archived_prd) == 1
    assert len(archived_progress) == 1


def test_archive_previous_run_same_branch(mock_project_root: Path) -> None:
    """Test archive_previous_run() when branches are the same."""
    archive_dir_before = mock_project_root / "archive"

    result = archive_previous_run("main", "main")

    assert isinstance(result, Success)
    # No archive should be created
    if archive_dir_before.exists():
        archives = list(archive_dir_before.iterdir())
        assert len(archives) == 0


def test_archive_previous_run_empty_branch(mock_project_root: Path) -> None:
    """Test archive_previous_run() with empty branch name."""
    result = archive_previous_run("", "new/branch")

    assert isinstance(result, Success)


def test_check_branch_change_first_run(
    mock_project_root: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Test check_branch_change() on first run (no .last-branch file)."""
    last_branch_file = mock_project_root / ".last-branch"
    prd_file = mock_project_root / "prd.json"
    prd_file.write_text(json.dumps({"branchName": "main"}))

    # Ensure .last-branch doesn't exist
    if last_branch_file.exists():
        last_branch_file.unlink()

    with patch("ralph.archiver.get_current_branch", return_value=Some("main")):
        result = check_branch_change()

    assert result is False
    # .last-branch should be created
    assert last_branch_file.exists()
    assert last_branch_file.read_text().strip() == "main"


def test_check_branch_change_no_change(
    mock_project_root: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Test check_branch_change() when branch hasn't changed."""
    last_branch_file = mock_project_root / ".last-branch"
    last_branch_file.write_text("main")

    with patch("ralph.archiver.get_current_branch", return_value=Some("main")):
        result = check_branch_change()

    assert result is False


def test_check_branch_change_detected(
    mock_project_root: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    """Test check_branch_change() when branch has changed."""
    last_branch_file = mock_project_root / ".last-branch"
    progress_file = mock_project_root / "progress.txt"
    last_branch_file.write_text("old/branch")
    progress_file.write_text("Old progress\n")

    with patch("ralph.archiver.get_current_branch", return_value=Some("new/branch")):
        result = check_branch_change()

    assert result is True
    # Check that .last-branch was updated
    assert last_branch_file.read_text().strip() == "new/branch"
    # Check that progress was reset
    assert progress_file.read_text() == ""


def test_check_branch_change_no_prd(mock_project_root: Path) -> None:
    """Test check_branch_change() when PRD file is missing."""
    prd_file = mock_project_root / "prd.json"
    prd_file.unlink()

    with patch("ralph.archiver.get_current_branch", return_value=Nothing):
        result = check_branch_change()

    assert result is False


def test_sanitize_branch_name() -> None:
    """Test _sanitize_branch_name() with various inputs."""
    from ralph.archiver import _sanitize_branch_name

    assert _sanitize_branch_name("feature/my-feature") == "feature-my-feature"
    assert _sanitize_branch_name("ralph/python-rewrite") == "python-rewrite"
    assert _sanitize_branch_name("main") == "main"
    assert _sanitize_branch_name("  feature/test  ") == "feature-test"
    assert _sanitize_branch_name("") == "unknown-branch"
    assert _sanitize_branch_name("  ") == "unknown-branch"
    assert _sanitize_branch_name("a b c") == "a-b-c"
