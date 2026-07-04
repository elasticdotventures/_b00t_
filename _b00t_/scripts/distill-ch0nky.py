#!/usr/bin/env python3
"""Compress ch0nky sub-agent transcript to diff+test output contract (≤50 lines).
Called by `just distill-ch0nky`. Input on stdin. Output to stdout."""
import sys
import re

lines = sys.stdin.read().split("\n")
diff_lines, test_lines = [], []
in_diff = False

for line in lines:
    if line.startswith("diff --git") or (line.startswith("---") and " a/" in line):
        in_diff = True
    if in_diff and (
        line.startswith("+") or line.startswith("-") or
        line.startswith("@@") or line.startswith("diff")
    ):
        diff_lines.append(line)
        if len(diff_lines) > 40:
            diff_lines = diff_lines[-40:]
    elif re.search(r"test result:|PASS|FAIL|\d+ passed|\d+ failed|ok\. \d+", line, re.I):
        test_lines.append(line)
    if not (line.startswith("+") or line.startswith("-") or line.startswith("@@")):
        in_diff = False

if not diff_lines and not test_lines:
    # No diff or tests found — emit last 10 lines as fallback
    print("\n".join(lines[-10:]))
    sys.exit(0)

if diff_lines:
    print("## diff")
    print("\n".join(diff_lines[-20:]))
if test_lines:
    print("\n## tests")
    print("\n".join(test_lines[-5:]))
