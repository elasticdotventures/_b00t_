"""Tool execution abstractions for the Ralph CLI."""

from __future__ import annotations

import contextlib
import io
import os
import subprocess
import sys
from collections.abc import Mapping, MutableMapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Protocol

from returns.result import Failure, Result, Success

from ralph.config import RalphConfig
from ralph.logging_utils import configure_logging, log_error

Command = Sequence[str]


class ToolExecutor(Protocol):
    """Protocol defining Ralph tool executors."""

    def run(self) -> Result[str, Exception]:
        """Execute the tool and return aggregated output."""


@dataclass(frozen=True, slots=True)
class ExecutorError(RuntimeError):
    """Structured execution error for subprocess failures."""

    detail: str
    command: tuple[str, ...] | None = None
    returncode: int | None = None
    output: str = ""

    def __str__(self) -> str:  # pragma: no cover - trivial
        message = self.detail
        if self.command:
            message = f"{message} [{' '.join(self.command)}]"
        if self.returncode is not None:
            message = f"{message} (exit={self.returncode})"
        if self.output:
            snippet = self.output.strip()
            if snippet:
                message = f"{message}: {snippet}"
        return message


class _TeeToStderr(io.TextIOBase):
    """File-like tee that mirrors writes to sys.stderr and buffers output."""

    def __init__(self) -> None:
        self._parts: list[str] = []
        # Create a pipe to provide fileno() support
        self._pipe_read, self._pipe_write = os.pipe()

    def writable(self) -> bool:  # pragma: no cover - io.TextIOBase requirement
        return True

    def write(self, data: str) -> int:
        sys.stderr.write(data)
        sys.stderr.flush()
        self._parts.append(data)
        # Also write to the pipe for fileno() compatibility
        with contextlib.suppress(OSError):
            os.write(self._pipe_write, data.encode("utf-8"))
        return len(data)

    def flush(self) -> None:  # pragma: no cover - delegated flush
        sys.stderr.flush()

    def fileno(self) -> int:
        """Return the write end of the pipe for subprocess compatibility."""
        return self._pipe_write

    def close(self) -> None:
        """Close the pipe file descriptors."""
        with contextlib.suppress(OSError):
            os.close(self._pipe_read)
        with contextlib.suppress(OSError):
            os.close(self._pipe_write)
        super().close()

    @property
    def value(self) -> str:
        return "".join(self._parts)


def _read_prompt(path: Path) -> Result[str, ExecutorError]:
    try:
        return Success(path.read_text(encoding="utf-8"))
    except OSError as exc:
        log_error(configure_logging(), f"Unable to read prompt file: {path}", exc)
        detail = f"Unable to read prompt file: {path}"
        return Failure(ExecutorError(detail=detail, output=str(exc)))


def _run_subprocess(
    command: Command,
    *,
    input_text: str | None = None,
    cwd: Path | None = None,
    env: Mapping[str, str] | None = None,
) -> Result[str, ExecutorError]:
    """
    Run subprocess with streaming output to prevent deadlock.

    Uses Popen with real-time stdout reading to avoid pipe buffer overflow
    that would cause subprocess.run() to hang.
    """
    cwd_value = str(cwd) if cwd is not None else None
    env_value: MutableMapping[str, str] | None = None
    if env is not None:
        env_value = dict(env)

    tee_stream = _TeeToStderr()
    returncode: int | None = None

    try:
        process = subprocess.Popen(
            command,
            stdin=subprocess.PIPE if input_text is not None else None,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            cwd=cwd_value,
            env=env_value,
        )

        # Send input and close stdin so child sees EOF
        if input_text is not None and process.stdin is not None:
            try:
                process.stdin.write(input_text)
                process.stdin.close()
            except OSError:
                # Ignore errors writing to stdin (e.g., if process exits early)
                with contextlib.suppress(OSError):
                    process.stdin.close()

        # Stream output from child, teeing to stderr and accumulating
        if process.stdout is not None:
            for line in process.stdout:
                tee_stream.write(line)
                sys.stderr.write(line)
                sys.stderr.flush()

        returncode = process.wait()

    except OSError as exc:
        log_error(configure_logging(), f"Failed to execute {' '.join(command)}", exc)
        detail = f"Failed to execute {' '.join(command)}"
        return Failure(ExecutorError(detail=detail, command=tuple(command), output=str(exc)))
    finally:
        tee_stream.close()

    output = tee_stream.value
    if returncode == 0:
        return Success(output)

    detail = f"Command {' '.join(command)} exited with {returncode}"
    return Failure(
        ExecutorError(
            detail=detail,
            command=tuple(command),
            returncode=returncode,
            output=output,
        )
    )


@dataclass(frozen=True, slots=True)
class AmpExecutor:
    """Executor for the amp CLI agent."""

    prompt_path: Path
    working_dir: Path | None = None

    def run(self) -> Result[str, Exception]:
        prompt_result = _read_prompt(self.prompt_path)
        match prompt_result:
            case Success(prompt_text):
                pass
            case Failure(error):
                return Failure(error)

        cwd_value = self.working_dir or self.prompt_path.parent
        command: Command = ("amp", "--dangerously-allow-all")
        return _run_subprocess(command, input_text=prompt_text, cwd=cwd_value)


@dataclass(frozen=True, slots=True)
class ClaudeExecutor:
    """Executor for Claude Code agent runs."""

    prompt_path: Path
    working_dir: Path | None = None

    def run(self) -> Result[str, Exception]:
        prompt_result = _read_prompt(self.prompt_path)
        match prompt_result:
            case Success(prompt_text):
                pass
            case Failure(error):
                return Failure(error)

        cwd_value = self.working_dir or self.prompt_path.parent
        command: Command = (
            "claude",
            "--model",
            "sonnet",
            "--dangerously-skip-permissions",
            "--print",
        )
        return _run_subprocess(command, input_text=prompt_text, cwd=cwd_value)


@dataclass(frozen=True, slots=True)
class CodexExecutor:
    """Executor for the Codex CLI with configuration-aware arguments."""

    config: RalphConfig
    working_dir: Path
    env: Mapping[str, str] | None = None

    def run(self) -> Result[str, Exception]:
        command: list[str] = [
            "codex",
            "exec",
            "-m",
            self.config.codex_model,
            "--config",
            f'model_reasoning_effort="{self.config.codex_reasoning_effort}"',
            "--sandbox",
            self.config.codex_sandbox,
            "--dangerously-bypass-approvals-and-sandbox",
            "--cd",
            str(self.working_dir),
        ]

        if self.config.codex_extra_args:
            command.extend(self.config.codex_extra_args.split())

        command.append("@ralph-next")

        env_vars: MutableMapping[str, str] = (
            dict(self.env) if self.env is not None else dict(os.environ)
        )

        env_vars.setdefault("CODEX_PROMPT_FILE", str(self.config.codex_prompt_file))
        env_vars.setdefault("CODEX_MODEL", self.config.codex_model)
        env_vars.setdefault("CODEX_REASONING_EFFORT", self.config.codex_reasoning_effort)
        env_vars.setdefault("CODEX_SANDBOX", self.config.codex_sandbox)
        env_vars.setdefault("CODEX_FULL_AUTO", "true" if self.config.codex_full_auto else "false")
        if self.config.codex_extra_args:
            env_vars.setdefault("CODEX_EXTRA_ARGS", self.config.codex_extra_args)

        return _run_subprocess(tuple(command), cwd=self.working_dir, env=env_vars)


@dataclass(frozen=True, slots=True)
class OpenCodeExecutor:
    """Executor for OpenCode CLI agent."""

    prompt_path: Path
    working_dir: Path | None = None
    model: str = "gpt-4"
    extra_args: str = ""

    def run(self) -> Result[str, Exception]:
        prompt_result = _read_prompt(self.prompt_path)
        match prompt_result:
            case Success(prompt_text):
                pass
            case Failure(error):
                return Failure(error)

        cwd_value = self.working_dir or self.prompt_path.parent
        command: list[str] = ["opencode", "--model", self.model]

        if self.extra_args:
            command.extend(self.extra_args.split())

        return _run_subprocess(tuple(command), input_text=prompt_text, cwd=cwd_value)


__all__ = [
    "AmpExecutor",
    "ClaudeExecutor",
    "CodexExecutor",
    "ExecutorError",
    "OpenCodeExecutor",
    "ToolExecutor",
]
