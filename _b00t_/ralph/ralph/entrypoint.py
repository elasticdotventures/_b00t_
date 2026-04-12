from __future__ import annotations

import argparse
import datetime as _dt
import json
import logging
import os
import shlex
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

from fastmcp import FastMCP

# Keep the MCP surface area close to the runtime implementation so there is
# a single source of truth for "how Ralph runs".
mcp = FastMCP("Ralph Wiggum 🎯")

logging.basicConfig(level=logging.INFO, format="%(message)s")
LOGGER = logging.getLogger("ralph")

__version__ = "0.1.0"


def _project_root() -> Path:
    # Prefer the git root of the current working directory so `ralph` can be used
    # as a tool against *any* repo, not just this package's source checkout.
    cwd = Path.cwd().resolve()
    for candidate in (cwd, *cwd.parents):
        if (candidate / ".git").is_dir():
            return candidate
    return cwd


def _tasks_file(root: Path) -> Path:
    return root / ".taskmaster" / "tasks" / "tasks.json"


def _print_tasks_missing_instructions() -> None:
    # Keep this short and copy/paste friendly. It is printed from both bash and python
    # entrypoints to keep behavior consistent regardless of invocation method.
    sys.stderr.write("\n")
    sys.stderr.write("To create tasks with the ralph-prd skill, run your designated agent with this prompt:\n\n")
    sys.stderr.write(
        "Use the ralph-prd skill to generate TaskMaster tasks.json for this repo.\n"
        "Requirements:\n"
        "- Output must be TaskMaster format with tasks[] and metadata.\n"
        "- Include 3-7 small, actionable tasks with acceptance criteria.\n"
        "- Use IETF 2119 MUST/SHOULD/MAY in acceptance criteria.\n"
        "- Set metadata.project and metadata.branchName appropriately.\n"
    )
    sys.stderr.write("\nThen re-run: ./ralph.sh --agent <amp|claude|codex> [max_iterations]\n\n")


def _require_tasks(root: Path) -> bool:
    """Return True if tasks exist and are non-empty; otherwise print instructions and return False."""
    tasks_path = _tasks_file(root)
    if not tasks_path.exists():
        LOGGER.warning("TaskMaster tasks.json not found; nothing to do.")
        _print_tasks_missing_instructions()
        return False

    try:
        payload = json.loads(tasks_path.read_text())
    except json.JSONDecodeError:
        LOGGER.warning("TaskMaster tasks.json is invalid JSON; nothing to do.")
        _print_tasks_missing_instructions()
        return False

    tasks = payload.get("tasks") if isinstance(payload, dict) else None
    if not tasks:
        LOGGER.warning("TaskMaster tasks.json is empty; nothing to do.")
        _print_tasks_missing_instructions()
        return False

    return True


def _parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        prog="ralph",
        description="Ralph Wiggum - Long-running AI agent loop",
    )
    parser.add_argument(
        "--mcp",
        action="store_true",
        help="Run as an MCP server (ignores --agent/max_iterations)",
    )
    parser.add_argument(
        "--transport",
        choices=("stdio", "http"),
        default="stdio",
        help="MCP transport to use (default: stdio)",
    )
    parser.add_argument(
        "--host",
        default="127.0.0.1",
        help="HTTP host when using --transport http (default: 127.0.0.1)",
    )
    parser.add_argument(
        "--port",
        type=int,
        default=8000,
        help="HTTP port when using --transport http (default: 8000)",
    )

    parser.add_argument("--agent", choices=("amp", "claude", "codex", "opencode"))
    parser.add_argument("max_iterations", nargs="?", type=int, default=10)
    parser.add_argument(
        "--version",
        action="version",
        version=f"%(prog)s {__version__}",
    )

    args = parser.parse_args(argv)
    if not args.mcp and not args.agent:
        parser.error("--agent is required. Use --agent amp|claude|codex|opencode.")
    return args


def _ensure_progress_file(progress_file: Path) -> None:
    if progress_file.exists():
        return
    progress_file.write_text(
        "# Ralph Progress Log\n"
        f"Started: {_dt.datetime.now()}\n"
        "---\n"
    )


def _run_and_capture(cmd: list[str], stdin_path: Path | None = None) -> str:
    stdin = None
    try:
        if stdin_path is not None:
            stdin = stdin_path.open("r")
        proc = subprocess.Popen(
            cmd,
            stdin=stdin,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1,
        )
        assert proc.stdout is not None
        output_chunks: list[str] = []
        for line in proc.stdout:
            sys.stderr.write(line)
            output_chunks.append(line)
        proc.wait()
        return "".join(output_chunks)
    finally:
        if stdin is not None:
            stdin.close()


def main(argv: list[str] | None = None) -> int:
    """
    Ralph runtime execution.

    Note: Preflight checks (uv sync, .taskmaster setup, .gitignore) are handled
    by ralph.sh wrapper. This function focuses on execution only.
    """
    if argv is None:
        argv = sys.argv[1:]

    args = _parse_args(argv)
    root = _project_root()

    if args.mcp:
        # FastMCP consumes the transport + kwargs; we provide a stable CLI surface
        # without relying on sys.argv surgery.
        if args.transport == "http":
            mcp.run(transport="http", host=args.host, port=args.port)
        else:
            mcp.run(transport="stdio")
        return 0

    if not _require_tasks(root):
        return 1

    progress_file = root / "progress.txt"
    _ensure_progress_file(progress_file)

    codex_model = os.environ.get("CODEX_MODEL", "gpt-5.2-codex")
    codex_reasoning_effort = os.environ.get("CODEX_REASONING_EFFORT", "high")
    codex_sandbox = os.environ.get("CODEX_SANDBOX", "workspace-write")
    codex_extra_args = os.environ.get("CODEX_EXTRA_ARGS", "")

    LOGGER.info(
        "Starting Ralph - Agent: %s - Max iterations: %s",
        args.agent,
        args.max_iterations,
    )

    for i in range(1, args.max_iterations + 1):
        LOGGER.info("")
        LOGGER.info("===============================================================")
        LOGGER.info("  Ralph Iteration %s of %s (%s)", i, args.max_iterations, args.agent)
        LOGGER.info("===============================================================")

        if args.agent == "amp":
            output = _run_and_capture(
                ["amp", "--dangerously-allow-all"],
                stdin_path=root / "prompt.md",
            )
        elif args.agent == "codex":
            codex_args = [
                "codex",
                "exec",
                "-m",
                codex_model,
                "--config",
                f"model_reasoning_effort=\"{codex_reasoning_effort}\"",
                "--sandbox",
                codex_sandbox,
                "--dangerously-bypass-approvals-and-sandbox",
                "--cd",
                str(root),
            ]
            if codex_extra_args:
                codex_args.extend(shlex.split(codex_extra_args))
            codex_args.append("@ralph-next")
            output = _run_and_capture(codex_args)
        else:
            # Keep behavior consistent across agents: _run_and_capture already streams output.
            output = _run_and_capture(
                [
                    "claude",
                    "--model",
                    "sonnet",
                    "--dangerously-skip-permissions",
                    "--print",
                ],
                stdin_path=root / "CLAUDE.md",
            )

        if "<promise>COMPLETE</promise>" in output:
            LOGGER.info("")
            LOGGER.info("Ralph completed all tasks!")
            LOGGER.info("Completed at iteration %s of %s", i, args.max_iterations)
            return 0

        LOGGER.info("Iteration %s complete. Continuing...", i)
        time.sleep(2)

    LOGGER.info("")
    LOGGER.info(
        "Ralph reached max iterations (%s) without completing all tasks.",
        args.max_iterations,
    )
    LOGGER.info("Check %s for status.", progress_file)
    return 1


@mcp.tool()
def run_ralph_iteration(
    agent: str = "codex",
    max_iterations: int = 1,
) -> dict[str, str | int]:
    """Run Ralph autonomous agent for specified iterations."""
    args = ["--agent", agent, str(max_iterations)]

    exit_code = main(args)
    root = _project_root()
    progress_file = root / "progress.txt"

    return {
        "exit_code": exit_code,
        "status": "complete" if exit_code == 0 else "incomplete",
        "max_iterations": max_iterations,
        "progress_file": str(progress_file),
    }


@mcp.tool()
def get_ralph_status() -> dict[str, Any]:
    """Get current Ralph execution status from progress.txt."""
    root = _project_root()
    progress_file = root / "progress.txt"

    if not progress_file.exists():
        return {"status": "no_progress_file", "message": "No progress.txt found"}

    content = progress_file.read_text()
    lines = content.strip().split("\n")

    return {
        "status": "active",
        "progress_file": str(progress_file),
        "last_lines": lines[-10:] if len(lines) > 10 else lines,
        "total_lines": len(lines),
    }


@mcp.tool()
def get_task_status() -> dict[str, Any]:
    """Get TaskMaster completion status via taskmaster CLI."""
    try:
        result = subprocess.run(
            ["taskmaster", "list", "--format", "json"],
            capture_output=True,
            text=True,
            check=True,
        )
        task_data = json.loads(result.stdout)

        total = len(task_data.get("tasks", []))
        completed = sum(1 for t in task_data.get("tasks", []) if t.get("status") == "done")
        in_progress = sum(1 for t in task_data.get("tasks", []) if t.get("status") == "in-progress")
        pending = sum(1 for t in task_data.get("tasks", []) if t.get("status") == "pending")

        return {
            "status": "loaded",
            "project": task_data.get("metadata", {}).get("project", "Unknown"),
            "total_tasks": total,
            "completed": completed,
            "in_progress": in_progress,
            "pending": pending,
            "completion_percentage": round((completed / total) * 100, 1) if total > 0 else 0,
        }
    except subprocess.CalledProcessError as e:
        return {"status": "error", "message": f"taskmaster CLI error: {e}"}
    except FileNotFoundError:
        return {"status": "error", "message": "taskmaster CLI not found. Install taskmaster-ai first."}


@mcp.resource("ralph://tasks")
def get_tasks_resource() -> str:
    """Get current tasks via taskmaster CLI (not direct file access)."""
    try:
        result = subprocess.run(
            ["taskmaster", "list", "--format", "json"],
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout
    except subprocess.CalledProcessError:
        return json.dumps({"error": "Failed to fetch tasks from taskmaster"})
    except FileNotFoundError:
        return json.dumps({"error": "taskmaster CLI not found"})


@mcp.resource("ralph://progress")
def get_progress_resource() -> str:
    """Get the current progress log as a resource."""
    root = _project_root()
    progress_file = root / "progress.txt"

    if not progress_file.exists():
        return "No progress file found"

    return progress_file.read_text()
