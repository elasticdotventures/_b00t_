#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = ["pygit2"]
# ///
# EXPERIMENTAL — safe corrupt git object pruner
# 🤓 safe-to-remove = corrupt ∩ unreachable (never delete reachable corrupt objects)
import sys, pathlib, subprocess
import pygit2

delete = len(sys.argv) > 1 and sys.argv[1] == "true"

repo = pygit2.Repository(pygit2.discover_repository("."))

# For worktrees repo.path is the worktree-specific gitdir; objects live in the common dir.
# Resolve via the `commondir` file written by git when creating the worktree.
_gitdir = pathlib.Path(repo.path)
_cdf = _gitdir / "commondir"
git_dir = (_gitdir / _cdf.read_text().strip()).resolve() if _cdf.exists() else _gitdir

corrupt = set()
for prefix in (git_dir / "objects").iterdir():
    if not prefix.is_dir() or len(prefix.name) != 2:
        continue
    for obj_file in prefix.iterdir():
        oid = prefix.name + obj_file.name
        try:
            repo.get(oid)
        except Exception:
            corrupt.add(oid)

fsck = subprocess.run(["git", "fsck", "--unreachable"], capture_output=True, text=True)
unreachable = {w for line in (fsck.stdout + fsck.stderr).splitlines()
               if line.startswith("unreachable")
               for w in [line.split()[-1]] if len(w) == 40}

safe = corrupt & unreachable
reachable_corrupt = corrupt - unreachable

print(f"corrupt: {len(corrupt)}  unreachable: {len(unreachable)}  safe-to-remove: {len(safe)}")

if reachable_corrupt:
    print(f"\n⚠️  {len(reachable_corrupt)} corrupt object(s) still REACHABLE — do not delete:")
    for h in sorted(reachable_corrupt):
        print(f"  {h}")

if not safe:
    print("nothing safe to remove.")
    sys.exit(0)

for h in sorted(safe):
    path = git_dir / "objects" / h[:2] / h[2:]
    if delete:
        path.unlink(missing_ok=True)
        print(f"  rm {path}")
    else:
        print(f"  would rm {path}")

if not delete:
    print("\ndry-run — pass true to delete: just git-prune-corrupt delete=true")
else:
    print("\ndone — run: git gc --prune=now")
