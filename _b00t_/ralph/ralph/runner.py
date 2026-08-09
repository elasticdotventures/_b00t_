"""OODA loop runner for Ralph autonomous agent.

Cycle: Observe → Orient → Decide → Act
Terminates when no pending tasks remain or tool emits <promise>COMPLETE</promise>.
"""

from __future__ import annotations

import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import TypeVar

from returns.result import Failure, Result, Success

from ralph.config import RalphConfig
from ralph.executors import (
    AmpExecutor,
    ClaudeExecutor,
    CodexExecutor,
    OpenCodeExecutor,
    PiExecutor,
    ToolExecutor,
)
from ralph.logging_utils import (
    configure_logging,
    log_error,
    log_info,
    log_success,
    log_warning,
)
from ralph.progress_display import display_progress_summary
from ralph.taskmaster_adapter import Task, create_client

WORKING_DIR = Path.cwd()
PROMPT_FILE = WORKING_DIR / "prompt.md"
CLAUDE_PROMPT_FILE = WORKING_DIR / "CLAUDE.md"
COMPLETE_MARKER = "<promise>COMPLETE</promise>"
CAKE = "🍰"

ResultValue = TypeVar("ResultValue")


def _unwrap_result(result: Result[ResultValue, Exception], message: str) -> ResultValue:
    if isinstance(result, Failure):
        raise RuntimeError(message) from result.failure()
    return result.unwrap()


def _check_for_completion(output: str) -> bool:
    return COMPLETE_MARKER in output


def _build_executor(tool: str, config: RalphConfig) -> ToolExecutor:
    match tool:
        case "amp":
            return AmpExecutor(prompt_path=PROMPT_FILE, working_dir=WORKING_DIR)
        case "claude":
            return ClaudeExecutor(prompt_path=CLAUDE_PROMPT_FILE, working_dir=WORKING_DIR)
        case "codex":
            return CodexExecutor(config=config, working_dir=WORKING_DIR)
        case "opencode":
            return OpenCodeExecutor(
                prompt_path=PROMPT_FILE,
                working_dir=WORKING_DIR,
                model=config.opencode_model,
                extra_args=config.opencode_extra_args,
            )
        case "pi":
            return PiExecutor(
                prompt_path=PROMPT_FILE,
                working_dir=WORKING_DIR,
                provider=config.pi_provider,
                model=config.pi_model,
                extra_args=config.pi_extra_args,
            )
    raise ValueError(f"Unsupported tool: {tool}")


def _pending_count(tasks: list[Task]) -> int:
    return sum(1 for t in tasks if t.status in ("pending", "in-progress"))


# ── OODA phases ────────────────────────────────────────────────────────────────

def _observe(taskmaster, logger) -> list[Task]:
    """OBSERVE: snapshot current task state."""
    tasks_result = taskmaster.get_all_tasks()
    if isinstance(tasks_result, Failure):
        log_warning(logger, f"Observe failed: {tasks_result.failure()}")
        return []
    return tasks_result.unwrap()


def _orient(tasks: list[Task], logger) -> Task | None:
    """ORIENT: find highest-priority actionable task."""
    available = [
        t for t in tasks
        if t.status == "pending" and not t.blocked_by
    ]
    if not available:
        return None
    available.sort(key=lambda t: t.priority)
    task = available[0]
    log_info(logger, f"  orient → #{task.id} {task.title!r} (P{task.priority})")
    return task


def _orient_many(tasks: list[Task], n: int, logger) -> list[Task]:
    """ORIENT (fan-out): find up to n highest-priority actionable tasks.

    Generalizes _orient() for the swarm/parallel path (#311): instead of a
    single task, returns a batch of unblocked pending tasks sized to the
    requested parallelism so the caller can fan them out concurrently.
    """
    available = [
        t for t in tasks
        if t.status == "pending" and not t.blocked_by
    ]
    if not available:
        return []
    available.sort(key=lambda t: t.priority)
    batch = available[:n]
    for task in batch:
        log_info(logger, f"  orient → #{task.id} {task.title!r} (P{task.priority})")
    return batch


def _decide(task: Task | None, iteration: int, max_iter: int, logger) -> str:
    """DECIDE: return action string or 'halt'."""
    if task is None:
        log_info(logger, "  decide → no actionable tasks; halt")
        return "halt"
    if iteration > max_iter:
        log_warning(logger, f"  decide → max iterations ({max_iter}) reached; halt")
        return "halt"
    log_info(logger, f"  decide → execute task #{task.id}")
    return "execute"


def _act(task: Task, executor: ToolExecutor, taskmaster, logger) -> str | None:
    """ACT: mark in-progress, run executor, return output or None on failure."""
    taskmaster.update_task_status(task.id, "in-progress")
    try:
        result = executor.run()
        if isinstance(result, Failure):
            log_error(logger, "Execution failed", result.failure())
            taskmaster.update_task_status(task.id, "pending")
            return None
        return result.unwrap()
    except Exception as exc:
        log_error(logger, "Unexpected executor error", exc)
        taskmaster.update_task_status(task.id, "pending")
        return None


# ── Main OODA loop ──────────────────────────────────────────────────────────

def run_ralph(config: RalphConfig, max_iterations: int) -> int:
    """Run the OODA loop for up to max_iterations cycles."""
    logger = configure_logging()

    log_info(logger, f"ralph OODA start — tool={config.tool} max={max_iterations}")

    taskmaster = create_client(prefer_mcp=config.use_mcp, mcp_url=config.taskmaster_url)
    executor = _build_executor(config.tool, config)

    for iteration in range(1, max_iterations + 1):
        log_info(logger, "")
        log_info(logger, f"{'═' * 63}")
        log_info(logger, f"OODA cycle {iteration}/{max_iterations}")
        log_info(logger, f"{'═' * 63}")

        # OBSERVE
        tasks = _observe(taskmaster, logger)
        if not tasks:
            log_warning(logger, "No tasks found — halting")
            break

        summary = display_progress_summary(tasks)
        log_info(logger, "\n" + summary)

        pending = _pending_count(tasks)
        if pending == 0:
            log_success(logger, f"All tasks done!  {CAKE}  Mission accomplished.  {CAKE}")
            return 0

        # ORIENT
        task = _orient(tasks, logger)

        # DECIDE
        action = _decide(task, iteration, max_iterations, logger)
        if action == "halt":
            break

        # ACT
        output = _act(task, executor, taskmaster, logger)  # type: ignore[arg-type]
        if output is None:
            log_warning(logger, f"Iteration {iteration}: executor returned no output; continuing")
            time.sleep(2)
            continue

        if _check_for_completion(output):
            taskmaster.update_task_status(task.id, "done")  # type: ignore[union-attr]
            tasks = _observe(taskmaster, logger)
            if _pending_count(tasks) == 0:
                log_success(logger, "")
                log_success(logger, f"  {CAKE}  Ralph completed the mission!  {CAKE}")
                log_success(logger, "")
                return 0
            remaining = _pending_count(tasks)
            log_info(logger, f"Task #{task.id} done — {remaining} remaining")  # type: ignore[union-attr]
        else:
            log_info(logger, f"Iteration {iteration} complete. Continuing OODA…")

        time.sleep(1)

    log_warning(logger, f"OODA loop ended without mission completion after {max_iterations} cycles.")
    return 1


# ── Parallel (fan-out/join) OODA loop — issue #311 ──────────────────────────

def run_ralph_parallel(config: RalphConfig, max_iterations: int, parallel_n: int) -> int:
    """Run the OODA loop, fanning each cycle out to up to parallel_n tasks.

    Swarm/fan-out pattern (gist.github.com/kieranklaassen/4f2aba89594a4aea4ad64d753984b2ea
    as assimilated in issue #311): each OODA cycle selects a batch of up to
    parallel_n unblocked pending tasks and runs one executor per task
    concurrently via a thread pool (executors are stateless frozen dataclasses
    wrapping subprocess.Popen — safe to invoke concurrently), then joins by
    marking each task done/pending from its own output. parallel_n=1 callers
    should use run_ralph() instead; this path exists for parallel_n > 1.
    """
    logger = configure_logging()

    log_info(
        logger,
        f"ralph OODA start (parallel={parallel_n}) — tool={config.tool} max={max_iterations}",
    )

    taskmaster = create_client(prefer_mcp=config.use_mcp, mcp_url=config.taskmaster_url)
    executor = _build_executor(config.tool, config)

    for iteration in range(1, max_iterations + 1):
        log_info(logger, "")
        log_info(logger, f"{'═' * 63}")
        log_info(logger, f"OODA cycle {iteration}/{max_iterations} (batch ≤{parallel_n})")
        log_info(logger, f"{'═' * 63}")

        # OBSERVE
        tasks = _observe(taskmaster, logger)
        if not tasks:
            log_warning(logger, "No tasks found — halting")
            break

        summary = display_progress_summary(tasks)
        log_info(logger, "\n" + summary)

        pending = _pending_count(tasks)
        if pending == 0:
            log_success(logger, f"All tasks done!  {CAKE}  Mission accomplished.  {CAKE}")
            return 0

        # ORIENT (fan-out)
        batch = _orient_many(tasks, parallel_n, logger)
        if not batch:
            log_info(logger, "  decide → no actionable tasks; halt")
            break

        # ACT (parallel)
        with ThreadPoolExecutor(max_workers=len(batch)) as pool:
            future_to_task = {
                pool.submit(_act, task, executor, taskmaster, logger): task
                for task in batch
            }
            outputs: dict[str, str | None] = {}
            for future in as_completed(future_to_task):
                task = future_to_task[future]
                outputs[task.id] = future.result()

        # JOIN
        completed_any = False
        for task in batch:
            output = outputs.get(task.id)
            if output is None:
                log_warning(logger, f"Task #{task.id}: executor returned no output; will retry")
                continue
            if _check_for_completion(output):
                taskmaster.update_task_status(task.id, "done")
                completed_any = True
                log_info(logger, f"Task #{task.id} done")
            else:
                log_info(logger, f"Task #{task.id}: iteration complete, continuing OODA…")

        tasks = _observe(taskmaster, logger)
        if _pending_count(tasks) == 0:
            log_success(logger, "")
            log_success(logger, f"  {CAKE}  Ralph completed the mission!  {CAKE}")
            log_success(logger, "")
            return 0

        if not completed_any:
            time.sleep(1)

    log_warning(logger, f"OODA loop ended without mission completion after {max_iterations} cycles.")
    return 1


__all__ = ["run_ralph", "run_ralph_parallel"]
