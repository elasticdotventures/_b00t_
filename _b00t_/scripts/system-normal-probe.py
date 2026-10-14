#!/usr/bin/env python3
"""Read-only Git health observations: 0 satisfied, 1 violated, 2 unknown.

This script does not repair a repository, drop stashes or initialize submodules.
The gate evaluator sets its working directory to the checklist's datum directory.
"""

import subprocess
import sys
from collections.abc import Sequence


def observe(argv: Sequence[str]) -> subprocess.CompletedProcess[bytes] | None:
    try:
        return subprocess.run(argv, capture_output=True, timeout=2, check=False)
    except (OSError, subprocess.TimeoutExpired):
        return None


def git_health(check: str) -> int:
    if check == "git-no-conflicts":
        result = observe(["git", "ls-files", "--unmerged", "-z"])
        if result is None or result.returncode != 0:
            return 2
        return int(bool(result.stdout))
    if check == "git-task-branch":
        result = observe(["git", "symbolic-ref", "--quiet", "--short", "HEAD"])
        if result is None:
            return 2
        if result.returncode == 1:  # Git's documented detached-HEAD result.
            return 1
        if result.returncode != 0:
            return 2
        return int(result.stdout.strip() in {b"main", b"master", b""})
    if check == "git-stash-reviewed":
        result = observe(["git", "stash", "list", "--format=%H"])
        if result is None or result.returncode != 0:
            return 2
        return int(bool(result.stdout.strip()))
    if check == "git-submodule-pins":
        result = observe(["git", "submodule", "status"])
        if result is None or result.returncode != 0:
            return 2
        prefixes = [line[:1] for line in result.stdout.splitlines() if line]
        if any(prefix in {b"+", b"U"} for prefix in prefixes):
            return 1
        if any(prefix != b" " for prefix in prefixes):
            return 2  # Uninitialized modules are unavailable evidence.
        return 0
    return 2


def main() -> int:
    status = git_health(sys.argv[1]) if len(sys.argv) == 2 else 2
    print({0: "satisfied", 1: "violated", 2: "unknown"}[status])
    return status


if __name__ == "__main__":
    raise SystemExit(main())
