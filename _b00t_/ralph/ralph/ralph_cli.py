from __future__ import annotations

import argparse
import sys

__version__ = "0.1.0"


def cmd_run(args: argparse.Namespace) -> int:
    """Execute the agent loop."""
    from returns.result import Failure

    from ralph.config import RalphConfig
    from ralph.file_manager import initialize_progress_file
    from ralph.runner import run_ralph

    # Initialize progress file if it doesn't exist
    result = initialize_progress_file()
    if isinstance(result, Failure):
        print(f"Error initializing progress file: {result.failure()}", file=sys.stderr)
        return 1

    config = RalphConfig.from_env(tool=args.tool)

    # Handle dry-run mode
    if args.dry_run:
        print(f"[DRY RUN] Would execute: tool={args.tool}, max_iterations={args.max_iterations}")
        if args.task_id:
            print(f"[DRY RUN] Would target task: {args.task_id}")
        return 0

    return run_ralph(config, args.max_iterations)


def cmd_status(_args: argparse.Namespace) -> int:
    """Show current task status summary."""
    from returns.result import Failure

    from ralph.progress_display import display_progress_summary
    from ralph.taskmaster_adapter import create_client

    client = create_client(prefer_mcp=False)
    tasks_result = client.get_all_tasks()

    if isinstance(tasks_result, Failure):
        print(f"Error loading tasks: {tasks_result.failure()}", file=sys.stderr)
        return 1

    tasks = tasks_result.unwrap()
    summary = display_progress_summary(tasks)
    print(summary)
    return 0


def cmd_list_tasks(args: argparse.Namespace) -> int:
    """List all tasks with optional filtering."""
    from returns.result import Failure

    from ralph.taskmaster_adapter import create_client

    client = create_client(prefer_mcp=False)
    tasks_result = client.get_all_tasks()

    if isinstance(tasks_result, Failure):
        print(f"Error loading tasks: {tasks_result.failure()}", file=sys.stderr)
        return 1

    tasks = tasks_result.unwrap()

    # Apply filter
    if args.filter != "all":
        tasks = [t for t in tasks if t.status == args.filter]

    # Sort by priority
    tasks.sort(key=lambda t: t.priority)

    if not tasks:
        print(f"No tasks found with filter: {args.filter}")
        return 0

    # Display tasks in table format
    print(f"\n{'ID':<12} {'Priority':<10} {'Status':<15} {'Title':<50}")
    print("-" * 87)
    for task in tasks:
        title = task.title[:47] + "..." if len(task.title) > 50 else task.title
        print(f"{task.id:<12} {task.priority:<10} {task.status:<15} {title:<50}")

    print(f"\nTotal: {len(tasks)} task(s)")
    return 0


def main(argv: list[str] | None = None) -> int:
    """Main entry point for ralph CLI."""
    # Create main parser
    parser = argparse.ArgumentParser(
        prog="ralph",
        description="Ralph - Autonomous coding agent loop runner",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Run with default tool (amp) for 10 iterations
  ralph run

  # Run with Claude for 5 iterations
  ralph run --tool claude --max-iterations 5

  # Run with OpenCode
  ralph run --tool opencode

  # Show current task status
  ralph status

  # List all pending tasks
  ralph list-tasks --filter pending

  # Dry-run mode (no actual execution)
  ralph run --tool amp --dry-run
        """,
    )

    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )

    # Create subparsers
    subparsers = parser.add_subparsers(
        title="commands",
        description="Available commands",
        dest="command",
        required=True,
    )

    # 'run' subcommand
    run_parser = subparsers.add_parser(
        "run",
        help="Execute the agent loop",
        description="Execute the Ralph agent loop with specified tool and options",
    )
    run_parser.add_argument(
        "--tool",
        choices=["amp", "claude", "codex", "opencode"],
        default="amp",
        help="Tool to run (default: amp)",
    )
    run_parser.add_argument(
        "--max-iterations",
        type=int,
        default=10,
        metavar="N",
        help="Maximum iterations to run (default: 10)",
    )
    run_parser.add_argument(
        "--task-id",
        metavar="TASK",
        help="Target a specific task ID",
    )
    run_parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be executed without running",
    )
    run_parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Enable verbose output",
    )
    run_parser.set_defaults(func=cmd_run)

    # 'status' subcommand
    status_parser = subparsers.add_parser(
        "status",
        help="Show current task status summary",
        description="Display progress summary with visual task tree",
    )
    status_parser.set_defaults(func=cmd_status)

    # 'list-tasks' subcommand
    list_parser = subparsers.add_parser(
        "list-tasks",
        help="List all tasks",
        description="List all tasks with optional status filtering",
    )
    list_parser.add_argument(
        "--filter",
        choices=["all", "pending", "in-progress", "done", "review", "cancelled"],
        default="all",
        help="Filter tasks by status (default: all)",
    )
    list_parser.set_defaults(func=cmd_list_tasks)

    # Parse and execute
    args = parser.parse_args(argv)
    result: int = args.func(args)
    return result


if __name__ == "__main__":
    sys.exit(main())
