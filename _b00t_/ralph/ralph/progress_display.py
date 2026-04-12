"""Visual progress display for Ralph automation."""

from __future__ import annotations

from dataclasses import dataclass

from ralph.taskmaster_adapter import Task


@dataclass(frozen=True)
class ProgressStats:
    """Statistics about task progress."""

    total_tasks: int
    completed: int
    in_progress: int
    pending: int
    blocked: int


def compute_progress_stats(tasks: list[Task]) -> ProgressStats:
    """Compute progress statistics from a list of tasks."""
    completed = sum(1 for t in tasks if t.status == "done")
    in_progress = sum(1 for t in tasks if t.status == "in-progress")
    pending = sum(1 for t in tasks if t.status == "pending")
    blocked = sum(1 for t in tasks if t.status == "pending" and t.blocked_by)

    return ProgressStats(
        total_tasks=len(tasks),
        completed=completed,
        in_progress=in_progress,
        pending=pending,
        blocked=blocked,
    )


def display_progress_bar(stats: ProgressStats, width: int = 20) -> str:
    """Generate a Unicode box-drawing progress bar.

    Args:
        stats: Progress statistics
        width: Width of the progress bar in characters

    Returns:
        Formatted progress bar string like: [████████░░░░] 66.7%
    """
    percentage = 0.0 if stats.total_tasks == 0 else (stats.completed / stats.total_tasks) * 100

    filled_blocks = int((stats.completed / stats.total_tasks) * width) if stats.total_tasks > 0 else 0
    empty_blocks = width - filled_blocks

    bar = "█" * filled_blocks + "░" * empty_blocks
    return f"[{bar}] {percentage:.1f}%"


def display_task_tree(tasks: list[Task]) -> str:
    """Generate a dependency tree visualization with Unicode box-drawing.

    Args:
        tasks: List of tasks to display

    Returns:
        Multi-line string showing task tree with status icons
    """
    if not tasks:
        return "(no tasks)"

    # Status emoji mapping
    status_icons = {
        "done": "✓",
        "in-progress": "⚡",
        "pending": "○",
        "review": "👀",
        "cancelled": "✗",
    }

    # Sort tasks by priority
    sorted_tasks = sorted(tasks, key=lambda t: t.priority)

    lines = []
    for i, task in enumerate(sorted_tasks):
        is_last = i == len(sorted_tasks) - 1
        prefix = "└─" if is_last else "├─"

        icon = status_icons.get(task.status, "?")
        status_display = f"[{task.status}]"

        # Truncate title if too long
        title = task.title[:50] + "..." if len(task.title) > 50 else task.title

        line = f"{prefix} {task.id}: {icon} {title} {status_display}"
        lines.append(line)

        # Show blocked dependencies with indentation
        if task.blocked_by:
            indent = "   " if is_last else "│  "
            blockers = ", ".join(task.blocked_by)
            lines.append(f"{indent}└─ (blocked by: {blockers})")

    return "\n".join(lines)


def display_progress_summary(tasks: list[Task]) -> str:
    """Generate a complete progress summary with bar and tree.

    Args:
        tasks: List of tasks to display

    Returns:
        Multi-line formatted progress display
    """
    stats = compute_progress_stats(tasks)
    bar = display_progress_bar(stats)
    tree = display_task_tree(tasks)

    separator = "=" * 50
    summary_line = (
        f"Completed: {stats.completed}/{stats.total_tasks} | "
        f"In Progress: {stats.in_progress} | "
        f"Pending: {stats.pending} | "
        f"Blocked: {stats.blocked}"
    )

    return f"""{separator}
Progress: {bar}
{summary_line}
{separator}

Task Tree:
{tree}
"""


__all__ = [
    "ProgressStats",
    "compute_progress_stats",
    "display_progress_bar",
    "display_task_tree",
    "display_progress_summary",
]
