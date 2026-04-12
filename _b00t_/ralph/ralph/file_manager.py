from __future__ import annotations

import json
from datetime import datetime
from pathlib import Path
from typing import Any

from returns.maybe import Maybe, Nothing, Some
from returns.result import Failure, Result, Success

# Type alias for backwards compatibility
Option = Maybe


def _project_root() -> Path:
    """Get the current working directory (where ralph is being run from)."""
    return Path.cwd()


def _default_prd_path() -> Path:
    return _project_root() / "prd.json"


def _default_progress_path() -> Path:
    return _project_root() / "progress.txt"


# Export constants for use by other modules
PRD_PATH = _default_prd_path()
PROGRESS_PATH = _default_progress_path()


def read_prd(prd_path: Path | None = None) -> Result[dict[str, Any], Exception]:
    """Read and parse the PRD JSON file."""
    target = prd_path or _default_prd_path()
    try:
        content = target.read_text()
        data = json.loads(content)
    except Exception as exc:
        return Failure(exc)
    return Success(data)


def get_current_branch(prd_path: Path | None = None) -> Maybe[str]:
    """Get the configured branch name from the PRD."""
    result = read_prd(prd_path)
    if isinstance(result, Failure):
        return Nothing
    data = result.unwrap()
    branch = data.get("branchName")
    if isinstance(branch, str) and branch:
        return Some(branch)
    return Nothing


def initialize_progress_file(progress_path: Path | None = None) -> Result[None, Exception]:
    """Create progress.txt with a header if it does not exist."""
    target = progress_path or _default_progress_path()
    try:
        if target.exists():
            return Success(None)
        target.write_text(f"# Ralph Progress Log\nStarted: {datetime.now()}\n---\n")
    except Exception as exc:
        return Failure(exc)
    return Success(None)


def append_to_progress(
    message: str,
    progress_path: Path | None = None,
) -> Result[None, Exception]:
    """Append a message to the progress log."""
    target = progress_path or _default_progress_path()
    try:
        with target.open("a", encoding="utf-8") as handle:
            if message.endswith("\n"):
                handle.write(message)
            else:
                handle.write(f"{message}\n")
    except Exception as exc:
        return Failure(exc)
    return Success(None)
