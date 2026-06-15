"""Backlog adapter for Ralph task management.

Primary source is TODO-next.md markdown backlog.
Legacy TaskMaster-style JSON task files remain readable for compatibility.
"""

from __future__ import annotations

import json
import subprocess
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Any, Protocol

from returns.maybe import Maybe, Nothing, Some
from returns.result import Failure, Result, Success


@dataclass(frozen=True)
class Task:
    """Represents a single task from TaskMaster."""

    id: str
    title: str
    description: str
    status: str  # pending, in-progress, done, review, cancelled
    priority: int
    acceptance_criteria: list[str]
    depends_on: list[str]
    blocked_by: list[str]  # Auto-computed from dependencies
    notes: list[str]
    created_at: str
    updated_at: str

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> Task:
        """Create Task from dictionary."""
        return cls(
            id=data.get("id", ""),
            title=data.get("title", ""),
            description=data.get("description", ""),
            status=data.get("status", "pending"),
            priority=data.get("priority", 0),
            acceptance_criteria=data.get("acceptanceCriteria", []),
            depends_on=data.get("dependsOn", []),
            blocked_by=data.get("blockedBy", []),
            notes=data.get("notes", []),
            created_at=data.get("createdAt", datetime.now().isoformat()),
            updated_at=data.get("updatedAt", datetime.now().isoformat()),
        )

    def to_dict(self) -> dict[str, Any]:
        """Convert Task to dictionary for JSON serialization."""
        return {
            "id": self.id,
            "title": self.title,
            "description": self.description,
            "status": self.status,
            "priority": self.priority,
            "acceptanceCriteria": self.acceptance_criteria,
            "dependsOn": self.depends_on,
            "blockedBy": self.blocked_by,
            "notes": self.notes,
            "createdAt": self.created_at,
            "updatedAt": self.updated_at,
        }


class TaskMasterClient(Protocol):
    """Protocol for backlog management operations."""

    def get_next_task(self) -> Result[Task, Exception]:
        """Get the next available task (highest priority, not blocked)."""
        ...

    def get_task_by_id(self, task_id: str) -> Result[Task, Exception]:
        """Get a specific task by ID."""
        ...

    def update_task_status(
        self, task_id: str, status: str
    ) -> Result[None, Exception]:
        """Update task status (pending, in-progress, done, etc.)."""
        ...

    def add_task_note(
        self, task_id: str, note: str
    ) -> Result[None, Exception]:
        """Add a timestamped note to a task."""
        ...

    def get_all_tasks(self) -> Result[list[Task], Exception]:
        """Get all tasks from the task list."""
        ...


@dataclass(frozen=True, slots=True)
class FileTaskMasterClient:
    """File-based backlog client for TODO-next.md or legacy JSON task files."""

    tasks_file: Path = Path("TODO-next.md")

    def _is_markdown_backlog(self) -> bool:
        return self.tasks_file.suffix.lower() in {".md", ".markdown"}

    def _read_markdown_tasks(self) -> list[Task]:
        tasks: list[Task] = []
        lines = self.tasks_file.read_text().splitlines()
        section = "backlog"

        for idx, line in enumerate(lines, start=1):
            stripped = line.strip()
            if stripped.startswith("#"):
                section = stripped.lstrip("#").strip() or section
                continue

            status = None
            if stripped.startswith("- [ ] "):
                status = "pending"
                title = stripped[6:].strip()
            elif stripped.startswith("- [x] ") or stripped.startswith("- [X] "):
                status = "done"
                title = stripped[6:].strip()
            else:
                continue

            tasks.append(
                Task(
                    id=f"todo-{idx}",
                    title=title,
                    description=f"{section}: {title}",
                    status=status,
                    priority=idx,
                    acceptance_criteria=[],
                    depends_on=[],
                    blocked_by=[],
                    notes=[],
                    created_at=datetime.now().isoformat(),
                    updated_at=datetime.now().isoformat(),
                )
            )

        return tasks

    def _write_markdown_status(self, task_id: str, status: str) -> Result[None, Exception]:
        try:
            target_line = int(task_id.removeprefix("todo-"))
        except ValueError:
            return Failure(Exception(f"Unknown markdown backlog task id: {task_id}"))

        lines = self.tasks_file.read_text().splitlines()
        if target_line < 1 or target_line > len(lines):
            return Failure(Exception(f"Task {task_id} not found"))

        line = lines[target_line - 1]
        if "- [ ] " not in line and "- [x] " not in line and "- [X] " not in line:
            return Failure(Exception(f"Task {task_id} is not a checklist item"))

        checked = status in {"done", "completed", "complete"}
        prefix_src = "- [x] " if "- [x] " in line else "- [X] " if "- [X] " in line else "- [ ] "
        lines[target_line - 1] = line.replace(prefix_src, "- [x] " if checked else "- [ ] ", 1)
        self.tasks_file.write_text("\n".join(lines) + "\n")
        return Success(None)

    def get_next_task(self) -> Result[Task, Exception]:
        """Get the next available task from the backlog."""
        tasks_result = self.get_all_tasks()
        if isinstance(tasks_result, Failure):
            return tasks_result

        tasks = tasks_result.unwrap()

        # Filter for pending tasks that aren't blocked
        available = [
            t for t in tasks
            if t.status == "pending" and not t.blocked_by
        ]

        if not available:
            return Failure(Exception("No available tasks"))

        # Sort by priority (lowest number = highest priority)
        available.sort(key=lambda t: t.priority)

        return Success(available[0])

    def get_task_by_id(self, task_id: str) -> Result[Task, Exception]:
        """Get a specific task by ID from the backlog."""
        tasks_result = self.get_all_tasks()
        if isinstance(tasks_result, Failure):
            return tasks_result

        tasks = tasks_result.unwrap()
        for task in tasks:
            if task.id == task_id:
                return Success(task)

        return Failure(Exception(f"Task {task_id} not found"))

    def update_task_status(
        self, task_id: str, status: str
    ) -> Result[None, Exception]:
        """Update task status in markdown backlog or legacy JSON."""
        if self._is_markdown_backlog():
            return self._write_markdown_status(task_id, status)

        try:
            # Read current data
            data = json.loads(self.tasks_file.read_text())
            tasks_data = data.get("tasks", [])

            # Find and update task
            found = False
            for task_dict in tasks_data:
                if task_dict.get("id") == task_id:
                    task_dict["status"] = status
                    task_dict["updatedAt"] = datetime.now().isoformat()
                    found = True
                    break

            if not found:
                return Failure(Exception(f"Task {task_id} not found"))

            # Write back
            data["tasks"] = tasks_data
            self.tasks_file.write_text(json.dumps(data, indent=2) + "\n")
            return Success(None)

        except Exception as exc:
            return Failure(exc)

    def add_task_note(
        self, task_id: str, note: str
    ) -> Result[None, Exception]:
        """Add a timestamped note to a legacy JSON task file."""
        if self._is_markdown_backlog():
            return Success(None)

        try:
            # Read current data
            data = json.loads(self.tasks_file.read_text())
            tasks_data = data.get("tasks", [])

            # Find and update task
            found = False
            timestamped_note = f"{datetime.now().isoformat()}: {note}"
            for task_dict in tasks_data:
                if task_dict.get("id") == task_id:
                    notes = task_dict.get("notes", [])
                    notes.append(timestamped_note)
                    task_dict["notes"] = notes
                    task_dict["updatedAt"] = datetime.now().isoformat()
                    found = True
                    break

            if not found:
                return Failure(Exception(f"Task {task_id} not found"))

            # Write back
            data["tasks"] = tasks_data
            self.tasks_file.write_text(json.dumps(data, indent=2) + "\n")
            return Success(None)

        except Exception as exc:
            return Failure(exc)

    def get_all_tasks(self) -> Result[list[Task], Exception]:
        """Get all tasks from TODO-next.md or legacy JSON."""
        try:
            if not self.tasks_file.exists():
                return Failure(Exception(f"Tasks file not found: {self.tasks_file}"))

            if self._is_markdown_backlog():
                tasks = self._read_markdown_tasks()
                if not tasks:
                    return Failure(Exception(f"No checklist items found in {self.tasks_file}"))
                return Success(tasks)

            data = json.loads(self.tasks_file.read_text())
            tasks_data = data.get("tasks", [])
            tasks = [Task.from_dict(t) for t in tasks_data]
            return Success(tasks)
        except Exception as exc:
            return Failure(exc)


@dataclass(frozen=True, slots=True)
class CLITaskMasterClient:
    """Legacy TaskMaster CLI client retained for compatibility only."""

    def get_next_task(self) -> Result[Task, Exception]:
        """Get the next available task via taskmaster CLI."""
        tasks_result = self.get_all_tasks()
        if isinstance(tasks_result, Failure):
            return tasks_result

        tasks = tasks_result.unwrap()

        # Filter for pending tasks that aren't blocked
        available = [
            t for t in tasks
            if t.status == "pending" and not t.blocked_by
        ]

        if not available:
            return Failure(Exception("No available tasks"))

        # Sort by priority (lowest number = highest priority)
        available.sort(key=lambda t: t.priority)

        return Success(available[0])

    def get_task_by_id(self, task_id: str) -> Result[Task, Exception]:
        """Get a specific task by ID via CLI."""
        try:
            result = subprocess.run(
                ["task-master", "get", task_id, "--format", "json"],
                capture_output=True,
                text=True,
                check=True,
            )
            task_data = json.loads(result.stdout)
            return Success(Task.from_dict(task_data))
        except subprocess.CalledProcessError as e:
            return Failure(Exception(f"task-master get failed: {e.stderr}"))
        except FileNotFoundError:
            return Failure(Exception("task-master CLI not found"))
        except Exception as exc:
            return Failure(exc)

    def update_task_status(
        self, task_id: str, status: str
    ) -> Result[None, Exception]:
        """Update task status via CLI."""
        try:
            subprocess.run(
                ["task-master", "update", task_id, "--status", status],
                capture_output=True,
                text=True,
                check=True,
            )
            return Success(None)
        except subprocess.CalledProcessError as e:
            return Failure(Exception(f"task-master update failed: {e.stderr}"))
        except FileNotFoundError:
            return Failure(Exception("task-master CLI not found"))
        except Exception as exc:
            return Failure(exc)

    def add_task_note(
        self, task_id: str, note: str
    ) -> Result[None, Exception]:
        """Add a timestamped note via CLI."""
        try:
            timestamped_note = f"{datetime.now().isoformat()}: {note}"
            subprocess.run(
                ["task-master", "add-note", task_id, timestamped_note],
                capture_output=True,
                text=True,
                check=True,
            )
            return Success(None)
        except subprocess.CalledProcessError as e:
            return Failure(Exception(f"task-master add-note failed: {e.stderr}"))
        except FileNotFoundError:
            return Failure(Exception("task-master CLI not found"))
        except Exception as exc:
            return Failure(exc)

    def get_all_tasks(self) -> Result[list[Task], Exception]:
        """Get all tasks via CLI."""
        try:
            result = subprocess.run(
                ["task-master", "list", "--format", "json"],
                capture_output=True,
                text=True,
                check=True,
            )
            data = json.loads(result.stdout)
            tasks_data = data.get("tasks", [])
            tasks = [Task.from_dict(t) for t in tasks_data]
            return Success(tasks)
        except subprocess.CalledProcessError as e:
            return Failure(Exception(f"task-master list failed: {e.stderr}"))
        except FileNotFoundError:
            return Failure(Exception("task-master CLI not found"))
        except Exception as exc:
            return Failure(exc)


@dataclass(frozen=True, slots=True)
class MCPTaskMasterClient:
    """MCP-based TaskMaster client - communicates with TaskMaster MCP server."""

    server_url: str | None = None

    def get_next_task(self) -> Result[Task, Exception]:
        """Get the next available task via MCP."""
        # TODO: Implement MCP client using taskmaster-ai MCP tools
        # For now, raise NotImplementedError
        return Failure(NotImplementedError("MCP client not yet implemented"))

    def get_task_by_id(self, _task_id: str) -> Result[Task, Exception]:
        """Get a specific task by ID via MCP."""
        # TODO: Implement MCP client
        return Failure(NotImplementedError("MCP client not yet implemented"))

    def update_task_status(
        self, _task_id: str, _status: str
    ) -> Result[None, Exception]:
        """Update task status via MCP."""
        # TODO: Implement MCP client
        return Failure(NotImplementedError("MCP client not yet implemented"))

    def add_task_note(
        self, _task_id: str, _note: str
    ) -> Result[None, Exception]:
        """Add a timestamped note via MCP."""
        # TODO: Implement MCP client
        return Failure(NotImplementedError("MCP client not yet implemented"))

    def get_all_tasks(self) -> Result[list[Task], Exception]:
        """Get all tasks via MCP."""
        # TODO: Implement MCP client
        return Failure(NotImplementedError("MCP client not yet implemented"))


@dataclass(frozen=True, slots=True)
class B00tTaskClient:
    """Task client backed by `b00t-cli task` — the canonical task store."""

    _B00T_CLI = "b00t-cli"

    def _run(self, *args: str) -> Result[str, Exception]:
        try:
            result = subprocess.run(
                [self._B00T_CLI, "task", *args],
                capture_output=True, text=True, check=True,
            )
            return Success(result.stdout.strip())
        except subprocess.CalledProcessError as e:
            return Failure(Exception(f"b00t-cli task {args[0] if args else ''} failed: {e.stderr}"))
        except FileNotFoundError:
            return Failure(Exception("b00t-cli not found in PATH"))
        except Exception as exc:
            return Failure(exc)

    def get_all_tasks(self) -> Result[list[Task], Exception]:
        r = self._run("list", "--json")
        if isinstance(r, Failure):
            return r
        try:
            raw = json.loads(r.unwrap())
            tasks = []
            for item in (raw if isinstance(raw, list) else raw.get("tasks", [])):
                tasks.append(Task(
                    id=str(item.get("id", "")),
                    title=item.get("title", ""),
                    description=item.get("description", "") or item.get("notes", ""),
                    status=item.get("status", "pending"),
                    priority=int(item.get("priority", 99)),
                    acceptance_criteria=item.get("acceptance_criteria", []),
                    depends_on=[str(d) for d in item.get("depends_on", [])],
                    blocked_by=[str(d) for d in item.get("blocked_by", [])],
                    notes=[item.get("notes", "")] if item.get("notes") else [],
                    created_at=item.get("created_at", datetime.now().isoformat()),
                    updated_at=item.get("updated_at", datetime.now().isoformat()),
                ))
            return Success(tasks)
        except Exception as exc:
            return Failure(exc)

    def get_next_task(self) -> Result[Task, Exception]:
        r = self._run("next", "--json")
        if isinstance(r, Failure):
            return r
        try:
            item = json.loads(r.unwrap())
            return Success(Task(
                id=str(item["id"]),
                title=item.get("title", ""),
                description=item.get("description", "") or item.get("notes", ""),
                status=item.get("status", "pending"),
                priority=int(item.get("priority", 99)),
                acceptance_criteria=item.get("acceptance_criteria", []),
                depends_on=[str(d) for d in item.get("depends_on", [])],
                blocked_by=[str(d) for d in item.get("blocked_by", [])],
                notes=[item.get("notes", "")] if item.get("notes") else [],
                created_at=item.get("created_at", datetime.now().isoformat()),
                updated_at=item.get("updated_at", datetime.now().isoformat()),
            ))
        except Exception as exc:
            return Failure(exc)

    def get_task_by_id(self, task_id: str) -> Result[Task, Exception]:
        r = self._run("show", task_id, "--json")
        if isinstance(r, Failure):
            return r
        try:
            item = json.loads(r.unwrap())
            return Success(Task(
                id=str(item["id"]),
                title=item.get("title", ""),
                description=item.get("description", "") or item.get("notes", ""),
                status=item.get("status", "pending"),
                priority=int(item.get("priority", 99)),
                acceptance_criteria=item.get("acceptance_criteria", []),
                depends_on=[str(d) for d in item.get("depends_on", [])],
                blocked_by=[str(d) for d in item.get("blocked_by", [])],
                notes=[item.get("notes", "")] if item.get("notes") else [],
                created_at=item.get("created_at", datetime.now().isoformat()),
                updated_at=item.get("updated_at", datetime.now().isoformat()),
            ))
        except Exception as exc:
            return Failure(exc)

    def update_task_status(self, task_id: str, status: str) -> Result[None, Exception]:
        r = self._run("update", task_id, "--status", status)
        return Success(None) if isinstance(r, Success) else r  # type: ignore[return-value]

    def add_task_note(self, task_id: str, note: str) -> Result[None, Exception]:
        timestamped = f"{datetime.now().isoformat()}: {note}"
        r = self._run("update", task_id, "--notes", timestamped)
        return Success(None) if isinstance(r, Success) else r  # type: ignore[return-value]


def create_client(
    prefer_mcp: bool = False,
    mcp_url: str | None = None,
    tasks_file: Path | None = None,
) -> TaskMasterClient:
    """
    Factory function to create appropriate backlog client.

    Args:
        prefer_mcp: If True, try MCP first and fallback to file-based
        mcp_url: URL for MCP server (optional)
        tasks_file: Path to backlog file (default: ./TODO-next.md)

    Returns:
        TaskMasterClient implementation (MCP, legacy CLI, or file-based)
    """
    # b00t-cli task is the canonical source — try it first
    b00t_client = B00tTaskClient()
    probe = b00t_client.get_all_tasks()
    if isinstance(probe, Success):
        return b00t_client

    if prefer_mcp:
        mcp_client = MCPTaskMasterClient(server_url=mcp_url)
        test_result = mcp_client.get_all_tasks()
        if isinstance(test_result, Success):
            return mcp_client

    # File-based fallback (TODO-next.md or legacy JSON)
    if tasks_file is None:
        default_backlog = Path("TODO-next.md")
        if default_backlog.exists():
            return FileTaskMasterClient(tasks_file=default_backlog)
        legacy_backlog = Path(".taskmaster/tasks/tasks.json")
        if legacy_backlog.exists():
            return FileTaskMasterClient(tasks_file=legacy_backlog)

    return FileTaskMasterClient(tasks_file=tasks_file or Path("TODO-next.md"))


def get_current_branch() -> Maybe[str]:
    """Get branch name from git, with legacy task-master fallback."""
    try:
        result = subprocess.run(
            ["git", "branch", "--show-current"],
            capture_output=True,
            text=True,
            check=True,
        )
        branch = result.stdout.strip()
        if branch:
            return Some(branch)
    except Exception:
        pass

    try:
        result = subprocess.run(
            ["task-master", "metadata", "--field", "branchName"],
            capture_output=True,
            text=True,
            check=True,
        )
        branch = result.stdout.strip()
        if branch:
            return Some(branch)
        return Nothing
    except Exception:
        return Nothing
