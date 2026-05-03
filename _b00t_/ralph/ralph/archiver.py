"""Branch change detection and archival utilities."""

from __future__ import annotations

from datetime import date
from pathlib import Path

from returns.maybe import Maybe as Option
from returns.maybe import Nothing, Some
from returns.result import Failure, Result, Success

from ralph.file_manager import PRD_PATH, PROGRESS_PATH, get_current_branch

PACKAGE_DIR = PRD_PATH.parent
ARCHIVE_DIR = PACKAGE_DIR / "archive"
LAST_BRANCH_PATH = PACKAGE_DIR / ".last-branch"


def _sanitize_branch_name(branch: str) -> str:
    """Normalize branch names for archive folder usage."""

    cleaned = branch.strip().removeprefix("ralph/")
    cleaned = cleaned.replace("/", "-").replace(" ", "-")
    return cleaned or "unknown-branch"


def _read_last_branch() -> Option[str]:
    """Load the last recorded branch name."""

    if not LAST_BRANCH_PATH.exists():
        return Nothing
    try:
        content = LAST_BRANCH_PATH.read_text(encoding="utf-8").strip()
    except OSError:
        return Nothing
    if not content:
        return Nothing
    return Some(content)


def _write_last_branch(branch: str) -> Result[Path, Exception]:
    """Persist the current branch name to disk."""

    try:
        LAST_BRANCH_PATH.write_text(branch.strip(), encoding="utf-8")
    except OSError as exc:
        return Failure(exc)
    return Success(LAST_BRANCH_PATH)


def _reset_progress_file() -> Result[Path, Exception]:
    """Clear progress.txt when a new branch starts."""

    try:
        PROGRESS_PATH.write_text("", encoding="utf-8")
    except OSError as exc:
        return Failure(exc)
    return Success(PROGRESS_PATH)


def _copy_if_exists(source: Path, destination: Path) -> Result[None, Exception]:
    """Copy file contents when the source exists."""

    if not source.exists():
        return Success(None)
    try:
        destination.write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
    except OSError as exc:
        return Failure(exc)
    return Success(None)


def archive_previous_run(last_branch: str, current_branch: str) -> Result[Path, Exception]:
    """Archive prd.json and progress.txt for the previous branch."""

    if not last_branch.strip() or last_branch.strip() == current_branch.strip():
        return Success(ARCHIVE_DIR)

    date_str = date.today().isoformat()
    branch_label = _sanitize_branch_name(last_branch)
    archive_folder = ARCHIVE_DIR / f"{date_str}-{branch_label}"

    try:
        archive_folder.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        return Failure(exc)

    copy_prd_result = _copy_if_exists(PRD_PATH, archive_folder / PRD_PATH.name)
    match copy_prd_result:
        case Success(_):
            pass
        case Failure(error):
            return Failure(error)

    copy_progress_result = _copy_if_exists(PROGRESS_PATH, archive_folder / PROGRESS_PATH.name)
    match copy_progress_result:
        case Success(_):
            pass
        case Failure(error):
            return Failure(error)

    return Success(archive_folder)


def check_branch_change() -> bool:
    """Check for branch changes and archive previous runs."""

    current_branch_result = get_current_branch()
    match current_branch_result:
        case Some(current_branch):
            pass
        case _:
            return False

    last_branch_result = _read_last_branch()
    match last_branch_result:
        case Some(last_branch):
            pass
        case _:
            write_result = _write_last_branch(current_branch)
            if isinstance(write_result, Failure):
                return False
            return False

    if last_branch == current_branch:
        return False

    archive_result = archive_previous_run(last_branch, current_branch)
    if isinstance(archive_result, Failure):
        return False

    write_result = _write_last_branch(current_branch)
    if isinstance(write_result, Failure):
        return False

    reset_result = _reset_progress_file()
    return not isinstance(reset_result, Failure)


__all__ = [
    "ARCHIVE_DIR",
    "LAST_BRANCH_PATH",
    "archive_previous_run",
    "check_branch_change",
]
