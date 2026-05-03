"""Unit tests for Ralph backlog adapter module."""

from __future__ import annotations

import json
from pathlib import Path
from unittest.mock import MagicMock, patch

import pytest
from returns.result import Failure, Success

from ralph.taskmaster_adapter import (
    CLITaskMasterClient,
    FileTaskMasterClient,
    MCPTaskMasterClient,
    Task,
    create_client,
    get_current_branch,
)


@pytest.fixture
def sample_task_data() -> dict:
    """Sample task data for testing."""
    return {
        "id": "task-001",
        "title": "Test Task",
        "description": "A test task",
        "status": "pending",
        "priority": 1,
        "acceptanceCriteria": ["criteria 1", "criteria 2"],
        "dependsOn": [],
        "blockedBy": [],
        "notes": [],
        "createdAt": "2026-02-01T00:00:00Z",
        "updatedAt": "2026-02-01T00:00:00Z",
    }


@pytest.fixture
def sample_tasks_json(tmp_path: Path, sample_task_data: dict) -> Path:
    """Create a sample tasks.json file."""
    tasks_file = tmp_path / "tasks.json"
    tasks_data = {
        "tasks": [
            sample_task_data,
            {
                "id": "task-002",
                "title": "Blocked Task",
                "description": "This task is blocked",
                "status": "pending",
                "priority": 2,
                "acceptanceCriteria": [],
                "dependsOn": ["task-001"],
                "blockedBy": ["task-001"],
                "notes": [],
                "createdAt": "2026-02-01T00:00:00Z",
                "updatedAt": "2026-02-01T00:00:00Z",
            },
            {
                "id": "task-003",
                "title": "Completed Task",
                "description": "This task is done",
                "status": "done",
                "priority": 0,
                "acceptanceCriteria": [],
                "dependsOn": [],
                "blockedBy": [],
                "notes": ["2026-02-01T12:00:00Z: Completed successfully"],
                "createdAt": "2026-02-01T00:00:00Z",
                "updatedAt": "2026-02-01T12:00:00Z",
            },
        ],
        "metadata": {
            "project": "test-project",
            "branchName": "main",
        },
    }
    tasks_file.write_text(json.dumps(tasks_data, indent=2))
    return tasks_file


@pytest.fixture
def sample_todo_next(tmp_path: Path) -> Path:
    """Create a sample TODO-next.md backlog."""
    backlog = tmp_path / "TODO-next.md"
    backlog.write_text(
        "# TODO-next\n\n## Critical path\n- [ ] First backlog item\n- [x] Done backlog item\n"
    )
    return backlog


# Task class tests


def test_task_from_dict(sample_task_data: dict) -> None:
    """Test Task.from_dict() creates Task from dictionary."""
    task = Task.from_dict(sample_task_data)
    assert task.id == "task-001"
    assert task.title == "Test Task"
    assert task.description == "A test task"
    assert task.status == "pending"
    assert task.priority == 1
    assert task.acceptance_criteria == ["criteria 1", "criteria 2"]
    assert task.depends_on == []
    assert task.blocked_by == []
    assert task.notes == []


def test_task_to_dict(sample_task_data: dict) -> None:
    """Test Task.to_dict() converts Task to dictionary."""
    task = Task.from_dict(sample_task_data)
    result = task.to_dict()
    assert result["id"] == "task-001"
    assert result["title"] == "Test Task"
    assert result["status"] == "pending"
    assert result["priority"] == 1
    assert result["acceptanceCriteria"] == ["criteria 1", "criteria 2"]


def test_task_from_dict_with_defaults() -> None:
    """Test Task.from_dict() handles missing fields with defaults."""
    minimal_data = {"id": "task-minimal"}
    task = Task.from_dict(minimal_data)
    assert task.id == "task-minimal"
    assert task.title == ""
    assert task.description == ""
    assert task.status == "pending"
    assert task.priority == 0
    assert task.acceptance_criteria == []
    assert task.depends_on == []
    assert task.blocked_by == []
    assert task.notes == []


# FileTaskMasterClient tests


def test_file_client_get_all_tasks(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.get_all_tasks() reads tasks from file."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.get_all_tasks()
    assert isinstance(result, Success)
    tasks = result.unwrap()
    assert len(tasks) == 3
    assert tasks[0].id == "task-001"
    assert tasks[1].id == "task-002"
    assert tasks[2].id == "task-003"


def test_file_client_get_all_tasks_file_not_found(tmp_path: Path) -> None:
    """Test FileTaskMasterClient.get_all_tasks() returns Failure for missing file."""
    client = FileTaskMasterClient(tasks_file=tmp_path / "nonexistent.json")
    result = client.get_all_tasks()
    assert isinstance(result, Failure)
    assert "Tasks file not found" in str(result.failure())


def test_file_client_get_next_task(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.get_next_task() returns highest priority unblocked task."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.get_next_task()
    assert isinstance(result, Success)
    task = result.unwrap()
    assert task.id == "task-001"  # priority 1, not blocked
    assert task.status == "pending"


def test_file_client_get_next_task_no_available(tmp_path: Path) -> None:
    """Test FileTaskMasterClient.get_next_task() returns Failure when no tasks available."""
    tasks_file = tmp_path / "tasks.json"
    tasks_data = {
        "tasks": [
            {
                "id": "task-blocked",
                "title": "Blocked",
                "status": "pending",
                "priority": 1,
                "blockedBy": ["task-other"],
                "dependsOn": [],
                "acceptanceCriteria": [],
                "notes": [],
                "description": "",
                "createdAt": "2026-02-01T00:00:00Z",
                "updatedAt": "2026-02-01T00:00:00Z",
            }
        ],
        "metadata": {},
    }
    tasks_file.write_text(json.dumps(tasks_data))
    client = FileTaskMasterClient(tasks_file=tasks_file)
    result = client.get_next_task()
    assert isinstance(result, Failure)
    assert "No available tasks" in str(result.failure())


def test_file_client_get_task_by_id(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.get_task_by_id() retrieves specific task."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.get_task_by_id("task-002")
    assert isinstance(result, Success)
    task = result.unwrap()
    assert task.id == "task-002"
    assert task.title == "Blocked Task"


def test_file_client_get_task_by_id_not_found(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.get_task_by_id() returns Failure for nonexistent task."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.get_task_by_id("task-nonexistent")
    assert isinstance(result, Failure)
    assert "not found" in str(result.failure())


def test_file_client_update_task_status(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.update_task_status() updates task status."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.update_task_status("task-001", "in-progress")
    assert isinstance(result, Success)

    # Verify the file was updated
    data = json.loads(sample_tasks_json.read_text())
    task_data = next(t for t in data["tasks"] if t["id"] == "task-001")
    assert task_data["status"] == "in-progress"
    assert "updatedAt" in task_data


def test_file_client_update_task_status_not_found(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.update_task_status() returns Failure for nonexistent task."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.update_task_status("task-nonexistent", "done")
    assert isinstance(result, Failure)
    assert "not found" in str(result.failure())


def test_file_client_add_task_note(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.add_task_note() adds timestamped note."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.add_task_note("task-001", "Test note")
    assert isinstance(result, Success)

    # Verify the file was updated
    data = json.loads(sample_tasks_json.read_text())
    task_data = next(t for t in data["tasks"] if t["id"] == "task-001")
    assert len(task_data["notes"]) == 1
    assert "Test note" in task_data["notes"][0]
    assert ":" in task_data["notes"][0]  # Timestamp format


def test_file_client_add_task_note_not_found(sample_tasks_json: Path) -> None:
    """Test FileTaskMasterClient.add_task_note() returns Failure for nonexistent task."""
    client = FileTaskMasterClient(tasks_file=sample_tasks_json)
    result = client.add_task_note("task-nonexistent", "Note")
    assert isinstance(result, Failure)
    assert "not found" in str(result.failure())


# CLITaskMasterClient tests


def test_cli_client_get_all_tasks_success() -> None:
    """Test CLITaskMasterClient.get_all_tasks() via subprocess."""
    mock_result = MagicMock()
    mock_result.stdout = json.dumps({
        "tasks": [
            {
                "id": "task-cli-001",
                "title": "CLI Task",
                "description": "",
                "status": "pending",
                "priority": 1,
                "acceptanceCriteria": [],
                "dependsOn": [],
                "blockedBy": [],
                "notes": [],
                "createdAt": "2026-02-01T00:00:00Z",
                "updatedAt": "2026-02-01T00:00:00Z",
            }
        ]
    })

    with patch("subprocess.run", return_value=mock_result):
        client = CLITaskMasterClient()
        result = client.get_all_tasks()
        assert isinstance(result, Success)
        tasks = result.unwrap()
        assert len(tasks) == 1
        assert tasks[0].id == "task-cli-001"


def test_cli_client_get_all_tasks_not_found() -> None:
    """Test CLITaskMasterClient.get_all_tasks() handles missing CLI."""
    with patch("subprocess.run", side_effect=FileNotFoundError):
        client = CLITaskMasterClient()
        result = client.get_all_tasks()
        assert isinstance(result, Failure)
        assert "task-master CLI not found" in str(result.failure())


def test_cli_client_update_task_status_success() -> None:
    """Test CLITaskMasterClient.update_task_status() via subprocess."""
    with patch("subprocess.run") as mock_run:
        client = CLITaskMasterClient()
        result = client.update_task_status("task-001", "done")
        assert isinstance(result, Success)
        mock_run.assert_called_once()
        args = mock_run.call_args[0][0]
        assert "task-master" in args
        assert "update" in args
        assert "task-001" in args
        assert "done" in args


def test_cli_client_get_task_by_id_success() -> None:
    """Test CLITaskMasterClient.get_task_by_id() via subprocess."""
    mock_result = MagicMock()
    mock_result.stdout = json.dumps({
        "id": "task-cli-001",
        "title": "CLI Task",
        "description": "",
        "status": "pending",
        "priority": 1,
        "acceptanceCriteria": [],
        "dependsOn": [],
        "blockedBy": [],
        "notes": [],
        "createdAt": "2026-02-01T00:00:00Z",
        "updatedAt": "2026-02-01T00:00:00Z",
    })

    with patch("subprocess.run", return_value=mock_result):
        client = CLITaskMasterClient()
        result = client.get_task_by_id("task-cli-001")
        assert isinstance(result, Success)
        task = result.unwrap()
        assert task.id == "task-cli-001"


def test_cli_client_get_next_task_success() -> None:
    """Test CLITaskMasterClient.get_next_task() via subprocess."""
    mock_result = MagicMock()
    mock_result.stdout = json.dumps({
        "tasks": [
            {
                "id": "task-pending",
                "title": "Pending Task",
                "description": "",
                "status": "pending",
                "priority": 1,
                "acceptanceCriteria": [],
                "dependsOn": [],
                "blockedBy": [],
                "notes": [],
                "createdAt": "2026-02-01T00:00:00Z",
                "updatedAt": "2026-02-01T00:00:00Z",
            }
        ]
    })

    with patch("subprocess.run", return_value=mock_result):
        client = CLITaskMasterClient()
        result = client.get_next_task()
        assert isinstance(result, Success)
        task = result.unwrap()
        assert task.id == "task-pending"


def test_cli_client_add_task_note_success() -> None:
    """Test CLITaskMasterClient.add_task_note() via subprocess."""
    with patch("subprocess.run") as mock_run:
        client = CLITaskMasterClient()
        result = client.add_task_note("task-001", "Test note")
        assert isinstance(result, Success)
        mock_run.assert_called_once()
        args = mock_run.call_args[0][0]
        assert "task-master" in args
        assert "add-note" in args
        assert "task-001" in args


def test_cli_client_get_task_by_id_cli_error() -> None:
    """Test CLITaskMasterClient.get_task_by_id() handles CalledProcessError."""
    import subprocess
    with patch("subprocess.run", side_effect=subprocess.CalledProcessError(1, "cmd", stderr="error")):
        client = CLITaskMasterClient()
        result = client.get_task_by_id("task-001")
        assert isinstance(result, Failure)
        assert "task-master get failed" in str(result.failure())


def test_cli_client_update_task_status_cli_error() -> None:
    """Test CLITaskMasterClient.update_task_status() handles CalledProcessError."""
    import subprocess
    with patch("subprocess.run", side_effect=subprocess.CalledProcessError(1, "cmd", stderr="error")):
        client = CLITaskMasterClient()
        result = client.update_task_status("task-001", "done")
        assert isinstance(result, Failure)
        assert "task-master update failed" in str(result.failure())


def test_cli_client_add_task_note_cli_error() -> None:
    """Test CLITaskMasterClient.add_task_note() handles CalledProcessError."""
    import subprocess
    with patch("subprocess.run", side_effect=subprocess.CalledProcessError(1, "cmd", stderr="error")):
        client = CLITaskMasterClient()
        result = client.add_task_note("task-001", "note")
        assert isinstance(result, Failure)
        assert "task-master add-note failed" in str(result.failure())


def test_cli_client_get_all_tasks_cli_error() -> None:
    """Test CLITaskMasterClient.get_all_tasks() handles CalledProcessError."""
    import subprocess
    with patch("subprocess.run", side_effect=subprocess.CalledProcessError(1, "cmd", stderr="error")):
        client = CLITaskMasterClient()
        result = client.get_all_tasks()
        assert isinstance(result, Failure)
        assert "task-master list failed" in str(result.failure())


# MCPTaskMasterClient tests


def test_mcp_client_not_implemented() -> None:
    """Test MCPTaskMasterClient raises NotImplementedError for all methods."""
    client = MCPTaskMasterClient()

    result = client.get_all_tasks()
    assert isinstance(result, Failure)
    assert isinstance(result.failure(), NotImplementedError)

    result = client.get_next_task()
    assert isinstance(result, Failure)

    result = client.get_task_by_id("task-001")
    assert isinstance(result, Failure)

    result = client.update_task_status("task-001", "done")
    assert isinstance(result, Failure)

    result = client.add_task_note("task-001", "note")
    assert isinstance(result, Failure)


# Factory function tests


def test_create_client_file_based_by_default(tmp_path: Path) -> None:
    """Test create_client() returns FileTaskMasterClient by default."""
    tasks_file = tmp_path / "tasks.json"
    tasks_file.write_text(json.dumps({"tasks": [], "metadata": {}}))
    client = create_client(prefer_mcp=False, tasks_file=tasks_file)
    assert isinstance(client, FileTaskMasterClient)


def test_markdown_backlog_is_parsed(sample_todo_next: Path) -> None:
    """Test FileTaskMasterClient parses TODO-next.md checklist items."""
    client = FileTaskMasterClient(tasks_file=sample_todo_next)
    result = client.get_all_tasks()
    assert isinstance(result, Success)
    tasks = result.unwrap()
    assert [task.id for task in tasks] == ["todo-4", "todo-5"]
    assert tasks[0].title == "First backlog item"
    assert tasks[0].status == "pending"


def test_create_client_mcp_fallback_to_file(tmp_path: Path) -> None:
    """Test create_client() falls back to FileTaskMasterClient when MCP fails."""
    tasks_file = tmp_path / "tasks.json"
    tasks_file.write_text(json.dumps({"tasks": [], "metadata": {}}))
    client = create_client(prefer_mcp=True, tasks_file=tasks_file)
    # MCP not implemented, should fall back to file
    assert isinstance(client, FileTaskMasterClient)


# get_current_branch tests


def test_get_current_branch_success() -> None:
    """Test get_current_branch() via subprocess."""
    mock_result = MagicMock()
    mock_result.stdout = "main\n"

    with patch("subprocess.run", return_value=mock_result):
        from returns.maybe import Some
        result = get_current_branch()
        assert isinstance(result, Some)
        assert result.unwrap() == "main"


def test_get_current_branch_failure() -> None:
    """Test get_current_branch() returns Nothing on failure."""
    from returns.maybe import Nothing
    with patch("subprocess.run", side_effect=Exception("error")):
        result = get_current_branch()
        assert result == Nothing
