"""Unit tests for progress display module."""

from __future__ import annotations

from ralph.progress_display import (
    ProgressStats,
    compute_progress_stats,
    display_progress_bar,
    display_progress_summary,
    display_task_tree,
)
from ralph.taskmaster_adapter import Task


def test_compute_progress_stats_empty() -> None:
    """Test compute_progress_stats() with empty task list."""
    stats = compute_progress_stats([])
    assert stats.total_tasks == 0
    assert stats.completed == 0
    assert stats.in_progress == 0
    assert stats.pending == 0
    assert stats.blocked == 0


def test_compute_progress_stats_mixed() -> None:
    """Test compute_progress_stats() with mixed task statuses."""
    tasks = [
        Task(
            id="task-001",
            title="Done Task",
            description="",
            status="done",
            priority=1,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-002",
            title="In Progress Task",
            description="",
            status="in-progress",
            priority=2,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-003",
            title="Pending Task",
            description="",
            status="pending",
            priority=3,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-004",
            title="Blocked Task",
            description="",
            status="pending",
            priority=4,
            acceptance_criteria=[],
            depends_on=["task-001"],
            blocked_by=["task-001"],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
    ]

    stats = compute_progress_stats(tasks)
    assert stats.total_tasks == 4
    assert stats.completed == 1
    assert stats.in_progress == 1
    assert stats.pending == 2
    assert stats.blocked == 1


def test_compute_progress_stats_all_done() -> None:
    """Test compute_progress_stats() with all tasks completed."""
    tasks = [
        Task(
            id=f"task-{i}",
            title=f"Task {i}",
            description="",
            status="done",
            priority=i,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        )
        for i in range(5)
    ]

    stats = compute_progress_stats(tasks)
    assert stats.total_tasks == 5
    assert stats.completed == 5
    assert stats.in_progress == 0
    assert stats.pending == 0
    assert stats.blocked == 0


def test_display_progress_bar_empty() -> None:
    """Test display_progress_bar() with zero tasks."""
    stats = ProgressStats(
        total_tasks=0,
        completed=0,
        in_progress=0,
        pending=0,
        blocked=0,
    )
    bar = display_progress_bar(stats, width=10)
    assert "[" in bar
    assert "]" in bar
    assert "0.0%" in bar
    assert bar == "[░░░░░░░░░░] 0.0%"


def test_display_progress_bar_partial() -> None:
    """Test display_progress_bar() with partial completion."""
    stats = ProgressStats(
        total_tasks=10,
        completed=5,
        in_progress=1,
        pending=4,
        blocked=0,
    )
    bar = display_progress_bar(stats, width=10)
    assert "█" in bar  # Filled blocks
    assert "░" in bar  # Empty blocks
    assert "50.0%" in bar
    assert bar == "[█████░░░░░] 50.0%"


def test_display_progress_bar_full() -> None:
    """Test display_progress_bar() with full completion."""
    stats = ProgressStats(
        total_tasks=10,
        completed=10,
        in_progress=0,
        pending=0,
        blocked=0,
    )
    bar = display_progress_bar(stats, width=10)
    assert bar == "[██████████] 100.0%"


def test_display_progress_bar_custom_width() -> None:
    """Test display_progress_bar() with custom width."""
    stats = ProgressStats(
        total_tasks=4,
        completed=2,
        in_progress=0,
        pending=2,
        blocked=0,
    )
    bar = display_progress_bar(stats, width=20)
    assert len(bar) > 20  # Includes percentage
    assert "50.0%" in bar
    assert bar.count("█") == 10  # 50% of 20
    assert bar.count("░") == 10


def test_display_task_tree_empty() -> None:
    """Test display_task_tree() with empty task list."""
    tree = display_task_tree([])
    assert tree == "(no tasks)"


def test_display_task_tree_single() -> None:
    """Test display_task_tree() with single task."""
    tasks = [
        Task(
            id="task-001",
            title="Single Task",
            description="",
            status="pending",
            priority=1,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        )
    ]
    tree = display_task_tree(tasks)
    assert "└─" in tree  # Last item marker
    assert "task-001" in tree
    assert "Single Task" in tree
    assert "○" in tree  # Pending icon
    assert "[pending]" in tree


def test_display_task_tree_multiple() -> None:
    """Test display_task_tree() with multiple tasks."""
    tasks = [
        Task(
            id="task-001",
            title="First",
            description="",
            status="done",
            priority=1,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-002",
            title="Second",
            description="",
            status="in-progress",
            priority=2,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
    ]
    tree = display_task_tree(tasks)
    assert "├─" in tree  # First item marker
    assert "└─" in tree  # Last item marker
    assert "task-001" in tree
    assert "task-002" in tree
    assert "✓" in tree  # Done icon
    assert "⚡" in tree  # In-progress icon


def test_display_task_tree_with_blocked() -> None:
    """Test display_task_tree() shows blocked dependencies."""
    tasks = [
        Task(
            id="task-001",
            title="Blocked Task",
            description="",
            status="pending",
            priority=1,
            acceptance_criteria=[],
            depends_on=["task-000"],
            blocked_by=["task-000"],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        )
    ]
    tree = display_task_tree(tasks)
    assert "blocked by" in tree
    assert "task-000" in tree


def test_display_task_tree_status_icons() -> None:
    """Test display_task_tree() shows correct status icons."""
    tasks = [
        Task(
            id="task-done",
            title="Done",
            status="done",
            description="",
            priority=1,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-progress",
            title="Progress",
            status="in-progress",
            description="",
            priority=2,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-pending",
            title="Pending",
            status="pending",
            description="",
            priority=3,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-review",
            title="Review",
            status="review",
            description="",
            priority=4,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-cancelled",
            title="Cancelled",
            status="cancelled",
            description="",
            priority=5,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
    ]
    tree = display_task_tree(tasks)
    assert "✓" in tree  # done
    assert "⚡" in tree  # in-progress
    assert "○" in tree  # pending
    assert "👀" in tree  # review
    assert "✗" in tree  # cancelled


def test_display_task_tree_truncates_long_titles() -> None:
    """Test display_task_tree() truncates long titles."""
    tasks = [
        Task(
            id="task-001",
            title="A" * 100,  # Very long title
            description="",
            status="pending",
            priority=1,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        )
    ]
    tree = display_task_tree(tasks)
    assert "..." in tree  # Truncation indicator
    # Title should be truncated to 50 chars + "..."
    assert tree.count("A") <= 50


def test_display_progress_summary_complete() -> None:
    """Test display_progress_summary() generates complete output."""
    tasks = [
        Task(
            id="task-001",
            title="Test Task",
            description="",
            status="done",
            priority=1,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
        Task(
            id="task-002",
            title="Pending Task",
            description="",
            status="pending",
            priority=2,
            acceptance_criteria=[],
            depends_on=[],
            blocked_by=[],
            notes=[],
            created_at="2026-02-01T00:00:00Z",
            updated_at="2026-02-01T00:00:00Z",
        ),
    ]

    summary = display_progress_summary(tasks)

    # Check all components are present
    assert "=" in summary  # Separator
    assert "Progress:" in summary  # Progress bar label
    assert "[" in summary  # Progress bar
    assert "]" in summary
    assert "%" in summary
    assert "Completed:" in summary
    assert "In Progress:" in summary
    assert "Pending:" in summary
    assert "Blocked:" in summary
    assert "Task Tree:" in summary
    assert "task-001" in summary
    assert "task-002" in summary


def test_display_progress_summary_empty() -> None:
    """Test display_progress_summary() with empty task list."""
    summary = display_progress_summary([])
    assert "0/0" in summary
    assert "(no tasks)" in summary
