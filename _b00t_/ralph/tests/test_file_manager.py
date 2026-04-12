"""Unit tests for Ralph file management."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
from returns.maybe import Nothing, Some
from returns.result import Failure, Success

from ralph.file_manager import (
    append_to_progress,
    get_current_branch,
    initialize_progress_file,
    read_prd,
)


@pytest.fixture
def temp_prd_file(tmp_path: Path) -> Path:
    """Create a temporary PRD file for testing."""
    prd_file = tmp_path / "prd.json"
    prd_data = {
        "project": "Test Project",
        "branchName": "test/branch",
        "userStories": [],
    }
    prd_file.write_text(json.dumps(prd_data, indent=2))
    return prd_file


@pytest.fixture
def temp_progress_file(tmp_path: Path) -> Path:
    """Create a temporary progress file path."""
    return tmp_path / "progress.txt"


def test_read_prd_success(temp_prd_file: Path) -> None:
    """Test read_prd() with valid PRD file."""
    result = read_prd(temp_prd_file)

    assert isinstance(result, Success)
    data = result.unwrap()
    assert data["project"] == "Test Project"
    assert data["branchName"] == "test/branch"


def test_read_prd_file_not_found(tmp_path: Path) -> None:
    """Test read_prd() with missing file."""
    missing_file = tmp_path / "nonexistent.json"
    result = read_prd(missing_file)

    assert isinstance(result, Failure)


def test_read_prd_invalid_json(tmp_path: Path) -> None:
    """Test read_prd() with invalid JSON."""
    invalid_file = tmp_path / "invalid.json"
    invalid_file.write_text("{ invalid json }")
    result = read_prd(invalid_file)

    assert isinstance(result, Failure)


def test_get_current_branch_success(temp_prd_file: Path) -> None:
    """Test get_current_branch() with valid PRD."""
    result = get_current_branch(temp_prd_file)

    assert isinstance(result, Some)
    assert result.unwrap() == "test/branch"


def test_get_current_branch_missing_field(tmp_path: Path) -> None:
    """Test get_current_branch() when branchName is missing."""
    prd_file = tmp_path / "prd.json"
    prd_file.write_text(json.dumps({"project": "Test"}))
    result = get_current_branch(prd_file)

    assert result == Nothing


def test_get_current_branch_empty_value(tmp_path: Path) -> None:
    """Test get_current_branch() when branchName is empty."""
    prd_file = tmp_path / "prd.json"
    prd_file.write_text(json.dumps({"branchName": ""}))
    result = get_current_branch(prd_file)

    assert result == Nothing


def test_get_current_branch_invalid_file(tmp_path: Path) -> None:
    """Test get_current_branch() with invalid PRD file."""
    missing_file = tmp_path / "nonexistent.json"
    result = get_current_branch(missing_file)

    assert result == Nothing


def test_initialize_progress_file_new(temp_progress_file: Path) -> None:
    """Test initialize_progress_file() creates new file."""
    result = initialize_progress_file(temp_progress_file)

    assert isinstance(result, Success)
    assert temp_progress_file.exists()
    content = temp_progress_file.read_text()
    assert "Ralph Progress Log" in content
    assert "Started:" in content


def test_initialize_progress_file_exists(temp_progress_file: Path) -> None:
    """Test initialize_progress_file() with existing file."""
    temp_progress_file.write_text("Existing content")
    result = initialize_progress_file(temp_progress_file)

    assert isinstance(result, Success)
    content = temp_progress_file.read_text()
    assert content == "Existing content"


def test_append_to_progress_success(temp_progress_file: Path) -> None:
    """Test append_to_progress() adds content."""
    temp_progress_file.write_text("Initial content\n")
    result = append_to_progress("New line", temp_progress_file)

    assert isinstance(result, Success)
    content = temp_progress_file.read_text()
    assert content == "Initial content\nNew line\n"


def test_append_to_progress_with_newline(temp_progress_file: Path) -> None:
    """Test append_to_progress() preserves existing newline."""
    temp_progress_file.write_text("")
    result = append_to_progress("Line with newline\n", temp_progress_file)

    assert isinstance(result, Success)
    content = temp_progress_file.read_text()
    assert content == "Line with newline\n"


def test_append_to_progress_creates_file(temp_progress_file: Path) -> None:
    """Test append_to_progress() creates file if missing."""
    result = append_to_progress("First line", temp_progress_file)

    assert isinstance(result, Success)
    assert temp_progress_file.exists()
    content = temp_progress_file.read_text()
    assert content == "First line\n"


def test_append_to_progress_permission_error(tmp_path: Path) -> None:
    """Test append_to_progress() with permission error."""
    readonly_dir = tmp_path / "readonly"
    readonly_dir.mkdir()
    readonly_file = readonly_dir / "progress.txt"

    # Make directory read-only (won't work on Windows)
    import platform

    if platform.system() != "Windows":
        readonly_dir.chmod(0o555)
        result = append_to_progress("Test", readonly_file)
        readonly_dir.chmod(0o755)  # Restore permissions

        assert isinstance(result, Failure)
